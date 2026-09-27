//! Human review of a saved Flow. The review is a Session whose Runs name the
//! Flow's invocation; the saved position keeps only the Flow's execution.
use anyhow::{anyhow, bail, ensure, Context, Result};

use crate::engine::{ConcreteSkill, ConcreteStep, Skill};
use crate::ops::flow_run::{self, FlowRun, StepToken};
use crate::ops::human_session;
use crate::run_record::{RunFlowMembership, RunFlowStep};
use crate::session::{Run, Session, SessionKind, TitleSource, WorkSource};
use crate::store::SharedStore;

fn session_id(token: &StepToken) -> String {
    format!("flow:{}:{}", token.invocation, token.boundary)
}

pub(crate) fn token(session_id: &str) -> Result<StepToken> {
    let (invocation, boundary) = session_id
        .strip_prefix("flow:")
        .and_then(|rest| rest.split_once(':'))
        .ok_or_else(|| anyhow!("Session {session_id} is not a saved Flow's review"))?;
    Ok(StepToken {
        invocation: invocation.to_string(),
        boundary: boundary.to_string(),
    })
}

/// The saved Flow, while it waits at this review.
fn waiting(token: &StepToken) -> Result<FlowRun> {
    let flow = flow_run::read(&token.invocation)?;
    ensure!(
        !flow.finished
            && flow.is_human()?
            && flow
                .active
                .as_ref()
                .is_some_and(|boundary| boundary.id == token.boundary && !boundary.completed),
        "Flow Session is stale or already decided"
    );
    Ok(flow)
}

fn current_skill(flow: &FlowRun) -> Result<&ConcreteSkill> {
    match flow.current_step()? {
        ConcreteStep::Skill(skill) => Ok(skill),
        _ => bail!("saved Flow position is not a human skill"),
    }
}

pub(crate) fn pinned_skill(token: &StepToken, requested: &str) -> Result<Skill> {
    let flow = waiting(token)?;
    let skill = &current_skill(&flow)?.skill;
    ensure!(
        skill.name == requested,
        "human Flow Session Skill does not match requested Skill"
    );
    Ok(skill.clone())
}

pub(crate) fn membership(token: &StepToken) -> Result<RunFlowMembership> {
    Ok(RunFlowMembership::Step(RunFlowStep::of_flow(&waiting(
        token,
    )?)?))
}

/// Store the review the Flow waits at, with its first Run prepared. Returns
/// the feedback once the review is complete.
pub(crate) async fn reserve(store: &SharedStore, token: &StepToken) -> Result<Option<String>> {
    let id = session_id(token);
    let session = match store.session(&id).await? {
        Some((session, _)) => session,
        None => {
            let flow = waiting(token)?;
            let skill = current_skill(&flow)?.skill.name.clone();
            let config = crate::engine::config::load_config(Some(&flow.cwd))?.unwrap_or_default();
            let (provider, model) =
                crate::engine::config::parse_agent(flow.model.as_deref().unwrap_or(config.agent()));
            let selector = flow
                .as_work
                .clone()
                .or(flow.task.as_ref().map(|id| format!("task:{id}")))
                .or(flow.wave.as_ref().map(|id| format!("wave:{id}")));
            // The invocation has no Task, so its Runs carry the Work's Wave.
            let wave_id = match selector {
                Some(selector) => crate::ops::resolve_work_binding(store, &flow.cwd, &selector)
                    .await
                    .ok()
                    .map(|binding| binding.wave_id),
                None => None,
            };
            let run = human_session::prepare_run(
                Run {
                    id: crate::durable::RunId::new(),
                    session_id: Some(id.clone()),
                    invocation_id: Some(flow.id.clone()),
                    node: None,
                    iterations: None,
                    attempt: None,
                    task_id: None,
                    work_source: wave_id.as_ref().map(|_| WorkSource::Declared),
                    wave_id,
                    created_at: 0,
                    published: true,
                    cwd: flow.cwd.clone(),
                    skill: Some(skill.clone()),
                    provider: Some(provider.to_string()),
                    model,
                    caller_run_id: None,
                },
                RunFlowMembership::Step(RunFlowStep::of_flow(&flow)?),
            )?;
            let session = Session {
                id,
                current_run_id: run.id.clone(),
                kind: SessionKind::FlowReview,
                title: skill,
                title_source: TitleSource::Generated,
                request: None,
                ready_summary: None,
                completed_at: None,
                created_at: run.created_at,
            };
            let invocation = crate::engine::invocation::QueuedInvocation {
                id: flow.id,
                flow: flow.flow,
                steps: flow.steps,
            };
            store
                .create_session(session, run, Some((invocation, flow.cursor)))
                .await?
                .0
        }
    };
    Ok(session.completed_at.and(session.ready_summary))
}

/// Close the review and hand its feedback to the Flow's next step.
pub(crate) async fn complete(store: &SharedStore, session: &Session, run: &Run) -> Result<()> {
    let token = token(&session.id)?;
    ensure!(
        waiting(&token)?.failure.is_none(),
        "Flow is blocked; resolve its failure first"
    );
    store.complete_session(&session.id, &run.id).await?;
    let launch = flow_run::launch_driver(&token.invocation).await;
    if let Err(error) = human_session::stop_run(&run.id) {
        tracing::warn!(run_id = %run.id, %error, "review completed but provider cleanup failed");
    }
    launch.with_context(|| {
        format!(
            "Review feedback saved; continue with `lf flow resume {}`",
            token.invocation
        )
    })
}

fn review_message(flow: &FlowRun, token: &StepToken) -> String {
    let id = session_id(token);
    let mut message = flow.message.clone().unwrap_or_default();
    if let Some(direction) = &flow.cursor.leaf().progress.direction {
        message.push_str(&format!("\n\nPrevious direction:\n{direction}"));
    }
    message.push_str(&format!(
        "\n\nThis is the interactive review step of saved Flow {}. Work with the human on the experience and finish any design clarification before ending the review.\n\
         Save self-contained, topic-named notes in scratch/ with the human feedback, agreed design changes, unresolved questions, and next useful action. Link the current design and evidence. Put the exact note paths and a short takeaway in the ready summary, then run `lf session ready \"feedback, design changes, and remaining work\"`.\n\
         The human completes this review with `lf session complete {id}`. Completion returns the saved feedback to the next Flow step; it does not choose Advance or Iterate. A following loop-decide step interprets the feedback and owns navigation.", flow.flow));
    message
}

/// The review's launch: the Flow's own selectors, its pinned Skill and step.
pub(crate) fn launch(command: &mut tokio::process::Command, token: &StepToken) -> Result<()> {
    let flow = waiting(token)?;
    command
        .arg("--tui")
        .env(flow_run::FLOW_STEP_ENV, serde_json::to_string(token)?);
    for (flag, value) in [
        ("--model", &flow.model),
        ("--wave", &flow.wave),
        ("--task", &flow.task),
        ("--as", &flow.as_work),
    ] {
        if let Some(value) = value {
            command.args([flag, value]);
        }
    }
    command.args([
        "skill",
        &current_skill(&flow)?.skill.name,
        &review_message(&flow, token),
    ]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{complete, pinned_skill, reserve, review_message, session_id, token};
    use crate::durable::RunId;
    use crate::engine::flow::{ConcretePath, ConcreteSkill, ConcreteStep, ConcreteXor};
    use crate::engine::{ExecutionCursor, NestedCursor, OccurrencePolicy, Skill};
    use crate::ops::flow_run;
    use crate::store::{open_ephemeral_store, StorageConfig};

    #[test]
    fn nested_review_is_one_session_and_returns_its_feedback() {
        let _lock = crate::journal::test_env_lock();
        let home = tempfile::tempdir().unwrap();
        let previous = std::env::var_os("LF_HOME");
        std::env::set_var("LF_HOME", home.path());
        let mut demo = Skill::named("demo");
        demo.content = Some("Pinned demo instructions".into());
        let steps = vec![ConcreteStep::Xor(ConcreteXor {
            router: Skill::named("xor-route"),
            paths: HashMap::from([(
                "chosen".into(),
                ConcretePath {
                    steps: vec![ConcreteStep::Skill(ConcreteSkill {
                        skill: demo,
                        policy: OccurrencePolicy {
                            id: Some("demo".into()),
                            human: true,
                            repeat: None,
                        },
                        flow_parents: Vec::new(),
                    })],
                    description: "saved review".into(),
                },
            )]),
            flow_parents: Vec::new(),
        })];
        let cli = crate::lf::Cli::default();
        let flow = flow_run::create("review", &steps, home.path(), None, &cli).unwrap();
        flow_run::update(&flow.id, |flow| {
            flow.cursor.child = Some(Box::new(NestedCursor::Xor {
                selected: "chosen".into(),
                cursor: ExecutionCursor::default(),
            }));
            Ok(())
        })
        .unwrap();
        let (step, _) = flow_run::begin_boundary(&flow.id).unwrap();
        let id = session_id(&step);
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = std::sync::Arc::new(
                open_ephemeral_store(&StorageConfig::sqlite(home.path().join("registry.db")))
                    .await
                    .unwrap(),
            );
            assert_eq!(reserve(&store, &step).await.unwrap(), None);
            assert_eq!(reserve(&store, &step).await.unwrap(), None);
            let (session, run) = store.session(&id).await.unwrap().unwrap();
            assert_eq!(token(&session.id).unwrap(), step);
            assert_eq!(
                store.session_runs(&id).await.unwrap(),
                std::slice::from_ref(&run)
            );
            assert_eq!(
                (run.invocation_id.as_deref(), &run.task_id),
                (Some(flow.id.as_str()), &None)
            );
            assert_eq!(
                store.waiting_flow(&id).await.unwrap(),
                Some(("review".to_string(), "0/chosen/0".to_string()))
            );
            assert_eq!(
                pinned_skill(&step, "demo").unwrap().content.as_deref(),
                Some("Pinned demo instructions")
            );
            let saved = flow_run::read(&flow.id).unwrap();
            assert!(review_message(&saved, &step).contains(&format!("lf session complete {id}")));

            assert!(complete(&store, &session, &run).await.is_err());
            assert!(store
                .ready_session(&id, &RunId::new(), "wrong Run")
                .await
                .is_err());
            store
                .ready_session(&id, &run.id, "Use the revised design")
                .await
                .unwrap();
            complete(&store, &session, &run).await.unwrap();
            assert!(complete(&store, &session, &run).await.is_err());
            assert!(store
                .ready_session(&id, &run.id, "late rewrite")
                .await
                .is_err());
            assert_eq!(
                reserve(&store, &step).await.unwrap().as_deref(),
                Some("Use the revised design")
            );
            assert!(store.open_sessions().await.unwrap().is_empty());
            assert_eq!(flow_run::read(&flow.id).unwrap().cursor, saved.cursor);
        });
        match previous {
            Some(value) => std::env::set_var("LF_HOME", value),
            None => std::env::remove_var("LF_HOME"),
        }
    }
}
