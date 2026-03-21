use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures_util::future::BoxFuture;
use futures_util::FutureExt;

use crate::engine::flow::{
    build_xor_routing_suffix, load_step, load_xor_path_items, ConcreteAnd, ConcreteItem,
    ConcreteLoop, ConcreteOp, ConcreteStep, ConcreteXor, Step,
};

pub const TEMP_XOR_ROUTE_STEP_NAME: &str = "xor-route";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlowProgress {
    pub index: usize,
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionStep {
    pub step: ConcreteStep,
    pub invoke_as: String,
    pub display_name: String,
    pub temporary_content: Option<String>,
}

impl ExecutionStep {
    pub fn regular(step: ConcreteStep) -> Self {
        let name = step.step.name.clone();
        Self {
            step,
            invoke_as: name.clone(),
            display_name: name,
            temporary_content: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionContext {
    pub progress: Option<FlowProgress>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    Completed,
    Waiting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowOutcome {
    Completed,
    Waiting,
}

#[async_trait]
pub trait StepExecutor: Send + Sync {
    fn repo_root(&self) -> &Path;

    async fn before_top_level_item(
        &self,
        _index: usize,
        _total: usize,
        _item: &ConcreteItem,
    ) -> Result<()> {
        Ok(())
    }

    async fn after_top_level_item(
        &self,
        _index: usize,
        _total: usize,
        _item: &ConcreteItem,
    ) -> Result<()> {
        Ok(())
    }

    async fn run_step(&self, step: &ExecutionStep, ctx: ExecutionContext) -> Result<StepOutcome>;

    async fn run_op(&self, ops: &ConcreteOp, ctx: ExecutionContext) -> Result<()>;

    async fn run_and(&self, fork: &ConcreteAnd, ctx: ExecutionContext) -> Result<()>;

    async fn read_xor_verdict(&self, branch: &ConcreteXor) -> Result<String>;
}

#[derive(Debug, Clone)]
pub struct FlowEngine<E> {
    executor: E,
}

impl<E> FlowEngine<E> {
    pub fn new(executor: E) -> Self {
        Self { executor }
    }

    pub fn executor(&self) -> &E {
        &self.executor
    }
}

impl<E: StepExecutor> FlowEngine<E> {
    pub async fn run(&self, items: &[ConcreteItem], start_index: usize) -> Result<FlowOutcome> {
        let total = items.len();
        for index in start_index..total {
            let item = items
                .get(index)
                .expect("top-level flow item index should stay within bounds");
            self.executor
                .before_top_level_item(index, total, item)
                .await?;
            let outcome = self
                .run_item(
                    item,
                    ExecutionContext {
                        progress: Some(FlowProgress { index, total }),
                    },
                )
                .await?;
            if outcome == FlowOutcome::Waiting {
                return Ok(FlowOutcome::Waiting);
            }
            self.executor
                .after_top_level_item(index, total, item)
                .await?;
        }
        Ok(FlowOutcome::Completed)
    }

    fn run_items_inline<'a>(
        &'a self,
        items: &'a [ConcreteItem],
    ) -> BoxFuture<'a, Result<FlowOutcome>> {
        async move {
            for item in items {
                let outcome = self
                    .run_item(item, ExecutionContext { progress: None })
                    .await?;
                if outcome == FlowOutcome::Waiting {
                    return Ok(FlowOutcome::Waiting);
                }
            }
            Ok(FlowOutcome::Completed)
        }
        .boxed()
    }

    fn run_item<'a>(
        &'a self,
        item: &'a ConcreteItem,
        ctx: ExecutionContext,
    ) -> BoxFuture<'a, Result<FlowOutcome>> {
        async move {
            match item {
                ConcreteItem::Step(step) => self.run_concrete_step(step, ctx).await,
                ConcreteItem::Op(ops) => {
                    self.executor.run_op(ops, ctx).await?;
                    Ok(FlowOutcome::Completed)
                }
                ConcreteItem::And(fork) => {
                    self.executor.run_and(fork, ctx).await?;
                    Ok(FlowOutcome::Completed)
                }
                ConcreteItem::Xor(branch) => {
                    self.run_xor(branch, ctx).await.map(|(outcome, _)| outcome)
                }
                ConcreteItem::Or(_) => Err(anyhow!(
                    "or (multi-select) execution is not yet implemented"
                )),
                ConcreteItem::Loop(body) => self.run_loop(body).await,
            }
        }
        .boxed()
    }

    async fn run_concrete_step(
        &self,
        step: &ConcreteStep,
        ctx: ExecutionContext,
    ) -> Result<FlowOutcome> {
        let outcome = self
            .executor
            .run_step(&ExecutionStep::regular(step.clone()), ctx)
            .await?;
        Ok(match outcome {
            StepOutcome::Completed => FlowOutcome::Completed,
            StepOutcome::Waiting => FlowOutcome::Waiting,
        })
    }

    fn run_xor<'a>(
        &'a self,
        branch: &'a ConcreteXor,
        ctx: ExecutionContext,
    ) -> BoxFuture<'a, Result<(FlowOutcome, String)>> {
        async move {
            let router_step = self.build_router_step(branch)?;
            let outcome = self.executor.run_step(&router_step, ctx).await?;
            if outcome == StepOutcome::Waiting {
                return Ok((FlowOutcome::Waiting, String::new()));
            }

            let selected = self.executor.read_xor_verdict(branch).await?;
            let xor_path = branch
                .paths
                .get(&selected)
                .ok_or_else(|| anyhow!("selected xor path '{selected}' not found"))?;
            let sub_items = load_xor_path_items(xor_path, self.executor.repo_root())?;
            Ok((self.run_items_inline(&sub_items).await?, selected))
        }
        .boxed()
    }

    fn run_loop<'a>(&'a self, body: &'a ConcreteLoop) -> BoxFuture<'a, Result<FlowOutcome>> {
        async move {
            loop {
                let outcome = self.run_items_inline(&body.steps).await?;
                if outcome == FlowOutcome::Waiting {
                    return Ok(FlowOutcome::Waiting);
                }

                let (outcome, selected) = self
                    .run_xor(&body.exit, ExecutionContext { progress: None })
                    .await?;
                if outcome == FlowOutcome::Waiting {
                    return Ok(FlowOutcome::Waiting);
                }

                if selected == "done" {
                    return Ok(FlowOutcome::Completed);
                }
            }
        }
        .boxed()
    }

    fn build_router_step(&self, branch: &ConcreteXor) -> Result<ExecutionStep> {
        let display_name = branch.router.as_deref().unwrap_or(TEMP_XOR_ROUTE_STEP_NAME);
        let routing_suffix = build_xor_routing_suffix(branch);
        let prompt = if let Some(router_name) = branch.router.as_deref() {
            let router = load_step(router_name, self.executor.repo_root())?;
            let base = router.content.as_deref().unwrap_or("");
            format!("{base}\n\n{routing_suffix}")
        } else {
            format!(
                "---\nagent: claude:sonnet\n---\n\
                 Previous steps have analyzed the current state and written their findings to scratch/.\n\
                 Read scratch/ to understand what's been decided, then choose the right path forward.\n\n\
                 {routing_suffix}"
            )
        };

        Ok(ExecutionStep {
            step: ConcreteStep {
                step: Step {
                    name: display_name.to_string(),
                    agent: None,
                    default_agent: Some("claude:sonnet".to_string()),
                    directions: Vec::new(),
                    action_style: None,
                    interactive: Some(false),
                    content: Some(prompt.clone()),
                    fast_path: None,
                },
                flow_parents: branch.flow_parents.clone(),
            },
            invoke_as: TEMP_XOR_ROUTE_STEP_NAME.to_string(),
            display_name: display_name.to_string(),
            temporary_content: Some(prompt),
        })
    }
}

pub fn xor_verdict_path(repo_root: &Path) -> PathBuf {
    repo_root.join("scratch/route-xor.md")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::flow::{ConcreteOp, Op, Step, XorPath};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    #[derive(Debug, Clone)]
    struct RecordingExecutor {
        repo_root: PathBuf,
        calls: Arc<Mutex<Vec<String>>>,
        verdicts: Arc<Mutex<Vec<String>>>,
        wait_on: Option<String>,
    }

    impl RecordingExecutor {
        fn new(repo_root: PathBuf) -> Self {
            Self {
                repo_root,
                calls: Arc::new(Mutex::new(Vec::new())),
                verdicts: Arc::new(Mutex::new(Vec::new())),
                wait_on: None,
            }
        }

        fn with_verdicts(self, verdicts: &[&str]) -> Self {
            *self.verdicts.lock().expect("verdict mutex") =
                verdicts.iter().map(|value| value.to_string()).collect();
            self
        }

        fn with_wait(mut self, step: &str) -> Self {
            self.wait_on = Some(step.to_string());
            self
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().expect("call mutex").clone()
        }
    }

    #[async_trait]
    impl StepExecutor for RecordingExecutor {
        fn repo_root(&self) -> &Path {
            &self.repo_root
        }

        async fn run_step(
            &self,
            step: &ExecutionStep,
            _ctx: ExecutionContext,
        ) -> Result<StepOutcome> {
            self.calls
                .lock()
                .expect("call mutex")
                .push(step.display_name.clone());
            if self.wait_on.as_deref() == Some(step.display_name.as_str()) {
                Ok(StepOutcome::Waiting)
            } else {
                Ok(StepOutcome::Completed)
            }
        }

        async fn run_op(&self, ops: &ConcreteOp, _ctx: ExecutionContext) -> Result<()> {
            self.calls
                .lock()
                .expect("call mutex")
                .push(format!("op:{}", ops.item.display_name()));
            Ok(())
        }

        async fn run_and(&self, _fork: &ConcreteAnd, _ctx: ExecutionContext) -> Result<()> {
            self.calls
                .lock()
                .expect("call mutex")
                .push("and".to_string());
            Ok(())
        }

        async fn read_xor_verdict(&self, _branch: &ConcreteXor) -> Result<String> {
            let mut verdicts = self.verdicts.lock().expect("verdict mutex");
            if verdicts.is_empty() {
                return Err(anyhow!("missing verdict"));
            }
            Ok(verdicts.remove(0))
        }
    }

    fn step(name: &str) -> ConcreteStep {
        ConcreteStep {
            step: Step::named(name),
            flow_parents: vec!["test".to_string()],
        }
    }

    #[tokio::test]
    async fn engine_runs_selected_xor_path() {
        let repo = tempdir().expect("tempdir");
        std::fs::create_dir_all(repo.path().join(".lf/flows")).expect("flows dir");
        std::fs::write(
            repo.path().join(".lf/flows/branch.yaml"),
            "- selected-step\n- op: next\n",
        )
        .expect("write flow");

        let executor = RecordingExecutor::new(repo.path().to_path_buf()).with_verdicts(&["ship"]);
        let engine = FlowEngine::new(executor.clone());
        let items = vec![ConcreteItem::Xor(ConcreteXor {
            router: None,
            paths: HashMap::from([(
                "ship".to_string(),
                XorPath {
                    flow: Some("branch".to_string()),
                    step: None,
                    steps: Vec::new(),
                    description: "ship it".to_string(),
                    direction: Vec::new(),
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
                "selected-step".to_string(),
                "op:next".to_string()
            ]
        );
    }

    #[tokio::test]
    async fn engine_retries_loop_until_done() {
        let repo = tempdir().expect("tempdir");
        let executor =
            RecordingExecutor::new(repo.path().to_path_buf()).with_verdicts(&["retry", "done"]);
        let engine = FlowEngine::new(executor.clone());
        let items = vec![ConcreteItem::Loop(ConcreteLoop {
            steps: vec![ConcreteItem::Step(step("body-step"))],
            exit: ConcreteXor {
                router: None,
                paths: HashMap::from([
                    (
                        "retry".to_string(),
                        XorPath {
                            flow: None,
                            step: None,
                            steps: Vec::new(),
                            description: "retry".to_string(),
                            direction: Vec::new(),
                        },
                    ),
                    (
                        "done".to_string(),
                        XorPath {
                            flow: None,
                            step: None,
                            steps: Vec::new(),
                            description: "done".to_string(),
                            direction: Vec::new(),
                        },
                    ),
                ]),
                flow_parents: vec!["test".to_string(), "loop".to_string()],
            },
            flow_parents: vec!["test".to_string()],
        })];

        let outcome = engine.run(&items, 0).await.expect("engine run");
        assert_eq!(outcome, FlowOutcome::Completed);
        assert_eq!(
            executor.calls(),
            vec![
                "body-step".to_string(),
                "xor-route".to_string(),
                "body-step".to_string(),
                "xor-route".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn engine_stops_when_executor_waits() {
        let repo = tempdir().expect("tempdir");
        let executor = RecordingExecutor::new(repo.path().to_path_buf()).with_wait("design");
        let engine = FlowEngine::new(executor.clone());
        let items = vec![
            ConcreteItem::Step(step("design")),
            ConcreteItem::Step(step("implement")),
        ];

        let outcome = engine.run(&items, 0).await.expect("engine run");
        assert_eq!(outcome, FlowOutcome::Waiting);
        assert_eq!(executor.calls(), vec!["design".to_string()]);
    }

    #[tokio::test]
    async fn engine_runs_ops_items() {
        let repo = tempdir().expect("tempdir");
        let executor = RecordingExecutor::new(repo.path().to_path_buf());
        let engine = FlowEngine::new(executor.clone());
        let items = vec![ConcreteItem::Op(ConcreteOp {
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
