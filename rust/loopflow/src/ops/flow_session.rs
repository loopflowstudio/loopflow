//! Human review of an ordinary saved Flow. The Flow record owns the boundary,
//! readiness, provider binding, and completion; Session records are projections.
use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, bail, ensure, Context, Result};

use crate::durable::RunId;
use crate::engine::{ConcreteSkill, ConcreteStep, Skill};
use crate::ops::flow_run::{self, FlowRun, StepToken};
use crate::ops::human_session::{self, HumanSessionToken, OpenMode, SessionKind, SessionRecord};

pub(crate) fn session_id(token: &StepToken) -> String {
    format!("flow:{}:{}", token.invocation, token.boundary)
}

pub(crate) fn parse_id(id: &str) -> Result<Option<StepToken>> {
    let Some(rest) = id.strip_prefix("flow:") else {
        return Ok(None);
    };
    let (invocation, boundary) = rest
        .split_once(':')
        .ok_or_else(|| anyhow!("invalid standalone Flow Session id"))?;
    uuid::Uuid::parse_str(invocation).context("invalid Flow invocation id")?;
    uuid::Uuid::parse_str(boundary).context("invalid Flow boundary id")?;
    Ok(Some(StepToken {
        invocation: invocation.to_string(),
        boundary: boundary.to_string(),
    }))
}

fn validate<'a>(run: &'a FlowRun, token: &StepToken) -> Result<&'a flow_run::Boundary> {
    ensure!(
        run.id == token.invocation && !run.finished,
        "Flow Session is stale"
    );
    let boundary = run
        .active
        .as_ref()
        .ok_or_else(|| anyhow!("Flow Session is no longer waiting"))?;
    ensure!(
        boundary.id == token.boundary && !boundary.completed,
        "Flow Session is stale or already decided"
    );
    ensure!(
        run.is_human()?,
        "Flow Session does not match the saved human boundary"
    );
    Ok(boundary)
}

fn current_skill(run: &FlowRun) -> Result<&ConcreteSkill> {
    match run.current_step()? {
        ConcreteStep::Skill(skill) => Ok(skill),
        _ => bail!("saved Flow position is not a human skill"),
    }
}

pub(crate) fn pinned_skill(token: &StepToken, requested: &str) -> Result<Skill> {
    let run = flow_run::read(&token.invocation)?;
    validate(&run, token)?;
    let skill = &current_skill(&run)?.skill;
    ensure!(
        skill.name == requested,
        "human Flow Session Skill does not match requested Skill"
    );
    Ok(skill.clone())
}

pub(crate) fn worktree(token: &StepToken) -> Result<PathBuf> {
    let run = flow_run::read(&token.invocation)?;
    validate(&run, token)?;
    Ok(run.cwd)
}

pub(crate) fn list() -> Result<Vec<SessionRecord>> {
    let mut sessions = reviews()?
        .iter()
        .map(|(run, token)| session_surface(run, token))
        .collect::<Result<Vec<_>>>()?;
    sessions.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(sessions)
}

/// Every saved Flow waiting at a human review, with the token naming it.
pub(crate) fn reviews() -> Result<Vec<(FlowRun, StepToken)>> {
    let directory = crate::store::current_home_lf_home_dir().join("flows");
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).context("list saved Flow Sessions"),
    };
    let mut reviews = Vec::new();
    for entry in entries {
        let entry = entry?;
        let id = entry.file_name().to_string_lossy().into_owned();
        if uuid::Uuid::parse_str(&id).is_err() || !entry.path().join("position.json").exists() {
            continue;
        }
        let run = match flow_run::read(&id) {
            Ok(run) => run,
            Err(error) => {
                tracing::warn!(invocation = %id, error = %format!("{error:#}"),
                    "cannot list this saved Flow Session; other Sessions remain available");
                continue;
            }
        };
        let Some(boundary) = &run.active else {
            continue;
        };
        if !run.finished && !boundary.completed && run.is_human()? {
            let token = StepToken {
                invocation: id,
                boundary: boundary.id.clone(),
            };
            reviews.push((run, token));
        }
    }
    Ok(reviews)
}

pub(crate) fn surface(token: &StepToken) -> Result<SessionRecord> {
    let run = flow_run::read(&token.invocation)?;
    session_surface(&run, token)
}

pub(crate) fn prepare_run(run: &mut FlowRun) -> Result<()> {
    if !run.is_human()? || run.active.as_ref().is_none_or(|b| b.run_id.is_some()) {
        return Ok(());
    }
    let config = crate::engine::config::load_config(Some(&run.cwd))?.unwrap_or_default();
    let (harness, model) =
        crate::engine::config::parse_agent(run.model.as_deref().unwrap_or(config.agent()));
    let subjects = run
        .as_work
        .clone()
        .into_iter()
        .chain(run.task.as_ref().map(|id| format!("task:{id}")))
        .chain(run.wave.as_ref().map(|id| format!("wave:{id}")))
        .map(crate::run_record::SubjectAttribution::declared)
        .collect();
    let id = crate::run_record::CaptureHandle::prepare(
        crate::run_record::RunSpec {
            harness: harness.to_string(),
            model,
            surface: "tui".into(),
            cwd: run.cwd.clone(),
            repo: None,
            worktree: Some(run.cwd.clone()),
            skill: Some(current_skill(run)?.skill.name.clone()),
            subjects,
            flow: crate::run_record::RunFlowMembership::Step(
                crate::run_record::RunFlowStep::of_flow(run)?,
            ),
        },
        None,
    )?;
    let (dir, _) =
        crate::run_record::resolve_manifest(&crate::store::observability_home_dir(), id.as_str())?;
    let boundary = run
        .active
        .as_mut()
        .expect("human boundary exists before preparation");
    boundary.run_id = Some(id);
    boundary.run_dir = Some(dir);
    Ok(())
}

fn session_surface(run: &FlowRun, token: &StepToken) -> Result<SessionRecord> {
    let boundary = validate(run, token)?;
    let id = session_id(token);
    let run_id = boundary.run_id.clone().ok_or_else(|| {
        anyhow!("Session {id} needs Run preparation; run `lf session open '{id}' --json`")
    })?;
    let (dir, _) = crate::run_record::resolve_manifest(
        &crate::store::observability_home_dir(),
        run_id.as_str(),
    )?;
    let name = human_session::session_name(Some(&dir), current_skill(run)?.skill.name.clone())?;
    let state =
        human_session::native_session_state(Some(&run_id), boundary.ready_summary.as_deref())?;
    Ok(SessionRecord {
        open_argv: human_session::human_open_argv(None, Some(&run.cwd), &id)?,
        id,
        kind: SessionKind::Flow,
        run_id,
        work_path: None,
        actions: human_session::session_actions(SessionKind::Flow, state),
        terminal_ids: Vec::new(),
        work: None,
        wave_id: None,
        title: name.title,
        title_source: name.source,
        flow_membership: human_session::SessionFlowMembership::Step {
            flow: run.flow.clone(),
            invocation_id: run.id.clone(),
            step: current_skill(run)?.skill.name.clone(),
            step_index: run.cursor.leaf().index as u32,
            iteration: run.cursor.iteration,
            occurrence: human_session::SessionFlowOccurrence::Current,
        },
        detail: current_skill(run)?.skill.name.clone(),
        cwd: run.cwd.display().to_string(),
        state,
        ready_summary: boundary.ready_summary.clone(),
    })
}

pub(crate) fn mark_ready(token: &StepToken, run_id: &RunId, summary: &str) -> Result<()> {
    ensure!(!summary.trim().is_empty(), "ready summary cannot be empty");
    flow_run::update(&token.invocation, |run| {
        let boundary = validate(run, token)?;
        ensure!(
            boundary.run_id.as_ref() == Some(run_id),
            "readiness belongs to another Flow Run"
        );
        run.active
            .as_mut()
            .expect("validated active boundary")
            .ready_summary = Some(summary.trim().to_string());
        Ok(())
    })
}

fn record_completion(token: &StepToken) -> Result<Option<RunId>> {
    flow_run::update(&token.invocation, |run| {
        let boundary = validate(run, token)?;
        ensure!(
            run.failure.is_none(),
            "Flow is blocked; resolve its failure first"
        );
        ensure!(
            boundary
                .ready_summary
                .as_ref()
                .is_some_and(|s| !s.trim().is_empty()),
            "review is not ready; its agent must record the feedback with `lf session ready` first"
        );
        let run_id = boundary.run_id.clone();
        run.active
            .as_mut()
            .expect("validated active boundary")
            .completed = true;
        Ok(run_id)
    })
}

pub(crate) async fn complete(token: &StepToken) -> Result<()> {
    let run_id = record_completion(token)?;
    let launch = flow_run::launch_driver(&token.invocation).await;
    if let Some(run_id) = run_id {
        if let Err(error) = human_session::stop_run(&run_id) {
            tracing::warn!(%run_id, %error, "review completed but provider cleanup failed");
        }
    }
    launch.with_context(|| {
        format!(
            "Review feedback saved; continue with `lf flow resume {}`",
            token.invocation
        )
    })
}

fn review_message(run: &FlowRun, token: &StepToken) -> String {
    let id = session_id(token);
    let mut message = run.message.clone().unwrap_or_default();
    if let Some(direction) = &run.cursor.leaf().progress.direction {
        message.push_str(&format!("\n\nPrevious direction:\n{direction}"));
    }
    message.push_str(&format!(
        "\n\nThis is the interactive review step of saved Flow {}. Work with the human on the experience and finish any design clarification before ending the review.\n\
         Save self-contained, topic-named notes in scratch/ with the human feedback, agreed design changes, unresolved questions, and next useful action. Link the current design and evidence. Put the exact note paths and a short takeaway in the ready summary, then run `lf session ready \"feedback, design changes, and remaining work\"`.\n\
         The human completes this review with `lf session complete {id}`. Completion returns the saved feedback to the next Flow step; it does not choose Advance or Iterate. A following loop-decide step interprets the feedback and owns navigation.", run.flow));
    message
}

/// Called under the Session launch lock. A Run can bind before its provider
/// publishes native history; a failed launch must not strand the review.
fn recover_unpublished_run(token: &StepToken) -> Result<()> {
    flow_run::update(&token.invocation, |run| {
        let boundary = validate(run, token)?;
        let Some(run_id) = &boundary.run_id else {
            return Ok(());
        };
        let (dir, manifest) = crate::run_record::resolve_manifest(
            &crate::store::observability_home_dir(),
            run_id.as_str(),
        )?;
        if human_session::run_is_prepared(run_id)?
            || crate::run_record::read_provider_session(&dir)?.is_some()
        {
            return Ok(());
        }
        ensure!(
            crate::lf::commands::util::active_provider_clients(&dir, &manifest.harness)?.is_empty(),
            "Flow Session provider is still starting; reopen after it becomes resumable"
        );
        let previous = run_id.clone();
        let boundary = run.active.as_mut().expect("validated human boundary");
        boundary.run_id = None;
        boundary.run_dir = None;
        boundary.ready_summary = None;
        prepare_run(run)?;
        human_session::carry_session_name(
            &session_id(token),
            Some(&previous),
            run.active.as_ref().and_then(|b| b.run_id.as_ref()),
        )?;
        Ok(())
    })
}

pub(crate) async fn open(token: &StepToken, mode: OpenMode, resume: bool) -> Result<SessionRecord> {
    ensure!(
        mode == OpenMode::Refuse,
        "--replace and --try apply only to interactive provider sessions"
    );
    let id = session_id(token);
    let lock =
        tokio::task::spawn_blocking(move || human_session::lock_session_launch(&id)).await??;
    recover_unpublished_run(token)?;
    flow_run::update(&token.invocation, prepare_run)?;
    let surface = surface(token)?;
    if !resume {
        return Ok(surface);
    }
    let run = flow_run::read(&token.invocation)?;
    let boundary = validate(&run, token)?;
    let human_token = HumanSessionToken::StandaloneFlow {
        token: token.clone(),
    };
    if let Some(run_id) = boundary.run_id.as_ref() {
        if !human_session::run_is_prepared(run_id)? {
            // Retain a published native identity even if history is temporarily absent.
            // The launch lock guards startup only, not the whole interactive session.
            let run_id = run_id.clone();
            drop(lock);
            ensure!(
                human_session::resume_native_run(&run_id, &human_token)?,
                "Flow Session {0} has Run {run_id} but native history is unavailable",
                session_id(token)
            );
            return Ok(surface);
        }
    }
    let skill = &current_skill(&run)?.skill;
    let lf = crate::engine::process::resolve_current_home_lf_binary_checked()?;
    let mut command = tokio::process::Command::new(lf);
    command
        .arg("--tui")
        .current_dir(&run.cwd)
        .env(
            human_session::HUMAN_SESSION_ENV,
            serde_json::to_string(&human_token)?,
        )
        .env(flow_run::FLOW_STEP_ENV, serde_json::to_string(token)?);
    for (flag, value) in [
        ("--model", &run.model),
        ("--wave", &run.wave),
        ("--task", &run.task),
        ("--as", &run.as_work),
    ] {
        if let Some(value) = value {
            command.args([flag, value]);
        }
    }
    command.args(["skill", &skill.name, &review_message(&run, token)]);
    let run_id = boundary
        .run_id
        .clone()
        .expect("human Flow Run was prepared");
    let mut child = human_session::spawn_session_run(&mut command, &run_id).await?;
    let binding = flow_run::update(&token.invocation, |current| {
        let boundary = validate(current, token)?;
        ensure!(
            boundary.run_id.as_ref().is_none_or(|id| id == &run_id),
            "Flow Session already owns another Run"
        );
        let (dir, _) = crate::run_record::resolve_manifest(
            &crate::store::observability_home_dir(),
            run_id.as_str(),
        )?;
        let boundary = current.active.as_mut().expect("validated active boundary");
        boundary.run_id = Some(run_id);
        boundary.run_dir = Some(dir);
        Ok(())
    });
    if let Err(error) = binding {
        let _ = child.kill().await;
        return Err(error);
    }
    drop(lock);
    let status = child.wait().await.context("wait for human Flow Session")?;
    // Provider termination never completes a review.
    if !status.success()
        && flow_run::read(&token.invocation)?
            .active
            .as_ref()
            .is_some_and(|boundary| boundary.id == token.boundary && !boundary.completed)
    {
        bail!("human Flow Session exited with {status}; its review is still waiting");
    }
    Ok(surface)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::ffi::OsString;
    use std::fs;

    use clap::Parser;

    use super::{
        list, mark_ready, parse_id, pinned_skill, record_completion, recover_unpublished_run,
        review_message, session_id,
    };
    use crate::durable::RunId;
    use crate::engine::flow::{
        ConcretePath, ConcreteSkill, ConcreteStep, ConcreteXor, OccurrencePolicy, RepeatPolicy,
        Skill,
    };
    use crate::engine::transitions::{FlowDecision, FlowVerdict};
    use crate::engine::{ExecutionCursor, NestedCursor};
    use crate::lf::Cli;
    use crate::ops::flow_run;
    use crate::ops::human_session::SessionState;

    struct TestHome {
        directory: tempfile::TempDir,
        previous: Vec<(&'static str, Option<OsString>)>,
    }

    impl TestHome {
        fn new() -> Self {
            let directory = tempfile::tempdir().unwrap();
            let previous = ["LF_HOME", "LF_BIN"]
                .into_iter()
                .map(|name| (name, std::env::var_os(name)))
                .collect();
            std::env::set_var("LF_HOME", directory.path());
            std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
            Self {
                directory,
                previous,
            }
        }
    }

    impl Drop for TestHome {
        fn drop(&mut self) {
            for (name, value) in &self.previous {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
    }

    struct WaitingExecutor {
        invocation: String,
    }

    #[async_trait::async_trait]
    impl crate::engine::SkillExecutor for WaitingExecutor {
        async fn checkpoint(&self, cursor: &ExecutionCursor) -> anyhow::Result<()> {
            flow_run::checkpoint(&self.invocation, cursor)
        }

        async fn run_skill(
            &self,
            _skill: &ConcreteSkill,
            _ctx: crate::engine::ExecutionContext,
        ) -> anyhow::Result<crate::engine::SkillOutcome> {
            let saved = flow_run::read(&self.invocation)?;
            if saved.is_human()? && saved.active.as_ref().is_some_and(|b| b.completed) {
                let (token, _) = flow_run::begin_boundary(&self.invocation)?;
                flow_run::finish_boundary(&token, None)
            } else {
                Ok(crate::engine::SkillOutcome::Waiting)
            }
        }

        async fn run_op(
            &self,
            _op: &crate::engine::ConcreteOp,
            _ctx: crate::engine::ExecutionContext,
        ) -> anyhow::Result<()> {
            anyhow::bail!("review recovery must not execute an operation")
        }
    }

    fn resume_to_next_step(invocation: &str) -> bool {
        let saved = flow_run::read(invocation).unwrap();
        let mut cursor = saved.cursor.clone();
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(
                crate::engine::FlowEngine::new(WaitingExecutor {
                    invocation: invocation.to_string(),
                })
                .run_with_cursor(&saved.steps, &mut cursor),
            )
            .unwrap();
        cursor != saved.cursor
    }

    fn skill(name: &str, human: bool, repeat: Option<RepeatPolicy>) -> ConcreteStep {
        let mut skill = Skill::named(name);
        skill.content = Some(format!("Pinned {name} instructions"));
        ConcreteStep::Skill(ConcreteSkill {
            skill,
            policy: OccurrencePolicy {
                id: Some(name.to_string()),
                human,
                repeat,
            },
            flow_parents: Vec::new(),
        })
    }

    struct NativeClient(std::process::Child);

    impl Drop for NativeClient {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[test]
    fn failed_human_launch_can_reopen_without_losing_published_history_or_feedback() {
        let _lock = crate::journal::test_env_lock();
        let home = TestHome::new();
        let cli = Cli::try_parse_from(["lf"]).unwrap();
        let steps = vec![skill("demo", true, None)];
        for state in ["failed", "starting", "published"] {
            let run =
                flow_run::create("review", &steps, home.directory.path(), None, &cli).unwrap();
            let (token, _) = flow_run::begin_boundary(&run.id).unwrap();
            let capture = crate::run_record::CaptureHandle::begin_at(
                home.directory.path(),
                crate::run_record::RunSpec {
                    harness: "sleep".into(),
                    model: None,
                    surface: "tui".into(),
                    cwd: home.directory.path().into(),
                    repo: None,
                    worktree: None,
                    skill: Some("demo".into()),
                    subjects: Vec::new(),
                    flow: crate::run_record::RunFlowMembership::Independent,
                },
            )
            .unwrap();
            let dir = capture.artifact_dir();
            flow_run::update(&run.id, |run| {
                let boundary = run.active.as_mut().unwrap();
                boundary.run_id = Some(capture.run_id());
                boundary.run_dir = Some(dir.clone());
                Ok(())
            })
            .unwrap();
            let _client = if state == "starting" {
                let child = NativeClient(
                    std::process::Command::new("sleep")
                        .arg("60")
                        .spawn()
                        .unwrap(),
                );
                crate::run_record::write_provider_client(&dir, child.0.id()).unwrap();
                Some(child)
            } else {
                None
            };
            if state == "published" {
                crate::run_record::write_provider_session(&dir, "native-review", None).unwrap();
            }
            crate::run_record::write_session_name(
                &dir,
                "Delivery review",
                crate::run_record::SessionTitleSource::Human,
            )
            .unwrap();
            let manifest = fs::read(dir.join("manifest.json")).unwrap();
            let launch_lock =
                crate::ops::human_session::lock_session_launch(&session_id(&token)).unwrap();
            let recovery = recover_unpublished_run(&token);
            let recovered = flow_run::read(&run.id).unwrap();
            let boundary = recovered.active.unwrap();
            assert_eq!(boundary.id, token.boundary);
            assert_eq!(recovered.cursor, run.cursor);
            assert!(!boundary.completed);
            assert_eq!(fs::read(dir.join("manifest.json")).unwrap(), manifest);
            if state == "failed" {
                recovery.unwrap();
                assert_ne!(boundary.run_id, Some(capture.run_id()));
                assert!(crate::ops::human_session::run_is_prepared(
                    boundary.run_id.as_ref().unwrap()
                )
                .unwrap());
                let name = crate::run_record::read_session_name(boundary.run_dir.as_ref().unwrap())
                    .unwrap()
                    .unwrap();
                assert_eq!(name.title, "Delivery review");
                assert_eq!(name.source, crate::run_record::SessionTitleSource::Human);
            } else {
                assert_eq!(boundary.run_id, Some(capture.run_id()));
                assert_eq!(recovery.is_err(), state == "starting");
            }
            mark_ready(&token, boundary.run_id.as_ref().unwrap(), "reviewed").unwrap();
            record_completion(&token).unwrap();
            assert!(recover_unpublished_run(&token).is_err());
            assert!(flow_run::read(&run.id).unwrap().active.unwrap().completed);
            drop(launch_lock);
        }
    }

    #[test]
    fn unreadable_flow_does_not_hide_another_human_review() {
        let _lock = crate::journal::test_env_lock();
        let home = TestHome::new();
        let cli = Cli::try_parse_from(["lf"]).unwrap();
        let steps = vec![skill("demo", true, None)];
        let valid = flow_run::create("feature", &steps, home.directory.path(), None, &cli).unwrap();
        let (token, _) = flow_run::begin_boundary(&valid.id).unwrap();
        let old =
            flow_run::create("old-feature", &steps, home.directory.path(), None, &cli).unwrap();
        let path = crate::store::current_home_lf_home_dir()
            .join("flows")
            .join(&old.id)
            .join("position.json");
        let mut json = serde_json::to_value(&old).unwrap();
        json["steps"] = serde_json::json!([{"Xor": {
            "router": null, "flow_parents": [],
            "paths": {"old": {"flow": "deleted", "skill": null, "description": "Old"}}
        }}]);
        let bytes = serde_json::to_vec(&json).unwrap();
        fs::write(&path, &bytes).unwrap();

        let sessions = list().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, session_id(&token));
        let error = format!("{:#}", flow_run::read(&old.id).unwrap_err());
        assert!(error.contains(&path.display().to_string()), "{error}");
        assert!(error.contains("lf flow"), "{error}");
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }

    #[test]
    fn saved_review_feedback_reaches_the_outer_decision_once() {
        let _lock = crate::journal::test_env_lock();
        let home = TestHome::new();
        let child = vec![
            skill("implement", false, None),
            skill(
                "inner",
                false,
                Some(RepeatPolicy {
                    from: "implement".into(),
                }),
            ),
            skill("demo", true, None),
            skill(
                "outer",
                false,
                Some(RepeatPolicy {
                    from: "implement".into(),
                }),
            ),
        ];
        let steps = vec![
            ConcreteStep::Xor(ConcreteXor {
                router: Skill::named("xor-route"),
                paths: HashMap::from([(
                    "chosen".into(),
                    ConcretePath {
                        steps: child,
                        description: "saved double loop".into(),
                    },
                )]),
                flow_parents: Vec::new(),
            }),
            skill("delivery", false, None),
        ];
        let cli = Cli::try_parse_from(["lf"]).unwrap();
        let run = flow_run::create("review", &steps, home.directory.path(), None, &cli).unwrap();
        flow_run::update(&run.id, |run| {
            run.cursor.child = Some(Box::new(NestedCursor::Xor {
                selected: "chosen".into(),
                cursor: ExecutionCursor {
                    index: 2,
                    ..ExecutionCursor::default()
                },
            }));
            Ok(())
        })
        .unwrap();
        let (token, _) = flow_run::begin_boundary(&run.id).unwrap();
        assert_eq!(parse_id(&session_id(&token)).unwrap(), Some(token.clone()));
        assert!(parse_id("flow:bad:bad").is_err());
        assert_eq!(
            pinned_skill(&token, "demo").unwrap().content.as_deref(),
            Some("Pinned demo instructions")
        );
        let prompt = review_message(&flow_run::read(&run.id).unwrap(), &token);
        assert!(prompt.contains(&format!("lf session complete {}", session_id(&token))));
        let provider = flow_run::read(&run.id)
            .unwrap()
            .active
            .unwrap()
            .run_id
            .unwrap();
        assert!(record_completion(&token).is_err());
        assert!(mark_ready(&token, &RunId::new(), "wrong Run").is_err());
        let before = flow_run::read(&run.id).unwrap().cursor;
        mark_ready(&token, &provider, "Use the revised interaction design").unwrap();
        assert_eq!(list().unwrap()[0].state, SessionState::Ready);
        assert!(!resume_to_next_step(&run.id));
        record_completion(&token).unwrap();
        let completed = flow_run::read(&run.id).unwrap();
        assert_eq!(completed.cursor, before);
        assert!(completed.cursor.leaf().progress.verdict.is_none());
        assert!(list().unwrap().is_empty());
        assert!(record_completion(&token).is_err());
        assert!(mark_ready(&token, &provider, "late rewrite").is_err());
        assert_eq!(
            serde_json::to_value(flow_run::read(&run.id).unwrap()).unwrap(),
            serde_json::to_value(completed).unwrap()
        );
        assert!(resume_to_next_step(&run.id));
        assert!(!resume_to_next_step(&run.id));
        let saved = flow_run::read(&run.id).unwrap();
        assert_eq!(saved.cursor.leaf().index, 3);
        assert_eq!(
            saved.cursor.leaf().progress.direction.as_deref(),
            Some("Use the revised interaction design")
        );
        assert!(saved.cursor.leaf().progress.verdict.is_none());
        assert_eq!(saved.cursor.iteration, 0);
        let (decision, _) = flow_run::begin_boundary(&run.id).unwrap();
        let decider = RunId::new();
        flow_run::update(&run.id, |run| {
            run.active.as_mut().unwrap().run_id = Some(decider.clone());
            Ok(())
        })
        .unwrap();
        flow_run::record_decision(
            &decision,
            &decider,
            &FlowVerdict {
                decision: FlowDecision::Iterate,
                summary: "Implement the revised design and prove it".into(),
            },
        )
        .unwrap();
        flow_run::finish_boundary(&decision, None).unwrap();
        assert!(resume_to_next_step(&run.id));
        let saved = flow_run::read(&run.id).unwrap();
        assert_eq!(saved.cursor.leaf().index, 0);
        assert_eq!(
            saved.cursor.leaf().progress.direction.as_deref(),
            Some("Implement the revised design and prove it")
        );
        assert_eq!(saved.cursor.iteration, 1);
        assert!(record_completion(&token).is_err());
    }
}
