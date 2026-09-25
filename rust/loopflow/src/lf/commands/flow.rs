use crate::engine::transitions::{FlowDecision, FlowVerdict};
use crate::engine::{
    expand_flow, human_occurrence_ids, ConcreteSkill, ConcreteStep, ConcreteXor, ExecutionContext,
    Flow, FlowEngine, FlowOutcome, SkillExecutor, SkillOutcome, StepProgress,
};
use crate::journal::{self, LfEventFields, LfEventType, LfNode};
use crate::lf::output::Colors;
use crate::lf::Cli;
use crate::ops::{commit_workflow, flow_run, CommitOptions, NullProgress};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};

/// Run a flow: print pipeline header, then execute each skill sequentially.
pub fn run(flow: &Flow, message: Option<&str>, cli: &Cli, repo: &Path) -> Result<()> {
    let items = expand_flow(flow, repo)?;
    print_pipeline_header(&flow.name, &items, repo)?;
    execute(&flow.name, &items, None, message, cli, repo)
}

pub fn run_bound(
    flow: &Flow,
    message: Option<&str>,
    cli: &Cli,
    binding: &crate::ops::WorkBinding,
) -> Result<()> {
    let message = crate::lf::commands::run::bound_message(binding, message);
    run(flow, Some(&message), cli, &binding.cwd)
}

pub fn show(name: &str, repo: &Path) -> Result<()> {
    let flow = crate::engine::load_flow(name, repo)?;
    let items = expand_flow(&flow, repo)?;
    for line in render_pipeline_lines(&items, repo)? {
        println!("{line}");
    }
    Ok(())
}

pub fn validate(name: &str, repo: &Path) -> Result<()> {
    let flow = crate::engine::load_flow(name, repo)?;
    let mut human = human_occurrence_ids(&flow, repo)?;
    human.sort();
    if human.is_empty() {
        println!("{}: valid (no review steps)", flow.name);
    } else {
        println!("{}: valid (review steps: {})", flow.name, human.join(", "));
    }
    Ok(())
}

/// Run exactly one expanded top-level step. The resident owns the cursor and
/// invokes this hidden primitive once per body, so a body boundary maps to
/// a product step instead of an entire flow.
pub fn run_step(flow: &str, index: usize, message: &str, cli: &Cli, repo: &Path) -> Result<()> {
    let definition = crate::engine::load_flow(flow, repo)?;
    let items = expand_flow(&definition, repo)?;
    let item = items
        .get(index)
        .cloned()
        .ok_or_else(|| anyhow!("flow '{flow}' has no step at index {index}"))?;
    execute(
        &definition.name,
        std::slice::from_ref(&item),
        Some(index as u32),
        Some(message),
        cli,
        repo,
    )
}

/// Execute expanded steps on a fresh runtime, bracketed by flow journal events.
/// `index` names the single step when the caller is running one body's worth.
fn execute(
    flow_name: &str,
    items: &[ConcreteStep],
    index: Option<u32>,
    message: Option<&str>,
    cli: &Cli,
    repo: &Path,
) -> Result<()> {
    let fields = |extra: LfEventFields| LfEventFields {
        flow: Some(flow_name.to_string()),
        index,
        ..extra
    };
    journal::emit(
        repo,
        LfNode::Flow,
        LfEventType::Started,
        fields(LfEventFields::default()),
    );
    let _flow_env = EnvVarGuard::set("LOOPFLOW_FLOW_NAME", flow_name);
    let record = flow_run::create(flow_name, items, repo, message, cli)?;
    eprintln!(
        "Flow invocation {} — resume with lf flow resume {}",
        record.id, record.id
    );
    let result = drive_saved(&record.id, cli);
    match &result {
        Ok(outcome) => journal::emit(
            repo,
            LfNode::Flow,
            if *outcome == FlowOutcome::Completed {
                LfEventType::Completed
            } else {
                LfEventType::Escalated
            },
            fields(LfEventFields::default()),
        ),
        Err(err) => journal::emit(
            repo,
            LfNode::Flow,
            LfEventType::Errored,
            fields(LfEventFields {
                error: Some(err.to_string()),
                ..LfEventFields::default()
            }),
        ),
    }
    report_outcome(result?)
}

fn report_outcome(outcome: FlowOutcome) -> Result<()> {
    match outcome {
        FlowOutcome::Completed => Ok(()),
        FlowOutcome::Waiting => {
            anyhow::bail!("Flow is waiting for human input; open its Flow Session")
        }
        FlowOutcome::Blocked(reason) => anyhow::bail!("Flow is blocked: {reason}"),
    }
}

pub fn control(command: &str, args: &[String], cli: &Cli, repo: &Path) -> Result<()> {
    match command {
        "decide" => {
            let decision = match args.first().map(String::as_str) {
                Some("advance") => FlowDecision::Advance,
                Some("iterate") => FlowDecision::Iterate,
                _ => anyhow::bail!("usage: lf flow decide advance|iterate SUMMARY"),
            };
            let summary = args[1..].join(" ");
            anyhow::ensure!(
                !summary.trim().is_empty(),
                "decision requires evidence or direction"
            );
            if let Some(token) = flow_run::token()? {
                let run = active_run_id()?;
                flow_run::record_decision(&token, &run, &FlowVerdict { decision, summary })?;
            } else {
                crate::ops::task::task_verdict(repo, decision, &summary)?;
            }
            println!("Decision recorded; it takes effect when this Run finishes successfully.");
            Ok(())
        }
        "route" => {
            anyhow::ensure!(args.len() == 1, "usage: lf flow route PATH");
            let run = active_run_id()?;
            if let Some(token) = flow_run::token()? {
                flow_run::record_route(&token, &run, &args[0])?;
            } else {
                let runtime = tokio::runtime::Runtime::new()?;
                runtime.block_on(async {
                    let store = std::sync::Arc::new(
                        crate::store::open_existing_store()
                            .await
                            .ok_or_else(|| anyhow!("no Loopflow registry"))?,
                    );
                    let task = crate::ops::task::task_for_checkout(&store, repo)
                        .await?
                        .ok_or_else(|| anyhow!("no Task in this checkout"))?;
                    store.record_flow_route(&task.id, &run, &args[0]).await?;
                    Ok::<_, anyhow::Error>(())
                })?;
            }
            println!("Route recorded; it takes effect when this Run finishes successfully.");
            Ok(())
        }
        "blocked" => {
            let reason = args.join(" ");
            anyhow::ensure!(!reason.trim().is_empty(), "usage: lf flow blocked REASON");
            let runtime = tokio::runtime::Runtime::new()?;
            runtime.block_on(async {
                let store = std::sync::Arc::new(crate::store::open_existing_store().await
                    .ok_or_else(|| anyhow!("no Loopflow registry on this Home"))?);
                let run_id = active_run_id()?;
                let key = if let Some(token) = flow_run::token()? {
                    flow_run::require_active(&token, &run_id)?
                } else {
                    let task = crate::ops::task::task_for_checkout(&store, repo).await?
                        .ok_or_else(|| anyhow!("no current Flow decision"))?;
                    let position = store.flow_position(&task.id).await?
                        .ok_or_else(|| anyhow!("Task has no current Flow"))?;
                    anyhow::ensure!(position.claim.as_ref().and_then(|c| c.worker_run_id.as_ref()) == Some(&run_id)
                        && !position.is_human(), "this Run does not own the Task decision");
                    anyhow::ensure!(matches!(position.current_plan(), ConcreteStep::Skill(s) if s.policy.repeat.is_some()),
                        "this step does not own a loop decision");
                    format!("task:{}:{}:{}", task.id, position.invocation.id, position.cursor.boundary_key())
                };
                let summary = crate::ops::human_session::ask_once(&store, &key, &reason, Some("unblock")).await?;
                println!("Session complete: {summary}\nReassess the current evidence before choosing Advance or Iterate. This returned feedback, not a navigation decision.");
                Ok(())
            })
        }
        "resume" => {
            anyhow::ensure!(
                args.len() == 1 || (args.len() == 2 && args[1] == "--retry"),
                "usage: lf flow resume INVOCATION [--retry]"
            );
            let record = flow_run::read(&args[0])?;
            if args.len() == 2 {
                let _driver = flow_run::driver_lock(&record.id)?;
                flow_run::retry(&record.id)?;
            }
            let argv = std::env::args().collect::<Vec<_>>();
            journal::with_runtime(&record.cwd, &argv, || {
                report_outcome(drive_saved(&record.id, cli)?)
            })
        }
        _ => anyhow::bail!("unknown Flow control {command}"),
    }
}

fn active_run_id() -> Result<crate::durable::RunId> {
    crate::durable::RunId::parse(
        &std::env::var(crate::durable::RUN_ID_ENV)
            .context("this operation requires the active decision Run")?,
    )
    .map_err(Into::into)
}

fn drive_saved(id: &str, cli: &Cli) -> Result<FlowOutcome> {
    let _driver = flow_run::driver_lock(id)?;
    flow_run::recover(id)?;
    let mut record = flow_run::read(id)?;
    if record.finished {
        println!("Flow {} is already finished.", record.flow);
        return Ok(FlowOutcome::Completed);
    }
    if let Some(reason) = &record.failure {
        anyhow::bail!("Flow {} is blocked: {reason}", record.id);
    }
    let _flow_env = EnvVarGuard::set("LOOPFLOW_FLOW_NAME", &record.flow);
    let mut launch = cli.launch_options();
    launch.wave = record.wave.clone();
    launch.task = record.task.clone();
    launch.as_work = record.as_work.clone();
    if launch.model.is_none() {
        launch.model = record.model.clone();
    }
    launch.bound_cwd = Some(record.cwd.clone());
    let executor = CliFlowExecutor {
        cli: &launch,
        message: record.message.as_deref(),
        repo: record.cwd.clone(),
        invocation: id.to_owned(),
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let outcome = runtime
        .block_on(FlowEngine::new(executor).run_with_cursor(&record.steps, &mut record.cursor));
    match outcome {
        Ok(FlowOutcome::Completed) => {
            flow_run::update(id, |run| {
                run.finished = true;
                run.active = None;
                Ok(())
            })?;
            Ok(FlowOutcome::Completed)
        }
        Ok(FlowOutcome::Waiting) => Ok(FlowOutcome::Waiting),
        Ok(FlowOutcome::Blocked(reason)) => {
            flow_run::update(id, |run| {
                run.failure = Some(reason.clone());
                Ok(())
            })?;
            anyhow::bail!("Flow {id} blocked: {reason}")
        }
        Err(error) => {
            flow_run::update(id, |run| {
                run.failure = Some(error.to_string());
                Ok(())
            })?;
            Err(error)
        }
    }
}

fn print_pipeline_header(flow_name: &str, items: &[ConcreteStep], repo: &Path) -> Result<()> {
    let colors = Colors::new();
    let lines = render_pipeline_lines(items, repo)?;
    let pipeline = lines
        .into_iter()
        .map(|line| {
            format!(
                "  {dim}{line}{reset}",
                dim = colors.dim,
                reset = colors.reset
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    eprintln!(
        "\n{dim}\u{2500}\u{2500} flow {reset}{bold}{name}{reset}\n{pipeline}\n",
        dim = colors.dim,
        reset = colors.reset,
        bold = colors.bold,
        name = flow_name,
        pipeline = pipeline,
    );
    Ok(())
}

fn render_pipeline_lines(items: &[ConcreteStep], repo: &Path) -> Result<Vec<String>> {
    let mut lines = Vec::new();
    for item in items {
        lines.extend(render_pipeline_item(item, repo)?);
    }
    Ok(lines)
}

fn render_pipeline_item(item: &ConcreteStep, repo: &Path) -> Result<Vec<String>> {
    match item {
        ConcreteStep::Skill(skill) if skill.policy.human => Ok(vec![format!(
            "{} [review:{}]",
            skill.skill.name,
            skill
                .policy
                .id
                .as_deref()
                .expect("validated review node has an id"),
        )]),
        ConcreteStep::Skill(skill) => Ok(vec![skill.skill.name.clone()]),
        ConcreteStep::Op(ops) => Ok(vec![format!("op: {}", ops.item.display_name())]),
        ConcreteStep::Xor(branch) => render_branch_item("xor", branch, repo),
    }
}

fn render_branch_item(kind: &str, branch: &ConcreteXor, repo: &Path) -> Result<Vec<String>> {
    render_branch_pipeline(kind, &branch.router.name, &branch.paths, repo)
}

fn render_branch_pipeline(
    kind: &str,
    router: &str,
    paths: &std::collections::HashMap<String, crate::engine::ConcretePath>,
    repo: &Path,
) -> Result<Vec<String>> {
    let mut lines = vec![format!("[{kind} via {router}]")];
    let mut keys: Vec<&String> = paths.keys().collect();
    keys.sort();

    for (index, key) in keys.into_iter().enumerate() {
        let path = paths
            .get(key)
            .expect("branch path key collected from map should exist");
        let nested = render_pipeline_lines(&path.steps, repo)?;
        let branch_prefix = tree_prefix(index, paths.len());
        if nested.is_empty() {
            lines.push(format!("{branch_prefix} {key}"));
            continue;
        }

        let nested_chain = nested.join(" → ");
        lines.push(format!("{branch_prefix} {key} → {nested_chain}"));
    }

    Ok(lines)
}

fn tree_prefix(index: usize, total: usize) -> &'static str {
    if index + 1 == total {
        "└─"
    } else {
        "├─"
    }
}

struct EnvVarGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var_os(key);
        std::env::set_var(key, value);
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(previous) = &self.previous {
            std::env::set_var(self.key, previous);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

struct CliFlowExecutor<'a> {
    cli: &'a Cli,
    message: Option<&'a str>,
    repo: PathBuf,
    invocation: String,
}

#[async_trait]
impl SkillExecutor for CliFlowExecutor<'_> {
    async fn run_skill(
        &self,
        skill: &ConcreteSkill,
        ctx: ExecutionContext,
    ) -> Result<SkillOutcome> {
        if skill.policy.human {
            let id = &self.invocation;
            let (token, completed) = flow_run::begin_boundary(id)?;
            if completed {
                return flow_run::finish_boundary(&token, None);
            }
            eprintln!(
                "Flow {id} is waiting at {}. Open its Flow Session with lf session.",
                skill.skill.name
            );
            return Ok(SkillOutcome::Waiting);
        }
        if let Some(progress) = ctx.progress {
            print_skill_progress(progress, &skill.skill.name);
        } else {
            print_nested_skill_progress(&skill.skill.name);
        }

        let (token, completed) = flow_run::begin_boundary(&self.invocation)?;
        if completed {
            return flow_run::finish_boundary(&token, None);
        }
        let encoded = serde_json::to_string(&token)?;
        let _token = EnvVarGuard::set(flow_run::FLOW_STEP_ENV, &encoded);
        let mut message = self.message.unwrap_or_default().to_string();
        if let Some(direction) = &ctx.direction {
            message.push_str(&format!(
                "\n\nPrevious step feedback or iteration direction:\n{direction}"
            ));
        }
        if skill.policy.repeat.is_some() {
            message.push_str("\n\nRecord this occurrence's decision with `lf flow decide advance \"evidence\"` or `lf flow decide iterate \"next action and proof\"`. If progress is stalled, use `lf flow blocked \"reason, attempted direction, evidence, and question\"`; it opens one Ask running unblock and returns the human's summary. Reassess afterward. Invalid commands return correction feedback; correct the decision here without replaying implementation. A recorded decision is accepted only after this Run succeeds.");
        }
        let mut launch = self.cli.launch_options();
        launch.batch = true;
        launch.interactive = false;
        launch.tui = false;
        launch.ide = false;
        let result = run_skill_with_journal(
            &self.repo,
            &skill.skill.name,
            ctx.progress.map(|p| p.index),
            || {
                crate::lf::commands::run::run_saved(
                    &skill.skill,
                    Some(&message),
                    &launch,
                    &self.repo,
                )?;
                commit_skill_work(&self.repo, &skill.skill.name)?;
                Ok(())
            },
        );
        let verdict = flow_run::finish_boundary(
            &token,
            result.as_ref().err().map(ToString::to_string).as_deref(),
        )?;
        result?;
        Ok(verdict)
    }

    async fn checkpoint(&self, cursor: &crate::engine::ExecutionCursor) -> Result<()> {
        flow_run::checkpoint(&self.invocation, cursor)
    }

    async fn run_op(&self, ops: &crate::engine::ConcreteOp, _ctx: ExecutionContext) -> Result<()> {
        let name = format!("op: {}", ops.item.display_name());
        let (token, completed) = flow_run::begin_boundary(&self.invocation)?;
        if completed {
            return Ok(());
        }
        eprintln!("{name}");
        let result = crate::ops::execute_flow_ops(&self.repo, &ops.item, &NullProgress);
        flow_run::finish_boundary(
            &token,
            result.as_ref().err().map(ToString::to_string).as_deref(),
        )?;
        result.map_err(Into::into)
    }
}

fn print_skill_progress(progress: StepProgress, skill_name: &str) {
    let colors = Colors::new();
    eprintln!(
        "{dim}[{current}/{total}]{reset} {bold}{name}{reset}",
        dim = colors.dim,
        reset = colors.reset,
        bold = colors.bold,
        current = progress.index + 1,
        total = progress.total,
        name = skill_name,
    );
}

fn print_nested_skill_progress(skill_name: &str) {
    let colors = Colors::new();
    eprintln!(
        "{dim}[*]{reset} {bold}{name}{reset}",
        dim = colors.dim,
        reset = colors.reset,
        bold = colors.bold,
        name = skill_name,
    );
}

fn run_skill_with_journal(
    repo: &Path,
    skill_name: &str,
    index: Option<usize>,
    run: impl FnOnce() -> Result<()>,
) -> Result<()> {
    journal::emit(
        repo,
        LfNode::Skill,
        LfEventType::Started,
        LfEventFields {
            skill: Some(skill_name.to_string()),
            index: index.map(|value| value as u32),
            ..LfEventFields::default()
        },
    );
    let result = run();
    match &result {
        Ok(_) => journal::emit(
            repo,
            LfNode::Skill,
            LfEventType::Completed,
            LfEventFields {
                skill: Some(skill_name.to_string()),
                index: index.map(|value| value as u32),
                ..LfEventFields::default()
            },
        ),
        Err(err) => journal::emit(
            repo,
            LfNode::Skill,
            LfEventType::Errored,
            LfEventFields {
                skill: Some(skill_name.to_string()),
                index: index.map(|value| value as u32),
                error: Some(err.to_string()),
                ..LfEventFields::default()
            },
        ),
    }
    result
}

/// Commit any uncommitted changes left by the previous skill.
pub(crate) fn commit_skill_work(repo: &Path, skill_name: &str) -> Result<bool> {
    let options = CommitOptions {
        add: true,
        message: Some(format!("lf commit: {skill_name}")),
        ..CommitOptions::for_task(skill_name)
    };
    commit_workflow(repo, &options, &NullProgress).map_err(Into::into)
}
#[cfg(test)]
mod tests {
    use super::render_pipeline_lines;
    use crate::engine::{ConcreteSkill, ConcreteStep, Flow, Skill};
    use crate::lf::Cli;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn render_pipeline_lines_expands_xor_paths_on_separate_lines() {
        let temp = tempdir().unwrap();
        let skills = temp.path().join(".lf/skills/tend");
        fs::create_dir_all(&skills).unwrap();
        for name in ["scan-waves", "assess", "play-chord", "review-chord"] {
            fs::write(skills.join(format!("{name}.md")), format!("Fixture {name}")).unwrap();
        }
        let flows_dir = temp.path().join(".lf/flows/tend");
        fs::create_dir_all(&flows_dir).unwrap();
        fs::write(
            flows_dir.join("tune.yaml"),
            "- tend/play-chord\n- tend/review-chord\n",
        )
        .unwrap();

        let flow = Flow {
            name: "tend".to_string(),
            items: vec![
                crate::engine::flow::Step::Skill(crate::engine::flow::SkillStep {
                    skill: crate::engine::flow::Skill::named("tend/scan-waves"),
                    policy: crate::engine::OccurrencePolicy::default(),
                }),
                crate::engine::flow::Step::Xor(crate::engine::flow::XorDef {
                    router: Some("tend/assess".to_string()),
                    paths: [
                        (
                            "tune".to_string(),
                            crate::engine::flow::XorPath {
                                flow: Some("tend/tune".to_string()),
                                skill: None,
                                steps: Vec::new(),
                                description: "Adjust the chord".to_string(),
                            },
                        ),
                        (
                            "silence".to_string(),
                            crate::engine::flow::XorPath {
                                flow: None,
                                skill: None,
                                steps: Vec::new(),
                                description: "No-op".to_string(),
                            },
                        ),
                    ]
                    .into_iter()
                    .collect(),
                }),
            ],
        };

        let items = crate::engine::expand_flow(&flow, temp.path()).unwrap();
        let lines = render_pipeline_lines(&items, temp.path()).unwrap();

        assert_eq!(
            lines,
            vec![
                "tend/scan-waves".to_string(),
                "[xor via tend/assess]".to_string(),
                "├─ silence".to_string(),
                "└─ tune → tend/play-chord → tend/review-chord".to_string(),
            ]
        );

        assert!(matches!(items[1], ConcreteStep::Xor(_)));
    }

    #[test]
    fn rendered_pipeline_lists_human_node_identity() {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let flow = crate::engine::load_flow("task-design", &repo).unwrap();
        let items = crate::engine::expand_flow(&flow, &repo).unwrap();

        let lines = render_pipeline_lines(&items, &repo).unwrap();
        assert_eq!(
            lines,
            vec![
                "kickoff".to_string(),
                "review-design [review:review_kickoff]".to_string(),
            ]
        );
    }

    #[test]
    fn validation_lists_human_nodes_hidden_in_xor_paths() {
        let repo = tempdir().unwrap();
        let flows = repo.path().join(".lf/flows");
        fs::create_dir_all(&flows).unwrap();
        fs::write(
            flows.join("choice.yaml"),
            "- xor:\n    paths:\n      review:\n        description: Review it\n        steps:\n          - step:\n              id: revise_choice\n              name: implement\n          - step:\n              id: review_choice\n              name: review-design\n              human: true\n",
        )
        .unwrap();
        let flow = crate::engine::load_flow("choice", repo.path()).unwrap();
        let human = crate::engine::human_occurrence_ids(&flow, repo.path()).unwrap();
        assert_eq!(human, vec!["review_choice"]);
        let items = crate::engine::expand_flow(&flow, repo.path()).unwrap();
        assert!(render_pipeline_lines(&items, repo.path())
            .unwrap()
            .iter()
            .any(|line| line.contains("review-design [review:review_choice]")));
    }

    #[test]
    fn ordinary_flow_parks_at_the_same_durable_review_after_recovery() {
        let _lock = crate::journal::test_env_lock();
        let home = tempdir().unwrap();
        let _home = super::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let _binary =
            super::EnvVarGuard::set("LF_BIN", std::env::current_exe().unwrap().to_str().unwrap());
        let cli = Cli {
            batch: true,
            ..Cli::default()
        };
        let steps = vec![ConcreteStep::Skill(ConcreteSkill {
            skill: Skill::named("concept-review"),
            policy: crate::engine::OccurrencePolicy {
                id: Some("review".into()),
                human: true,
                repeat: None,
            },
            flow_parents: vec![],
        })];
        let run = crate::ops::flow_run::create("proof", &steps, home.path(), None, &cli).unwrap();
        assert_eq!(
            super::drive_saved(&run.id, &cli).unwrap(),
            crate::engine::FlowOutcome::Waiting
        );
        let before = crate::ops::flow_run::read(&run.id).unwrap();
        let token = before.active.as_ref().unwrap().id.clone();
        assert!(before.failure.is_none());
        assert_eq!(crate::ops::flow_session::list().unwrap().len(), 1);
        assert_eq!(
            super::drive_saved(&run.id, &cli).unwrap(),
            crate::engine::FlowOutcome::Waiting
        );
        let after = crate::ops::flow_run::read(&run.id).unwrap();
        assert_eq!(after.active.unwrap().id, token);
        assert_eq!(after.cursor, before.cursor);
        assert!(!after.finished);

        let boundary = crate::ops::flow_run::StepToken {
            invocation: run.id.clone(),
            boundary: token,
        };
        let review = crate::durable::RunId::new();
        crate::ops::flow_run::update(&run.id, |saved| {
            saved.active.as_mut().unwrap().run_id = Some(review.clone());
            Ok(())
        })
        .unwrap();
        crate::ops::flow_session::mark_ready(
            &boundary,
            &review,
            "Design clarified; review finished",
        )
        .unwrap();
        assert_eq!(
            super::drive_saved(&run.id, &cli).unwrap(),
            crate::engine::FlowOutcome::Waiting
        );
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(crate::ops::flow_session::complete(&boundary))
            .unwrap();
        let completed = crate::ops::flow_run::read(&run.id).unwrap();
        assert_eq!(completed.cursor, before.cursor);
        assert!(completed.cursor.progress.verdict.is_none());
        assert!(completed.active.unwrap().completed);
        assert_eq!(
            super::drive_saved(&run.id, &cli).unwrap(),
            crate::engine::FlowOutcome::Completed
        );
        let finished = crate::ops::flow_run::read(&run.id).unwrap();
        assert!(finished.finished);
        assert_eq!(
            finished.cursor.progress.direction.as_deref(),
            Some("Design clarified; review finished")
        );
        assert!(finished.active.is_none());
        assert_eq!(
            super::drive_saved(&run.id, &cli).unwrap(),
            crate::engine::FlowOutcome::Completed
        );
    }
}
