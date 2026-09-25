//! Durable positions for ordinary Flow invocations. Task positions remain owned
//! by their Task transaction; both adapters use the same engine transitions.
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, ensure, Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};

use crate::durable::RunId;
use crate::engine::transitions::FlowVerdict;
use crate::engine::{ConcreteStep, ExecutionCursor};

pub(crate) const FLOW_STEP_ENV: &str = "LF_FLOW_STEP";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct StepToken {
    pub invocation: String,
    pub boundary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Boundary {
    pub id: String,
    pub run_id: Option<RunId>,
    pub run_dir: Option<PathBuf>,
    pub completed: bool,
    pub ready_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FlowRun {
    pub id: String,
    pub flow: String,
    pub cwd: PathBuf,
    pub steps: Vec<ConcreteStep>,
    pub cursor: ExecutionCursor,
    pub message: Option<String>,
    pub model: Option<String>,
    pub wave: Option<String>,
    pub task: Option<String>,
    pub as_work: Option<String>,
    pub active: Option<Boundary>,
    pub failure: Option<String>,
    pub finished: bool,
}

impl FlowRun {
    pub(crate) fn current_step(&self) -> Result<&ConcreteStep> {
        let (steps, cursor) = self.cursor.current_body(&self.steps);
        steps
            .get(cursor.index)
            .ok_or_else(|| anyhow!("saved Flow position has no current step"))
    }

    pub(crate) fn is_human(&self) -> Result<bool> {
        Ok(matches!(self.current_step()?, ConcreteStep::Skill(skill) if skill.policy.human))
    }
}

fn directory(id: &str) -> Result<PathBuf> {
    uuid::Uuid::parse_str(id).context("invalid Flow invocation id")?;
    Ok(crate::store::current_home_lf_home_dir()
        .join("flows")
        .join(id))
}

fn lock(id: &str, name: &str) -> Result<File> {
    let dir = directory(id)?;
    fs::create_dir_all(&dir)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join(name))?;
    FileExt::lock_exclusive(&file)?;
    Ok(file)
}

pub(crate) fn driver_lock(id: &str) -> Result<File> {
    lock(id, "driver.lock")
}

/// Request the existing Home process supervisor to continue this saved invocation.
#[cfg(not(test))]
pub(crate) async fn launch_driver(id: &str) -> Result<()> {
    let run = read(id)?;
    let lf = crate::engine::process::resolve_current_home_lf_binary_checked()?;
    let argv = vec![
        lf.display().to_string(),
        "-b".into(),
        "flow".into(),
        "resume".into(),
        id.into(),
    ];
    crate::engine::process::start_home_session(&format!("lf-flow-{id}"), &run.cwd, &argv).await
}

#[cfg(test)]
pub(crate) async fn launch_driver(id: &str) -> Result<()> {
    read(id)?;
    Ok(())
}

pub(crate) fn read(id: &str) -> Result<FlowRun> {
    let path = directory(id)?.join("position.json");
    let bytes =
        fs::read(&path).with_context(|| format!("read saved Flow position {}", path.display()))?;
    let run: FlowRun = serde_json::from_slice(&bytes).with_context(|| {
        format!(
            "cannot decode saved Flow position {}; the saved bytes are unchanged. \
             Start a new invocation with `lf flow <name>` if this definition cannot be recovered",
            path.display()
        )
    })?;
    ensure!(run.id == id, "Flow record identity differs from its path");
    Ok(run)
}

fn write(run: &FlowRun) -> Result<()> {
    let dir = directory(&run.id)?;
    fs::create_dir_all(&dir)?;
    let temporary = dir.join(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary)?;
    file.write_all(&serde_json::to_vec_pretty(run)?)?;
    file.sync_all()?;
    fs::rename(temporary, dir.join("position.json"))?;
    Ok(())
}

pub(crate) fn update<T>(id: &str, edit: impl FnOnce(&mut FlowRun) -> Result<T>) -> Result<T> {
    let _lock = lock(id, "position.lock")?;
    let mut run = read(id)?;
    let result = edit(&mut run)?;
    write(&run)?;
    Ok(result)
}

pub(crate) fn create(
    flow: &str,
    steps: &[ConcreteStep],
    cwd: &Path,
    message: Option<&str>,
    cli: &crate::lf::Cli,
) -> Result<FlowRun> {
    let run = FlowRun {
        id: uuid::Uuid::new_v4().to_string(),
        flow: flow.to_owned(),
        cwd: cwd.to_path_buf(),
        steps: steps.to_vec(),
        cursor: ExecutionCursor::default(),
        message: message.map(str::to_owned),
        model: cli.model.clone(),
        wave: cli.wave.clone(),
        task: cli.task.clone(),
        as_work: cli.as_work.clone(),
        active: None,
        failure: None,
        finished: false,
    };
    write(&run)?;
    Ok(run)
}

pub(crate) fn checkpoint(id: &str, cursor: &ExecutionCursor) -> Result<()> {
    update(id, |run| {
        // Saving a decision precedes taking the edge. Keep its Run receipt until
        // the engine changes the position, including a nested position.
        let mut old = run.cursor.clone();
        let mut next = cursor.clone();
        old.leaf_mut().progress.verdict = None;
        old.leaf_mut().route = None;
        next.leaf_mut().progress.verdict = None;
        next.leaf_mut().route = None;
        let saved = run.cursor.leaf_mut().progress.verdict.clone();
        let saved_route = run.cursor.leaf().route.clone();
        if old != next {
            run.active = None;
            run.cursor = cursor.clone();
        } else {
            let pending = cursor.leaf().progress.verdict.clone();
            ensure!(
                saved.is_none() || pending.is_none() || saved == pending,
                "checkpoint conflicts with the saved Flow decision"
            );
            let pending_route = cursor.leaf().route.clone();
            ensure!(
                saved_route.is_none() || pending_route.is_none() || saved_route == pending_route,
                "checkpoint conflicts with the saved route"
            );
            run.cursor = cursor.clone();
            run.cursor.leaf_mut().progress.verdict = saved.or(pending);
            run.cursor.leaf_mut().route = saved_route.or(pending_route);
        }
        Ok(())
    })
}

pub(crate) fn token() -> Result<Option<StepToken>> {
    std::env::var(FLOW_STEP_ENV)
        .ok()
        .map(|s| serde_json::from_str(&s).context("invalid active Flow step identity"))
        .transpose()
}

pub(crate) fn bind_run(run_id: &RunId, run_dir: &Path) -> Result<()> {
    let Some(token) = token()? else { return Ok(()) };
    update(&token.invocation, |run| {
        let active = run
            .active
            .as_mut()
            .ok_or_else(|| anyhow!("Flow has no active step"))?;
        ensure!(active.id == token.boundary, "stale Flow step launch");
        // Nested helper Runs inherit context, not the parent's decision authority.
        if active.run_id.is_none() {
            active.run_id = Some(run_id.clone());
            active.run_dir = Some(run_dir.to_path_buf());
        }
        Ok(())
    })
}

pub(crate) fn record_decision(
    token: &StepToken,
    run_id: &RunId,
    verdict: &FlowVerdict,
) -> Result<()> {
    ensure!(
        !verdict.summary.trim().is_empty(),
        "decision requires evidence or direction"
    );
    update(&token.invocation, |run| {
        let active = run
            .active
            .as_ref()
            .ok_or_else(|| anyhow!("Flow has no active step"))?;
        ensure!(
            active.id == token.boundary && active.run_id.as_ref() == Some(run_id),
            "decision belongs to a stale or different Flow Run"
        );
        ensure!(
            !run.is_human()?,
            "an interactive review returns feedback; its following decision step owns navigation"
        );
        ensure!(
            matches!(run.current_step()?, ConcreteStep::Skill(skill) if skill.policy.repeat.is_some()),
            "this Flow step does not own a decision"
        );
        ensure!(
            !active.completed && run.failure.is_none(),
            "Flow step is no longer running"
        );
        let progress = &mut run.cursor.leaf_mut().progress;
        ensure!(
            progress.verdict.as_ref().is_none_or(|old| old == verdict),
            "Flow step already has a different decision"
        );
        progress.verdict = Some(verdict.clone());
        Ok(())
    })
}

pub(crate) fn record_route(token: &StepToken, run_id: &RunId, path: &str) -> Result<()> {
    update(&token.invocation, |run| {
        let active = run
            .active
            .as_ref()
            .ok_or_else(|| anyhow!("Flow has no active step"))?;
        ensure!(
            active.id == token.boundary && active.run_id.as_ref() == Some(run_id),
            "route belongs to a stale or different Run"
        );
        ensure!(
            !active.completed && run.failure.is_none(),
            "Flow router is no longer running"
        );
        let ConcreteStep::Xor(branch) = run.current_step()? else {
            bail!("this Flow step is not a router")
        };
        ensure!(
            branch.paths.contains_key(path),
            "unknown Flow path {path:?}"
        );
        let route = &mut run.cursor.leaf_mut().route;
        ensure!(
            route.as_ref().is_none_or(|saved| saved == path),
            "router already selected a different path"
        );
        *route = Some(path.to_owned());
        Ok(())
    })
}

pub(crate) fn require_active(token: &StepToken, run_id: &RunId) -> Result<String> {
    let run = read(&token.invocation)?;
    let active = run
        .active
        .as_ref()
        .ok_or_else(|| anyhow!("Flow has no active step"))?;
    ensure!(
        active.id == token.boundary && active.run_id.as_ref() == Some(run_id),
        "this is not the current Flow decision Run"
    );
    ensure!(
        matches!(run.current_step()?, ConcreteStep::Skill(skill)
            if skill.policy.repeat.is_some() && !skill.policy.human)
            && !active.completed
            && !run.finished
            && run.failure.is_none(),
        "this step cannot report a loop blocker"
    );
    Ok(format!(
        "flow:{}:{}",
        token.invocation,
        run.cursor.boundary_key()
    ))
}

pub(crate) fn recover(id: &str) -> Result<()> {
    update(id, |run| {
        let Some(active) = run.active.as_ref() else {
            return Ok(());
        };
        if active.completed || run.is_human()? {
            return Ok(());
        }
        let name = match run.current_step()? {
            ConcreteStep::Skill(skill) => skill.skill.name.clone(),
            ConcreteStep::Xor(branch) => branch.router.name.clone(),
            ConcreteStep::Op(op) => format!("op: {}", op.item.display_name()),
        };
        let active = run.active.as_mut().expect("active boundary checked above");
        if let Some(dir) = &active.run_dir {
            let snapshot = crate::run_record::read_run_snapshot(dir)?;
            match snapshot.status() {
                "completed" => active.completed = true,
                "failed" | "interrupted" => {
                    run.cursor.leaf_mut().progress.verdict = None;
                    run.cursor.leaf_mut().route = None;
                    run.failure = Some(format!("{name} Run {}", snapshot.status()));
                }
                _ => {
                    // A saved decision alone is insufficient to settle a still-live
                    // or unterminated Run. Keep its owner and join/report it.
                    run.cursor.leaf_mut().progress.verdict = None;
                    run.cursor.leaf_mut().route = None;
                    bail!(
                        "Flow is waiting for Run {}; its completion is not recorded",
                        snapshot.id
                    );
                }
            }
        } else {
            // The launch did not bind a provider. An operation's side effect might
            // already have happened; require inspection rather than replaying it.
            run.failure = Some(format!(
                "{name} was interrupted before a completion receipt"
            ));
        }
        Ok(())
    })
}

pub(crate) fn retry(id: &str) -> Result<()> {
    recover(id)?;
    update(id, |run| {
        ensure!(
            run.failure.is_some(),
            "Flow has no recorded failure to retry"
        );
        ensure!(
            !run.is_human()?,
            "reopen the human Session to complete this review"
        );
        run.failure = None;
        run.active = None;
        run.cursor.leaf_mut().progress.verdict = None;
        run.cursor.leaf_mut().route = None;
        Ok(())
    })
}

pub(crate) fn begin_boundary(id: &str) -> Result<(StepToken, bool)> {
    update(id, |run| {
        ensure!(
            !run.finished && run.failure.is_none(),
            "Flow is not ready to execute"
        );
        run.current_step()?;
        let boundary = run.active.get_or_insert_with(|| Boundary {
            id: uuid::Uuid::new_v4().to_string(),
            run_id: None,
            run_dir: None,
            completed: false,
            ready_summary: None,
        });
        Ok((
            StepToken {
                invocation: id.into(),
                boundary: boundary.id.clone(),
            },
            boundary.completed,
        ))
    })
}

pub(crate) fn finish_boundary(
    token: &StepToken,
    failure: Option<&str>,
) -> Result<crate::engine::SkillOutcome> {
    update(&token.invocation, |run| {
        let human = run.is_human()?;
        let boundary = run
            .active
            .as_mut()
            .ok_or_else(|| anyhow!("Flow step disappeared"))?;
        ensure!(boundary.id == token.boundary, "Flow step was replaced");
        let cursor = run.cursor.leaf_mut();
        if let Some(reason) = failure {
            cursor.progress.verdict = None;
            cursor.route = None;
            run.failure = Some(reason.to_string());
        } else {
            boundary.completed = true;
        }
        Ok(if let Some(route) = &cursor.route {
            crate::engine::SkillOutcome::Routed(route.clone())
        } else {
            cursor.progress.verdict.clone().map_or(
                crate::engine::SkillOutcome::Completed {
                    feedback: human.then(|| boundary.ready_summary.clone()).flatten(),
                },
                crate::engine::SkillOutcome::Decided,
            )
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{
        begin_boundary, bind_run, checkpoint, create, finish_boundary, read, record_decision,
        recover, require_active, update, FLOW_STEP_ENV,
    };
    use crate::durable::RunId;
    use crate::engine::flow::RepeatPolicy;
    use crate::engine::transitions::{FlowDecision, FlowVerdict};
    use crate::engine::{ConcreteSkill, ConcreteStep, ExecutionCursor, OccurrencePolicy, Skill};
    use crate::lf::Cli;
    use crate::run_record::{CaptureHandle, RunSpec};
    use std::ffi::OsString;

    struct Home {
        dir: tempfile::TempDir,
        previous: Vec<(&'static str, Option<OsString>)>,
    }
    impl Home {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let previous = [
                "LF_HOME",
                "LF_CONTROL_HOME",
                "LF_CONTROL_DB_PATH",
                "LF_DB_PATH",
                FLOW_STEP_ENV,
                "LF_RUN_CONTEXT",
                "LF_RUN_ID",
                "LF_RUN_DIR",
            ]
            .into_iter()
            .map(|key| {
                let value = std::env::var_os(key);
                std::env::remove_var(key);
                (key, value)
            })
            .collect();
            std::env::set_var("LF_HOME", dir.path());
            Self { dir, previous }
        }
        fn run(&self) -> super::FlowRun {
            let steps = [
                step("work", None),
                step("loop-decide", Some("work")),
                step("finish", None),
            ];
            let run = create("proof", &steps, self.dir.path(), None, &Cli::default()).unwrap();
            update(&run.id, |run| {
                run.cursor.index = 1;
                Ok(())
            })
            .unwrap();
            read(&run.id).unwrap()
        }
        fn capture(&self) -> CaptureHandle {
            CaptureHandle::begin_at(
                self.dir.path(),
                RunSpec {
                    harness: "proof".into(),
                    model: None,
                    surface: "headless".into(),
                    cwd: self.dir.path().into(),
                    repo: None,
                    worktree: None,
                    skill: Some("loop-decide".into()),
                    subjects: vec![],
                },
            )
            .unwrap()
        }
    }
    impl Drop for Home {
        fn drop(&mut self) {
            for (key, value) in self.previous.drain(..) {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }
    fn step(id: &str, from: Option<&str>) -> ConcreteStep {
        ConcreteStep::Skill(ConcreteSkill {
            skill: Skill::named(id),
            flow_parents: vec![],
            policy: OccurrencePolicy {
                id: Some(id.into()),
                human: false,
                repeat: from.map(|from| RepeatPolicy { from: from.into() }),
            },
        })
    }
    fn verdict(decision: FlowDecision) -> FlowVerdict {
        FlowVerdict {
            decision,
            summary: "Observed progress; next proof is specific".into(),
        }
    }

    #[test]
    fn recovery_requires_success_and_rejects_stale_or_conflicting_decisions() {
        let _lock = crate::journal::test_env_lock();
        let home = Home::new();
        for outcome in ["completed", "failed", "interrupted"] {
            let run = home.run();
            let (token, _) = begin_boundary(&run.id).unwrap();
            std::env::set_var(FLOW_STEP_ENV, serde_json::to_string(&token).unwrap());
            let capture = home.capture();
            bind_run(&capture.run_id(), &capture.artifact_dir()).unwrap();
            let decision = verdict(FlowDecision::Iterate);
            record_decision(&token, &capture.run_id(), &decision).unwrap();
            record_decision(&token, &capture.run_id(), &decision).unwrap();
            let before = serde_json::to_value(read(&run.id).unwrap()).unwrap();
            assert!(record_decision(&token, &RunId::new(), &decision).is_err());
            assert!(
                record_decision(&token, &capture.run_id(), &verdict(FlowDecision::Advance))
                    .is_err()
            );
            assert_eq!(
                serde_json::to_value(read(&run.id).unwrap()).unwrap(),
                before
            );
            assert!(
                recover(&run.id).is_err(),
                "unterminated Run must not settle its candidate"
            );
            capture.finish(outcome).unwrap();
            recover(&run.id).unwrap();
            let recovered = read(&run.id).unwrap();
            assert_eq!(recovered.cursor.index, 1);
            if outcome == "completed" {
                assert_eq!(recovered.cursor.progress.verdict, Some(decision));
                assert!(recovered.active.unwrap().completed);
                let mut next = recovered.cursor;
                crate::engine::transitions::finish_step(&run.steps, 1, &mut next.progress).unwrap();
                next.index = 0;
                next.iteration += 1;
                checkpoint(&run.id, &next).unwrap();
                assert!(read(&run.id).unwrap().active.is_none());
                assert!(record_decision(
                    &token,
                    &capture.run_id(),
                    &verdict(FlowDecision::Advance)
                )
                .is_err());
            } else {
                assert!(recovered.cursor.progress.verdict.is_none());
                assert!(recovered.failure.unwrap().contains(outcome));
                assert!(require_active(&token, &capture.run_id()).is_err());
            }
        }
    }

    #[test]
    fn human_provider_exit_never_completes_the_review() {
        let _lock = crate::journal::test_env_lock();
        let home = Home::new();
        let run = home.run();
        update(&run.id, |run| {
            let ConcreteStep::Skill(skill) = &mut run.steps[1] else {
                unreachable!();
            };
            skill.policy.human = true;
            skill.policy.repeat = None;
            Ok(())
        })
        .unwrap();
        let (token, _) = begin_boundary(&run.id).unwrap();
        std::env::set_var(FLOW_STEP_ENV, serde_json::to_string(&token).unwrap());
        let capture = home.capture();
        bind_run(&capture.run_id(), &capture.artifact_dir()).unwrap();
        // Older positions duplicated policy on the boundary. Recover their
        // attempt identity and receipt using the captured definition's policy.
        let mut old = serde_json::to_value(read(&run.id).unwrap()).unwrap();
        old["active"]["name"] = "loop-decide".into();
        old["active"]["human"] = true.into();
        old["active"]["deciding"] = true.into();
        std::fs::write(
            super::directory(&run.id).unwrap().join("position.json"),
            serde_json::to_vec(&old).unwrap(),
        )
        .unwrap();
        assert!(
            record_decision(&token, &capture.run_id(), &verdict(FlowDecision::Advance)).is_err()
        );
        assert!(require_active(&token, &capture.run_id()).is_err());
        capture.finish("completed").unwrap();
        recover(&run.id).unwrap();
        let saved = read(&run.id).unwrap();
        let boundary = saved.active.as_ref().unwrap();
        assert_eq!(boundary.id, token.boundary);
        assert_eq!(boundary.run_id, Some(capture.run_id()));
        assert_eq!(boundary.run_dir, Some(capture.artifact_dir()));
        assert!(!boundary.completed);
        assert!(saved.cursor.progress.verdict.is_none());
        let encoded = serde_json::to_value(&saved).unwrap();
        for field in ["name", "human", "deciding"] {
            assert!(encoded["active"].get(field).is_none());
        }
    }

    #[test]
    fn completed_operation_is_not_replayed_and_ambiguous_operation_blocks() {
        let _lock = crate::journal::test_env_lock();
        let home = Home::new();
        let steps = ["publish", "deliver"].map(|command| {
            ConcreteStep::Op(crate::engine::ConcreteOp {
                item: crate::engine::Op {
                    command: command.into(),
                    args: vec![],
                },
                flow_parents: vec![],
            })
        });
        let run = create("operations", &steps, home.dir.path(), None, &Cli::default()).unwrap();
        let (token, completed) = begin_boundary(&run.id).unwrap();
        assert!(!completed);
        finish_boundary(&token, None).unwrap();
        recover(&run.id).unwrap();
        assert!(begin_boundary(&run.id).unwrap().1);
        let next = ExecutionCursor {
            index: 1,
            ..ExecutionCursor::default()
        };
        checkpoint(&run.id, &next).unwrap();
        begin_boundary(&run.id).unwrap();
        recover(&run.id).unwrap();
        let saved = read(&run.id).unwrap();
        assert_eq!(saved.cursor.index, 1);
        assert!(saved.failure.unwrap().contains("completion receipt"));
    }
    #[test]
    fn waiting_checkpoint_preserves_concurrent_review_completion() {
        let _lock = crate::journal::test_env_lock();
        let home = Home::new();
        let run = home.run();
        update(&run.id, |run| {
            let ConcreteStep::Skill(skill) = &mut run.steps[1] else {
                unreachable!();
            };
            skill.policy.human = true;
            skill.policy.repeat = None;
            Ok(())
        })
        .unwrap();
        let (_, _) = begin_boundary(&run.id).unwrap();
        let waiting = read(&run.id).unwrap().cursor;
        update(&run.id, |run| {
            let boundary = run.active.as_mut().unwrap();
            boundary.ready_summary = Some("implement the revised design".into());
            boundary.completed = true;
            Ok(())
        })
        .unwrap();
        checkpoint(&run.id, &waiting).unwrap();
        let saved = read(&run.id).unwrap();
        assert!(saved.cursor.progress.verdict.is_none());
        assert_eq!(saved.cursor, waiting);
        let boundary = saved.active.unwrap();
        assert!(boundary.completed);
        assert_eq!(
            boundary.ready_summary.as_deref(),
            Some("implement the revised design")
        );
    }
    #[test]
    fn routing_is_run_owned_and_recovers_only_after_success() {
        let _lock = crate::journal::test_env_lock();
        let home = Home::new();
        let steps = [ConcreteStep::Xor(crate::engine::ConcreteXor {
            router: Skill::named("xor-route"),
            paths: [
                (
                    "chosen".into(),
                    crate::engine::ConcretePath {
                        description: "work".into(),
                        steps: vec![step("work", None)],
                    },
                ),
                (
                    "empty".into(),
                    crate::engine::ConcretePath {
                        description: "skip".into(),
                        steps: vec![],
                    },
                ),
            ]
            .into(),
            flow_parents: vec![],
        })];
        for status in ["completed", "failed", "interrupted"] {
            let run = create("route", &steps, home.dir.path(), None, &Cli::default()).unwrap();
            let other = create("other", &steps, home.dir.path(), None, &Cli::default()).unwrap();
            let (token, _) = begin_boundary(&run.id).unwrap();
            let (other_token, _) = begin_boundary(&other.id).unwrap();
            std::env::set_var(FLOW_STEP_ENV, serde_json::to_string(&token).unwrap());
            let capture = home.capture();
            bind_run(&capture.run_id(), &capture.artifact_dir()).unwrap();
            super::record_route(&token, &capture.run_id(), "chosen").unwrap();
            super::record_route(&token, &capture.run_id(), "chosen").unwrap();
            assert!(super::record_route(&token, &RunId::new(), "chosen").is_err());
            assert!(super::record_route(&other_token, &capture.run_id(), "chosen").is_err());
            assert!(super::record_route(&token, &capture.run_id(), "unknown").is_err());
            assert!(super::record_route(&token, &capture.run_id(), "empty").is_err());
            // A stale waiting checkpoint cannot erase the committed candidate.
            checkpoint(&run.id, &ExecutionCursor::default()).unwrap();
            assert_eq!(
                read(&run.id).unwrap().cursor.route.as_deref(),
                Some("chosen")
            );
            assert!(recover(&run.id).is_err());
            capture.finish(status).unwrap();
            recover(&run.id).unwrap();
            let saved = read(&run.id).unwrap();
            if status == "completed" {
                let mut next = saved.cursor;
                assert!(!next.finish(&saved.steps).unwrap());
                checkpoint(&run.id, &next).unwrap();
                assert!(super::record_route(&token, &capture.run_id(), "chosen").is_err());
                let saved = read(&run.id).unwrap();
                assert_eq!(
                    crate::engine::current_skill(&saved.steps, &saved.cursor)
                        .unwrap()
                        .skill
                        .name,
                    "work"
                );
            } else {
                assert!(saved.cursor.route.is_none());
                assert!(saved.failure.is_some());
            }
            assert!(read(&other.id).unwrap().cursor.route.is_none());
        }
    }
}
