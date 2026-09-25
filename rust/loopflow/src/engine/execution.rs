use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::engine::flow::{ConcreteOp, ConcreteSkill, ConcreteStep, ConcreteXor};
use crate::engine::transitions::{
    finish_step, FlowProgress as TransitionProgress, FlowTransition, FlowVerdict,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepProgress {
    pub index: usize,
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionContext {
    pub progress: Option<StepProgress>,
    pub direction: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ExecutionCursor {
    pub index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child: Option<Box<NestedCursor>>,
    #[serde(default)]
    pub progress: TransitionProgress,
    #[serde(default)]
    pub iteration: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NestedCursor {
    Xor {
        selected: String,
        cursor: ExecutionCursor,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillOutcome {
    Completed { feedback: Option<String> },
    Waiting,
    Decided(FlowVerdict),
    Routed(String),
    Blocked(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowOutcome {
    Completed,
    Waiting,
    Blocked(String),
}

#[async_trait]
pub trait SkillExecutor: Send + Sync {
    /// Persist the complete root cursor before another boundary can execute.
    async fn checkpoint(&self, _cursor: &ExecutionCursor) -> Result<()> {
        Ok(())
    }

    async fn run_skill(&self, skill: &ConcreteSkill, ctx: ExecutionContext)
        -> Result<SkillOutcome>;

    async fn run_op(&self, ops: &ConcreteOp, ctx: ExecutionContext) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct FlowEngine<E> {
    executor: E,
}

impl<E> FlowEngine<E> {
    pub fn new(executor: E) -> Self {
        Self { executor }
    }
}

impl<E: SkillExecutor> FlowEngine<E> {
    pub async fn run(&self, items: &[ConcreteStep], start_index: usize) -> Result<FlowOutcome> {
        let mut cursor = ExecutionCursor {
            index: start_index,
            ..ExecutionCursor::default()
        };
        self.run_with_cursor(items, &mut cursor).await
    }

    pub async fn run_with_cursor(
        &self,
        items: &[ConcreteStep],
        cursor: &mut ExecutionCursor,
    ) -> Result<FlowOutcome> {
        while cursor.index < items.len() {
            let outcome = self.tick(items, cursor).await?;
            self.executor.checkpoint(cursor).await?;
            if let Some(outcome) = outcome {
                return Ok(outcome);
            }
        }
        Ok(FlowOutcome::Completed)
    }

    async fn tick(
        &self,
        items: &[ConcreteStep],
        cursor: &mut ExecutionCursor,
    ) -> Result<Option<FlowOutcome>> {
        let (body, leaf) = cursor.current_body(items);
        let Some(item) = body.get(leaf.index) else {
            // Older saved human settlements can leave a completed child
            // waiting to return to its parent.
            return settle_step(items, cursor);
        };
        if leaf.progress.verdict.is_some() || leaf.route.is_some() {
            return settle_step(items, cursor);
        }
        let ctx = ExecutionContext {
            progress: cursor.child.is_none().then_some(StepProgress {
                index: cursor.index,
                total: items.len(),
            }),
            direction: leaf.progress.direction.clone(),
        };
        let outcome = match item {
            ConcreteStep::Skill(skill) => self.executor.run_skill(skill, ctx).await?,
            ConcreteStep::Xor(branch) => {
                self.executor.run_skill(&branch.router_skill(), ctx).await?
            }
            ConcreteStep::Op(op) => {
                self.executor.run_op(op, ctx).await?;
                SkillOutcome::Completed { feedback: None }
            }
        };
        match outcome {
            SkillOutcome::Completed { feedback } => {
                if let Some(feedback) = feedback {
                    cursor.leaf_mut().progress.direction = Some(feedback);
                }
                settle_step(items, cursor)
            }
            SkillOutcome::Waiting => Ok(Some(FlowOutcome::Waiting)),
            SkillOutcome::Blocked(reason) => Ok(Some(FlowOutcome::Blocked(reason))),
            SkillOutcome::Decided(verdict) => {
                cursor.leaf_mut().progress.verdict = Some(verdict);
                Ok(None)
            }
            SkillOutcome::Routed(path) => {
                cursor.leaf_mut().route = Some(path);
                Ok(None)
            }
        }
    }
}

impl ConcreteXor {
    pub fn router_skill(&self) -> ConcreteSkill {
        ConcreteSkill {
            skill: self.router.clone(),
            policy: crate::engine::OccurrencePolicy::default(),
            flow_parents: self.flow_parents.clone(),
        }
    }
}

impl ExecutionCursor {
    pub fn leaf(&self) -> &Self {
        match self.child.as_deref() {
            Some(NestedCursor::Xor { cursor, .. }) => cursor.leaf(),
            None => self,
        }
    }

    pub fn leaf_mut(&mut self) -> &mut Self {
        match self.child {
            Some(ref mut child) => match child.as_mut() {
                NestedCursor::Xor { cursor, .. } => cursor.leaf_mut(),
            },
            None => self,
        }
    }

    /// Return no executable steps when the selection has no captured definition.
    pub fn current_body<'a>(&'a self, steps: &'a [ConcreteStep]) -> (&'a [ConcreteStep], &'a Self) {
        match self.child.as_deref() {
            Some(NestedCursor::Xor { selected, cursor }) => {
                match selected_body(steps, self.index, selected) {
                    Ok(body) => cursor.current_body(body),
                    Err(_) => (&[], cursor.leaf()),
                }
            }
            None => (steps, self),
        }
    }

    pub fn boundary_key(&self) -> String {
        let here = format!("{}:{}", self.index, self.iteration);
        match self.child.as_deref() {
            Some(NestedCursor::Xor {
                selected, cursor, ..
            }) => format!("{here}/{selected:?}/{}", cursor.boundary_key()),
            None => here,
        }
    }

    /// Settle one successful boundary, including a selected route or completed review.
    /// An error leaves the entire cursor unchanged.
    pub fn finish(&mut self, steps: &[ConcreteStep]) -> Result<bool> {
        let mut next = self.clone();
        next.finish_inner(steps)?;
        let finished = next.index == steps.len();
        *self = next;
        Ok(finished)
    }

    fn finish_inner(&mut self, steps: &[ConcreteStep]) -> Result<()> {
        if let Some(NestedCursor::Xor { selected, cursor }) = self.child.as_deref_mut() {
            let body = selected_body(steps, self.index, selected)?;
            if cursor.index > body.len() {
                return Err(anyhow!(
                    "XOR child cursor exceeds captured path {selected:?}"
                ));
            }
            let iteration = cursor.iteration;
            if cursor.index < body.len() {
                cursor.finish_inner(body)?;
            }
            self.iteration = self
                .iteration
                .checked_add(cursor.iteration - iteration)
                .ok_or_else(|| anyhow!("Flow iteration overflow"))?;
            if cursor.index < body.len() {
                return Ok(());
            }
            let prefix = format!("xor:{}:{selected}/", self.index);
            for (key, count) in &cursor.progress.repeats {
                self.progress
                    .repeats
                    .insert(format!("{prefix}{key}"), *count);
            }
            self.progress.direction = cursor.progress.direction.clone();
            self.child = None;
        } else if let Some(ConcreteStep::Xor(branch)) = steps.get(self.index) {
            let selected = self
                .route
                .take()
                .ok_or_else(|| anyhow!("XOR requires a route selection"))?;
            let path = branch
                .paths
                .get(&selected)
                .ok_or_else(|| anyhow!("unknown XOR path {selected:?}"))?;
            if !path.steps.is_empty() {
                let prefix = format!("xor:{}:{selected}/", self.index);
                let repeats = self
                    .progress
                    .repeats
                    .iter()
                    .filter_map(|(key, count)| {
                        key.strip_prefix(&prefix)
                            .map(|key| (key.to_owned(), *count))
                    })
                    .collect();
                self.child = Some(Box::new(NestedCursor::Xor {
                    selected,
                    cursor: Self {
                        progress: TransitionProgress {
                            direction: self.progress.direction.clone(),
                            repeats,
                            verdict: None,
                        },
                        ..Self::default()
                    },
                }));
                return Ok(());
            }
        }
        match finish_step(steps, self.index, &mut self.progress)? {
            FlowTransition::Next(index) => self.index = index,
            FlowTransition::Repeat(index) => {
                self.iteration = self
                    .iteration
                    .checked_add(1)
                    .ok_or_else(|| anyhow!("Flow iteration overflow"))?;
                self.index = index;
            }
            FlowTransition::Finished => self.index = steps.len(),
            FlowTransition::Blocked(reason) => return Err(anyhow!(reason)),
        }
        Ok(())
    }
}

fn selected_body<'a>(
    steps: &'a [ConcreteStep],
    index: usize,
    selected: &str,
) -> Result<&'a [ConcreteStep]> {
    let Some(ConcreteStep::Xor(branch)) = steps.get(index) else {
        return Err(anyhow!(
            "XOR child cursor has no captured parent at index {index}"
        ));
    };
    branch
        .paths
        .get(selected)
        .map(|path| path.steps.as_slice())
        .ok_or_else(|| anyhow!("XOR child cursor has no captured path {selected:?}"))
}

fn settle_step(
    items: &[ConcreteStep],
    cursor: &mut ExecutionCursor,
) -> Result<Option<FlowOutcome>> {
    match cursor.finish(items) {
        Ok(_) => Ok(None),
        Err(error) => Ok(Some(FlowOutcome::Blocked(error.to_string()))),
    }
}

pub fn current_skill(items: &[ConcreteStep], cursor: &ExecutionCursor) -> Option<ConcreteSkill> {
    let (items, cursor) = cursor.current_body(items);
    match items.get(cursor.index) {
        Some(ConcreteStep::Skill(skill)) => Some(skill.clone()),
        Some(ConcreteStep::Xor(branch)) => Some(branch.router_skill()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::execution::{
        current_skill, ExecutionContext, ExecutionCursor, FlowEngine, FlowOutcome, NestedCursor,
        SkillExecutor, SkillOutcome,
    };
    use crate::engine::flow::{
        ConcreteOp, ConcretePath, ConcreteSkill, ConcreteStep, ConcreteXor, Op, RepeatPolicy, Skill,
    };
    use crate::engine::transitions::{FlowDecision, FlowVerdict};
    use anyhow::{anyhow, Result};
    use async_trait::async_trait;
    use std::collections::{HashMap, VecDeque};
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    #[derive(Debug, Clone)]
    struct RecordingExecutor {
        calls: Arc<Mutex<Vec<String>>>,
        verdicts: Arc<Mutex<Vec<String>>>,
        wait_on: Option<String>,
        outcomes: Arc<Mutex<HashMap<String, VecDeque<SkillOutcome>>>>,
        contexts: Arc<Mutex<Vec<(String, ExecutionContext)>>>,
        checkpoints: Arc<Mutex<Vec<ExecutionCursor>>>,
        stop_after_checkpoint: Option<usize>,
    }

    impl RecordingExecutor {
        fn new(_repo_root: PathBuf) -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                verdicts: Arc::new(Mutex::new(Vec::new())),
                wait_on: None,
                outcomes: Arc::new(Mutex::new(HashMap::new())),
                contexts: Arc::new(Mutex::new(Vec::new())),
                checkpoints: Arc::new(Mutex::new(Vec::new())),
                stop_after_checkpoint: None,
            }
        }

        fn with_verdicts(self, verdicts: &[&str]) -> Self {
            *self.verdicts.lock().expect("verdict mutex") =
                verdicts.iter().map(|value| value.to_string()).collect();
            self
        }

        fn with_wait(mut self, skill: &str) -> Self {
            self.wait_on = Some(skill.to_string());
            self
        }

        fn with_outcomes(self, name: &str, outcomes: Vec<SkillOutcome>) -> Self {
            self.outcomes
                .lock()
                .unwrap()
                .insert(name.to_owned(), outcomes.into());
            self
        }

        fn stop_after(mut self, count: usize) -> Self {
            self.stop_after_checkpoint = Some(count);
            self
        }

        fn saved_cursor(&self) -> ExecutionCursor {
            let saved = self.checkpoints.lock().unwrap().last().unwrap().clone();
            serde_json::from_value(serde_json::to_value(saved).unwrap()).unwrap()
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().expect("call mutex").clone()
        }
    }

    #[async_trait]
    impl SkillExecutor for RecordingExecutor {
        async fn checkpoint(&self, cursor: &ExecutionCursor) -> Result<()> {
            let mut checkpoints = self.checkpoints.lock().unwrap();
            checkpoints.push(cursor.clone());
            if self.stop_after_checkpoint == Some(checkpoints.len()) {
                return Err(anyhow!("interrupted after checkpoint"));
            }
            Ok(())
        }

        async fn run_skill(
            &self,
            skill: &ConcreteSkill,
            ctx: ExecutionContext,
        ) -> Result<SkillOutcome> {
            self.calls
                .lock()
                .expect("call mutex")
                .push(skill.skill.name.clone());
            self.contexts
                .lock()
                .unwrap()
                .push((skill.skill.name.clone(), ctx));
            if let Some(outcome) = self
                .outcomes
                .lock()
                .unwrap()
                .get_mut(&skill.skill.name)
                .and_then(VecDeque::pop_front)
            {
                return Ok(outcome);
            }
            if skill.skill.name == "xor-route" {
                let mut routes = self.verdicts.lock().unwrap();
                return Ok(routes
                    .first()
                    .cloned()
                    .map_or(SkillOutcome::Blocked("missing route".into()), |_| {
                        SkillOutcome::Routed(routes.remove(0))
                    }));
            }
            if self.wait_on.as_deref() == Some(skill.skill.name.as_str()) {
                Ok(SkillOutcome::Waiting)
            } else {
                Ok(SkillOutcome::Completed { feedback: None })
            }
        }

        async fn run_op(&self, ops: &ConcreteOp, ctx: ExecutionContext) -> Result<()> {
            let name = format!("op:{}", ops.item.display_name());
            self.contexts.lock().unwrap().push((name.clone(), ctx));
            self.calls.lock().unwrap().push(name);
            Ok(())
        }
    }

    fn fixture_repo() -> std::io::Result<tempfile::TempDir> {
        let repo = tempdir()?;
        let skills = repo.path().join(".lf/skills");
        std::fs::create_dir_all(&skills)?;
        for name in [
            "work",
            "decide",
            "nested-work",
            "nested-review",
            "blocked-work",
            "selected-skill",
        ] {
            std::fs::write(
                skills.join(format!("{name}.md")),
                format!("Fixture skill {name}"),
            )?;
        }
        Ok(repo)
    }

    fn skill(name: &str) -> ConcreteSkill {
        ConcreteSkill {
            skill: Skill::named(name),
            policy: crate::engine::OccurrencePolicy::default(),
            flow_parents: vec!["test".to_string()],
        }
    }

    fn step(name: &str, edge: Option<&str>) -> ConcreteStep {
        let mut value = skill(name);
        value.policy.id = Some(name.to_owned());
        value.policy.repeat = edge.map(|from| RepeatPolicy {
            from: from.to_owned(),
        });
        ConcreteStep::Skill(value)
    }

    fn decision(decision: FlowDecision, summary: &str) -> SkillOutcome {
        SkillOutcome::Decided(FlowVerdict {
            decision,
            summary: summary.to_owned(),
        })
    }

    fn xor(flow: &str, repo: &Path) -> ConcreteStep {
        ConcreteStep::Xor(ConcreteXor {
            router: Skill::named("xor-route"),
            paths: HashMap::from([(
                "selected".to_owned(),
                ConcretePath {
                    steps: crate::engine::expand_flow(
                        &crate::engine::load_flow(flow, repo).unwrap(),
                        repo,
                    )
                    .unwrap(),
                    description: "selected path".to_owned(),
                },
            )]),
            flow_parents: vec!["test".to_owned()],
        })
    }

    #[tokio::test]
    async fn engine_runs_independent_loops_and_carries_direction_through_the_body() {
        let repo = fixture_repo().unwrap();
        let items = vec![
            step("init", None),
            step("a", None),
            step("compress", None),
            step("ra", Some("a")),
            step("middle", None),
            step("b", None),
            step("rb", Some("b")),
            step("final", None),
        ];
        let executor = RecordingExecutor::new(repo.path().to_owned())
            .with_outcomes(
                "ra",
                vec![
                    decision(FlowDecision::Iterate, "repair a"),
                    decision(FlowDecision::Advance, "a proven"),
                ],
            )
            .with_outcomes(
                "rb",
                vec![
                    decision(FlowDecision::Iterate, "repair b"),
                    decision(FlowDecision::Advance, "b proven"),
                ],
            );
        let mut cursor = ExecutionCursor::default();
        assert_eq!(
            FlowEngine::new(executor.clone())
                .run_with_cursor(&items, &mut cursor)
                .await
                .unwrap(),
            FlowOutcome::Completed
        );
        assert_eq!(
            executor.calls(),
            [
                "init", "a", "compress", "ra", "a", "compress", "ra", "middle", "b", "rb", "b",
                "rb", "final"
            ]
        );
        let contexts = executor.contexts.lock().unwrap();
        for index in [4, 5, 6] {
            assert_eq!(contexts[index].1.direction.as_deref(), Some("repair a"));
        }
        for index in [10, 11] {
            assert_eq!(contexts[index].1.direction.as_deref(), Some("repair b"));
        }
        for index in [0, 7, 8, 12] {
            assert_eq!(contexts[index].1.direction, None);
        }
        assert_eq!(
            cursor.progress.repeats,
            [("ra".to_owned(), 1), ("rb".to_owned(), 1)].into()
        );
        assert_eq!(cursor.iteration, 2);
        assert_eq!(cursor.index, items.len());
        assert_eq!(executor.saved_cursor(), cursor);
    }

    #[tokio::test]
    async fn engine_keeps_iterating_until_advance() {
        let repo = fixture_repo().unwrap();
        let items = vec![
            step("work", None),
            step("decide", Some("work")),
            step("final", None),
        ];
        let outcomes = (0..20)
            .map(|_| decision(FlowDecision::Iterate, "useful work remains"))
            .chain([decision(FlowDecision::Advance, "work demonstrated")])
            .collect();
        let executor =
            RecordingExecutor::new(repo.path().to_owned()).with_outcomes("decide", outcomes);
        let mut cursor = ExecutionCursor::default();
        assert_eq!(
            FlowEngine::new(executor.clone())
                .run_with_cursor(&items, &mut cursor)
                .await
                .unwrap(),
            FlowOutcome::Completed
        );
        assert_eq!(
            executor
                .calls()
                .iter()
                .filter(|name| *name == "work")
                .count(),
            21
        );
        assert_eq!(
            executor
                .calls()
                .iter()
                .filter(|name| *name == "final")
                .count(),
            1
        );
        assert_eq!(cursor.progress.repeats["decide"], 20);
        assert_eq!(cursor.index, items.len());
    }

    #[tokio::test]
    async fn overlapping_loops_revisit_work_until_their_decisions_advance() {
        let repo = fixture_repo().unwrap();
        let items = vec![
            step("start", None),
            step("middle", None),
            step("inner", Some("start")),
            step("outer", Some("middle")),
            step("final", None),
        ];
        let executor = RecordingExecutor::new(repo.path().to_owned())
            .with_outcomes(
                "inner",
                vec![
                    decision(FlowDecision::Iterate, "first pass"),
                    decision(FlowDecision::Advance, "proven"),
                    decision(FlowDecision::Iterate, "try again"),
                    decision(FlowDecision::Advance, "revised work proven"),
                ],
            )
            .with_outcomes(
                "outer",
                vec![
                    decision(FlowDecision::Iterate, "overlapping pass"),
                    decision(FlowDecision::Advance, "whole work proven"),
                ],
            );
        let mut cursor = ExecutionCursor::default();
        assert_eq!(
            FlowEngine::new(executor.clone())
                .run_with_cursor(&items, &mut cursor)
                .await
                .unwrap(),
            FlowOutcome::Completed
        );
        assert_eq!(
            executor.calls(),
            [
                "start", "middle", "inner", "start", "middle", "inner", "outer", "middle", "inner",
                "start", "middle", "inner", "outer", "final"
            ]
        );
        assert_eq!(cursor.index, items.len());
        assert_eq!(
            cursor.progress.repeats,
            [("inner".to_owned(), 2), ("outer".to_owned(), 1)].into()
        );
        assert!(cursor.progress.verdict.is_none());
        assert_eq!(executor.saved_cursor(), cursor);
    }

    #[tokio::test]
    async fn missing_empty_and_blocked_decisions_never_run_final_steps() {
        let repo = fixture_repo().unwrap();
        let items = vec![
            step("work", None),
            step("decide", Some("work")),
            step("final", None),
        ];
        for outcome in [
            SkillOutcome::Completed { feedback: None },
            SkillOutcome::Blocked("need a policy".to_owned()),
            decision(FlowDecision::Advance, " "),
        ] {
            let executor = RecordingExecutor::new(repo.path().to_owned())
                .with_outcomes("decide", vec![outcome]);
            let mut cursor = ExecutionCursor::default();
            assert!(matches!(
                FlowEngine::new(executor.clone())
                    .run_with_cursor(&items, &mut cursor)
                    .await
                    .unwrap(),
                FlowOutcome::Blocked(_)
            ));
            assert_eq!(executor.calls(), ["work", "decide"]);
            assert_eq!(cursor.index, 1);
            assert_eq!(cursor.iteration, 0);
            assert!(cursor.progress.repeats.is_empty());
            assert_eq!(executor.saved_cursor(), cursor);
        }
    }

    #[tokio::test]
    async fn saved_decision_recovers_once_without_rerunning_the_deciding_skill() {
        let repo = fixture_repo().unwrap();
        let items = vec![
            step("work", None),
            step("decide", Some("work")),
            step("final", None),
        ];
        let executor = RecordingExecutor::new(repo.path().to_owned())
            .with_outcomes("decide", vec![decision(FlowDecision::Iterate, "repair")])
            .stop_after(2);
        assert!(FlowEngine::new(executor.clone())
            .run(&items, 0)
            .await
            .unwrap_err()
            .to_string()
            .contains("interrupted"));
        let mut cursor = executor.saved_cursor();
        assert_eq!(cursor.index, 1);
        assert!(cursor.progress.verdict.is_some());
        let resumed = RecordingExecutor::new(repo.path().to_owned())
            .with_outcomes("decide", vec![decision(FlowDecision::Advance, "proven")]);
        assert_eq!(
            FlowEngine::new(resumed.clone())
                .run_with_cursor(&items, &mut cursor)
                .await
                .unwrap(),
            FlowOutcome::Completed
        );
        assert_eq!(resumed.calls(), ["work", "decide", "final"]);
        assert_eq!(cursor.progress.repeats["decide"], 1);
        assert!(cursor.progress.verdict.is_none());
        assert_eq!(
            FlowEngine::new(resumed.clone())
                .run_with_cursor(&items, &mut cursor)
                .await
                .unwrap(),
            FlowOutcome::Completed
        );
        assert_eq!(resumed.calls(), ["work", "decide", "final"]);
    }

    #[tokio::test]
    async fn nested_xor_checkpoints_root_and_recovers_a_pinned_pending_decision() {
        let repo = fixture_repo().unwrap();
        std::fs::create_dir_all(repo.path().join(".lf/flows")).unwrap();
        std::fs::write(
            repo.path().join(".lf/flows/outer.yaml"),
            "- xor:\n    paths:\n      selected:\n        flow: inner\n        description: selected path\n",
        )
        .unwrap();
        std::fs::write(repo.path().join(".lf/flows/inner.yaml"), "- step:\n    name: work\n    id: work\n- step:\n    name: decide\n    id: decide\n    repeat:\n      from: work\n").unwrap();
        let items = vec![
            step("prefix", None),
            xor("outer", repo.path()),
            step("suffix", None),
        ];
        let executor = RecordingExecutor::new(repo.path().to_owned())
            .with_verdicts(&["selected", "selected"])
            .with_outcomes(
                "decide",
                vec![decision(FlowDecision::Iterate, "repair nested")],
            )
            .stop_after(7);
        assert!(FlowEngine::new(executor.clone())
            .run(&items, 0)
            .await
            .unwrap_err()
            .to_string()
            .contains("interrupted"));
        assert_eq!(
            executor.calls(),
            ["prefix", "xor-route", "xor-route", "work", "decide"]
        );
        let saved = executor.checkpoints.lock().unwrap().clone();
        assert_eq!(
            saved.iter().map(|cursor| cursor.index).collect::<Vec<_>>(),
            [1, 1, 1, 1, 1, 1, 1]
        );
        let NestedCursor::Xor { cursor: outer, .. } = saved[4].child.as_deref().unwrap();
        let NestedCursor::Xor { cursor: inner, .. } = outer.child.as_deref().unwrap();
        assert_eq!(inner.index, 0);
        let NestedCursor::Xor { cursor: outer, .. } = saved[5].child.as_deref().unwrap();
        let NestedCursor::Xor { cursor: inner, .. } = outer.child.as_deref().unwrap();
        assert_eq!(inner.index, 1);
        assert!(inner.progress.verdict.is_none());
        let mut cursor = executor.saved_cursor();
        let captured: Vec<ConcreteStep> =
            serde_json::from_value(serde_json::to_value(&items).unwrap()).unwrap();
        let mut legacy = serde_json::to_value(&cursor).unwrap();
        let mut child = &mut legacy["child"];
        for _ in 0..2 {
            assert!(child.get("steps").is_none());
            // Old cursor copies are redundant only because the parent definition
            // captures every path. Even a differing copy cannot replace it.
            child["steps"] = serde_json::to_value(vec![step("obsolete-copy", None)]).unwrap();
            child = &mut child["cursor"]["child"];
        }
        cursor = serde_json::from_value(legacy).unwrap();
        assert_eq!(cursor, executor.saved_cursor());
        assert_eq!(
            serde_json::to_value(&cursor).unwrap(),
            serde_json::to_value(executor.saved_cursor()).unwrap()
        );
        std::fs::remove_dir_all(repo.path().join(".lf")).unwrap();
        let items = captured;
        assert_eq!(current_skill(&items, &cursor).unwrap().skill.name, "decide");
        let resumed = RecordingExecutor::new(repo.path().to_owned())
            .with_outcomes("decide", vec![decision(FlowDecision::Advance, "proven")]);
        assert_eq!(
            FlowEngine::new(resumed.clone())
                .run_with_cursor(&items, &mut cursor)
                .await
                .unwrap(),
            FlowOutcome::Completed
        );
        assert_eq!(resumed.calls(), ["work", "decide", "suffix"]);
        let contexts = resumed.contexts.lock().unwrap();
        assert_eq!(contexts[0].1.direction.as_deref(), Some("repair nested"));
        assert_eq!(contexts[1].1.direction.as_deref(), Some("repair nested"));
        assert_eq!(contexts[2].1.direction, None);
        assert_eq!(cursor.index, 3);
        assert!(cursor.child.is_none());
    }

    #[tokio::test]
    async fn loop_direction_reaches_ops_and_xor_children() {
        let repo = fixture_repo().unwrap();
        std::fs::create_dir_all(repo.path().join(".lf/flows")).unwrap();
        std::fs::write(
            repo.path().join(".lf/flows/branch.yaml"),
            "- nested-work\n- nested-review\n",
        )
        .unwrap();
        let items = vec![
            step("work", None),
            ConcreteStep::Op(ConcreteOp {
                item: Op {
                    command: "check".to_owned(),
                    args: vec![],
                },
                flow_parents: vec![],
            }),
            xor("branch", repo.path()),
            step("decide", Some("work")),
            step("final", None),
        ];
        let executor = RecordingExecutor::new(repo.path().to_owned())
            .with_verdicts(&["selected", "selected"])
            .with_outcomes(
                "decide",
                vec![
                    decision(FlowDecision::Iterate, "revise all"),
                    decision(FlowDecision::Advance, "proven"),
                ],
            );
        assert_eq!(
            FlowEngine::new(executor.clone())
                .run(&items, 0)
                .await
                .unwrap(),
            FlowOutcome::Completed
        );
        assert_eq!(
            executor.calls(),
            [
                "work",
                "op:check",
                "xor-route",
                "nested-work",
                "nested-review",
                "decide",
                "work",
                "op:check",
                "xor-route",
                "nested-work",
                "nested-review",
                "decide",
                "final"
            ]
        );
        let contexts = executor.contexts.lock().unwrap();
        for index in 6..12 {
            assert_eq!(contexts[index].1.direction.as_deref(), Some("revise all"));
        }
        assert_eq!(contexts[12].1.direction, None);
    }

    #[tokio::test]
    async fn empty_xor_path_is_checkpointed_before_the_suffix_runs() {
        let repo = fixture_repo().unwrap();
        let items = vec![
            ConcreteStep::Xor(ConcreteXor {
                router: Skill::named("xor-route"),
                flow_parents: vec![],
                paths: HashMap::from([(
                    "silence".to_owned(),
                    ConcretePath {
                        steps: vec![],
                        description: "nothing to do".to_owned(),
                    },
                )]),
            }),
            step("suffix", None),
        ];
        let executor = RecordingExecutor::new(repo.path().to_owned())
            .with_verdicts(&["silence"])
            .stop_after(1);
        assert!(FlowEngine::new(executor.clone())
            .run(&items, 0)
            .await
            .is_err());
        assert_eq!(executor.calls(), ["xor-route"]);
        let mut cursor = executor.saved_cursor();
        assert_eq!(cursor.route.as_deref(), Some("silence"));
        assert!(cursor.child.is_none());
        let resumed = RecordingExecutor::new(repo.path().to_owned());
        assert_eq!(
            FlowEngine::new(resumed.clone())
                .run_with_cursor(&items, &mut cursor)
                .await
                .unwrap(),
            FlowOutcome::Completed
        );
        assert_eq!(resumed.calls(), ["suffix"]);
    }

    #[tokio::test]
    async fn nested_blocker_retains_the_selected_path_and_stops_the_suffix() {
        let repo = fixture_repo().unwrap();
        std::fs::create_dir_all(repo.path().join(".lf/flows")).unwrap();
        std::fs::write(
            repo.path().join(".lf/flows/branch.yaml"),
            "- blocked-work\n",
        )
        .unwrap();
        let items = vec![xor("branch", repo.path()), step("suffix", None)];
        let executor = RecordingExecutor::new(repo.path().to_owned())
            .with_verdicts(&["selected"])
            .with_outcomes(
                "blocked-work",
                vec![SkillOutcome::Blocked("missing policy".to_owned())],
            );
        let mut cursor = ExecutionCursor::default();
        assert_eq!(
            FlowEngine::new(executor.clone())
                .run_with_cursor(&items, &mut cursor)
                .await
                .unwrap(),
            FlowOutcome::Blocked("missing policy".to_owned())
        );
        assert_eq!(executor.calls(), ["xor-route", "blocked-work"]);
        assert_eq!(cursor.index, 0);
        assert_eq!(
            current_skill(&items, &cursor).unwrap().skill.name,
            "blocked-work"
        );
        assert_eq!(executor.saved_cursor(), cursor);
    }

    #[tokio::test]
    async fn engine_runs_selected_xor_path() {
        let repo = fixture_repo().expect("tempdir");
        std::fs::create_dir_all(repo.path().join(".lf/flows")).expect("flows dir");
        std::fs::write(
            repo.path().join(".lf/flows/branch.yaml"),
            "- selected-skill\n- op: next\n",
        )
        .expect("write flow");

        let executor = RecordingExecutor::new(repo.path().to_path_buf()).with_verdicts(&["ship"]);
        let engine = FlowEngine::new(executor.clone());
        let items = vec![ConcreteStep::Xor(ConcreteXor {
            router: Skill::named("xor-route"),
            paths: HashMap::from([(
                "ship".to_string(),
                ConcretePath {
                    steps: crate::engine::expand_flow(
                        &crate::engine::load_flow("branch", repo.path()).unwrap(),
                        repo.path(),
                    )
                    .unwrap(),
                    description: "ship it".to_string(),
                },
            )]),
            flow_parents: vec!["test".to_string()],
        })];

        let outcome = engine.run(&items, 0).await.expect("engine run");
        assert_eq!(outcome, FlowOutcome::Completed);
        assert_eq!(
            executor.calls(),
            vec![
                "xor-route".to_string(),
                "selected-skill".to_string(),
                "op:next".to_string()
            ]
        );
    }

    #[tokio::test]
    async fn engine_stops_when_executor_waits() {
        let repo = fixture_repo().expect("tempdir");
        let executor = RecordingExecutor::new(repo.path().to_path_buf()).with_wait("design");
        let engine = FlowEngine::new(executor.clone());
        let items = vec![
            ConcreteStep::Skill(skill("design")),
            ConcreteStep::Skill(skill("implement")),
        ];

        let outcome = engine.run(&items, 0).await.expect("engine run");
        assert_eq!(outcome, FlowOutcome::Waiting);
        assert_eq!(executor.calls(), vec!["design".to_string()]);
    }

    #[tokio::test]
    async fn engine_resumes_nested_xor_after_waiting_skill() {
        let repo = fixture_repo().expect("tempdir");
        std::fs::create_dir_all(repo.path().join(".lf/flows")).expect("flows dir");
        std::fs::write(
            repo.path().join(".lf/flows/branch.yaml"),
            "- design\n- implement\n",
        )
        .expect("write flow");

        let items = vec![ConcreteStep::Xor(ConcreteXor {
            router: Skill::named("xor-route"),
            paths: HashMap::from([(
                "ship".to_string(),
                ConcretePath {
                    steps: crate::engine::expand_flow(
                        &crate::engine::load_flow("branch", repo.path()).unwrap(),
                        repo.path(),
                    )
                    .unwrap(),
                    description: "ship it".to_string(),
                },
            )]),
            flow_parents: vec!["test".to_string()],
        })];

        let mut cursor = ExecutionCursor::default();
        let executor = RecordingExecutor::new(repo.path().to_path_buf())
            .with_verdicts(&["ship"])
            .with_wait("design");
        let outcome = FlowEngine::new(executor.clone())
            .run_with_cursor(&items, &mut cursor)
            .await
            .expect("engine run");
        assert_eq!(outcome, FlowOutcome::Waiting);
        assert_eq!(
            executor.calls(),
            vec!["xor-route".to_string(), "design".to_string()]
        );

        cursor = executor.saved_cursor();
        std::fs::remove_file(repo.path().join(".lf/flows/branch.yaml")).unwrap();
        cursor.finish(&items).expect("advance cursor");
        let resumed = current_skill(&items, &cursor).expect("skill should remain");
        assert_eq!(resumed.skill.name, "implement");

        let executor = RecordingExecutor::new(repo.path().to_path_buf()).with_verdicts(&["ship"]);
        let outcome = FlowEngine::new(executor.clone())
            .run_with_cursor(&items, &mut cursor)
            .await
            .expect("resume engine");
        assert_eq!(outcome, FlowOutcome::Completed);
        assert_eq!(executor.calls(), vec!["implement".to_string()]);
    }

    #[tokio::test]
    async fn completed_saved_child_returns_without_replaying_work() {
        let body = vec![step("already-done", None)];
        let items = vec![
            ConcreteStep::Xor(ConcreteXor {
                router: Skill::named("xor-route"),
                paths: [(
                    "saved".into(),
                    ConcretePath {
                        description: "saved path".into(),
                        steps: body,
                    },
                )]
                .into(),
                flow_parents: vec![],
            }),
            step("suffix", None),
        ];
        let mut cursor = ExecutionCursor {
            child: Some(Box::new(NestedCursor::Xor {
                selected: "saved".into(),
                cursor: ExecutionCursor {
                    index: 1,
                    ..Default::default()
                },
            })),
            ..Default::default()
        };
        let executor = RecordingExecutor::new(PathBuf::new());
        assert_eq!(
            FlowEngine::new(executor.clone())
                .run_with_cursor(&items, &mut cursor)
                .await
                .unwrap(),
            FlowOutcome::Completed
        );
        assert_eq!(executor.calls(), ["suffix"]);
        assert!(cursor.child.is_none());
    }

    #[tokio::test]
    async fn saved_xor_selection_requires_its_captured_parent_and_path() {
        let legacy = serde_json::json!({
            "index": 0,
            "child": {
                "type": "xor",
                "selected": "saved",
                "steps": [step("orphaned-copy", None)],
                "cursor": { "index": 0 }
            }
        });
        let saved: ExecutionCursor = serde_json::from_value(legacy).unwrap();
        let branch = |name: &str, steps| {
            ConcreteStep::Xor(ConcreteXor {
                router: Skill::named("xor-route"),
                paths: [(
                    name.into(),
                    ConcretePath {
                        description: name.into(),
                        steps,
                    },
                )]
                .into(),
                flow_parents: vec![],
            })
        };
        for items in [
            vec![step("different-parent", None)],
            vec![branch("different-path", vec![step("wrong-work", None)])],
        ] {
            let mut cursor = saved.clone();
            assert!(current_skill(&items, &cursor).is_none());
            assert!(cursor
                .finish(&items)
                .unwrap_err()
                .to_string()
                .contains("captured"));
            let executor = RecordingExecutor::new(PathBuf::new());
            assert!(matches!(
                FlowEngine::new(executor.clone())
                    .run_with_cursor(&items, &mut cursor)
                    .await
                    .unwrap(),
                FlowOutcome::Blocked(_)
            ));
            assert!(executor.calls().is_empty());
            assert_eq!(cursor, saved);
        }
        let items = vec![branch("saved", vec![step("captured-work", None)])];
        let mut cursor = saved;
        cursor.leaf_mut().index = 2;
        let before = cursor.clone();
        assert!(current_skill(&items, &cursor).is_none());
        assert!(cursor
            .finish(&items)
            .unwrap_err()
            .to_string()
            .contains("exceeds"));
        assert_eq!(cursor, before);
    }

    #[tokio::test]
    async fn engine_runs_ops_items() {
        let repo = fixture_repo().expect("tempdir");
        let executor = RecordingExecutor::new(repo.path().to_path_buf());
        let engine = FlowEngine::new(executor.clone());
        let items = vec![ConcreteStep::Op(ConcreteOp {
            item: Op {
                command: "sync".to_string(),
                args: vec!["--fast".to_string()],
            },
            flow_parents: vec!["test".to_string()],
        })];

        let outcome = engine.run(&items, 0).await.expect("engine run");
        assert_eq!(outcome, FlowOutcome::Completed);
        assert_eq!(executor.calls(), vec!["op:sync --fast".to_string()]);
    }
}
