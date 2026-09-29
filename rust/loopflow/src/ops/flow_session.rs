//! Human review of a saved Flow. The review is a Session whose Runs name the
//! Flow's invocation and the Flow's Work, Task included; the invocation row
//! waits on it through `pending_session_id`.
use anyhow::{anyhow, bail, ensure, Context, Result};

use crate::durable::FlowSession;
use crate::engine::{ConcreteSkill, ConcreteStep, Skill};
use crate::ops::flow_run::{self, ActiveStep};
use crate::ops::human_session;
use crate::run_record::{RunFlowMembership, RunFlowStep};
use crate::session::{AgentSession, SessionKind, TitleSource, WorkSource};
use crate::store::SharedStore;

fn current_skill(flow: &FlowSession) -> Result<&ConcreteSkill> {
    match flow.current_step() {
        Some(ConcreteStep::Skill(skill)) if skill.policy.human => Ok(skill),
        _ => bail!("saved Flow position is not a human skill"),
    }
}

/// The Skill a review's agent runs: the one captured at the waiting step.
pub(crate) fn pinned_skill(session_id: &str, requested: &str) -> Result<Skill> {
    let store =
        crate::store::sqlite::SqliteStore::new(&crate::store::observability_database_path()?)?;
    let flow = store.waiting_review(session_id)?;
    let skill = &current_skill(&flow)?.skill;
    ensure!(
        skill.name == requested,
        "human Flow Session Skill does not match requested Skill"
    );
    Ok(skill.clone())
}

pub(crate) async fn membership(store: &SharedStore, session_id: &str) -> Result<RunFlowMembership> {
    Ok(RunFlowMembership::Step(RunFlowStep::of(
        &store.waiting_review(session_id).await?,
    )?))
}

/// Store the review the Flow waits at, with its first Run prepared. Returns
/// the feedback once the review is complete.
pub(crate) async fn reserve(store: &SharedStore, flow: &FlowSession) -> Result<Option<String>> {
    let session = match &flow.pending_session_id {
        Some(id) => store
            .session(id)
            .await?
            .ok_or_else(|| anyhow!("review Session {id} disappeared"))?,
        None => {
            let skill = current_skill(flow)?.skill.name.clone();
            let config = crate::engine::config::load_config(Some(&flow.cwd))?.unwrap_or_default();
            let (provider, model) =
                crate::engine::config::parse_agent(flow.model.as_deref().unwrap_or(config.agent()));
            let id = format!("session_{}", uuid::Uuid::new_v4().simple());
            let work = flow.declared_work();
            let session = human_session::prepare_input(
                AgentSession {
                    caller_input_id: None,
                    id,
                    input_id: crate::durable::RunId::new(),
                    input_published: true,
                    cwd: flow.cwd.clone(),
                    skill: Some(skill.clone()),
                    provider: Some(provider),
                    model,
                    node: None,
                    iterations: None,
                    task_id: flow.task_id.clone(),
                    wave_id: flow.wave_id.clone(),
                    flow_session_id: Some(flow.id().to_owned()),
                    work_source: work.map(|_| WorkSource::Inherited),
                    bound_at: None,
                    kind: SessionKind::FlowReview,
                    interactive: true,
                    repo: None,
                    title: skill,
                    title_source: TitleSource::Generated,
                    request: None,
                    ready_summary: None,
                    completed_at: None,
                    created_at: crate::store::rows::now_unix(),
                },
                RunFlowMembership::Step(RunFlowStep::of(flow)?),
                None,
            )?;
            store.create_session(session, Some(flow.clone())).await?
        }
    };
    Ok(session.completed_at.and(session.ready_summary))
}

/// Close the review and hand its feedback to the Flow's next step.
pub(crate) async fn complete(store: &SharedStore, session: &AgentSession) -> Result<()> {
    let flow = store.waiting_review(&session.id).await?;
    ensure!(
        flow.failure.is_none(),
        "Flow is blocked; resolve its failure first"
    );
    store
        .complete_session(&session.id, &session.input_id)
        .await?;
    let launch = flow_run::launch_driver(flow.id()).await;
    if let Err(error) = human_session::stop_run(&session.input_id) {
        tracing::warn!(run_id = %session.input_id, %error, "review completed but provider cleanup failed");
    }
    launch.with_context(|| {
        format!(
            "Review feedback saved; continue with `lf flow resume {}`",
            flow.id()
        )
    })
}

fn review_message(flow: &FlowSession, session_id: &str) -> String {
    let mut message = flow.message.clone().unwrap_or_default();
    if let Some(direction) = &flow.cursor.leaf().progress.direction {
        message.push_str(&format!("\n\nPrevious direction:\n{direction}"));
    }
    message.push_str(&format!(
        "\n\nThis is the interactive review step of saved Flow {}. Work with the human on the experience and finish any design clarification before ending the review.\n\
         Save self-contained, topic-named notes in scratch/ with the human feedback, agreed design changes, unresolved questions, and next useful action. Link the current design and evidence. Put the exact note paths and a short takeaway in the ready summary, then run `lf session ready \"feedback, design changes, and remaining work\"`.\n\
         The human completes this review with `lf session complete {session_id}`. Completion returns the saved feedback to the next Flow step; it does not choose Advance or Iterate. A following loop-decide step interprets the feedback and owns navigation.", flow.invocation.flow));
    message
}

/// The review's launch: the Flow's Work, its pinned Skill and step.
pub(crate) async fn launch(
    store: &SharedStore,
    command: &mut tokio::process::Command,
    session_id: &str,
) -> Result<()> {
    let flow = store.waiting_review(session_id).await?;
    let step = ActiveStep {
        invocation: flow.id().to_owned(),
        version: flow.version,
    };
    command
        .arg("--tui")
        .env(flow_run::FLOW_STEP_ENV, serde_json::to_string(&step)?);
    if let Some(model) = &flow.model {
        command.args(["--model", model]);
    }
    match (&flow.task_id, &flow.wave_id) {
        (Some(task), _) => command.args(["--task", task.as_str()]),
        (None, Some(wave)) => command.args(["--wave", wave.as_str()]),
        (None, None) => command,
    };
    command.args([
        "skill",
        &current_skill(&flow)?.skill.name,
        &review_message(&flow, session_id),
    ]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{complete, pinned_skill, reserve, review_message};
    use crate::durable::{FlowSession, RunId};
    use crate::engine::flow::{ConcretePath, ConcreteSkill, ConcreteStep, ConcreteXor};
    use crate::engine::invocation::QueuedInvocation;
    use crate::engine::{ExecutionCursor, NestedCursor, OccurrencePolicy, Skill};
    use crate::store::{open_ephemeral_store, StorageConfig};

    #[test]
    fn nested_review_is_one_session_and_returns_its_feedback() {
        let _lock = crate::journal::test_env_lock();
        let home = tempfile::tempdir().unwrap();
        let previous = [
            ("LF_HOME", std::env::var_os("LF_HOME")),
            ("LF_DB_PATH", std::env::var_os("LF_DB_PATH")),
            ("LF_CONTROL_HOME", std::env::var_os("LF_CONTROL_HOME")),
            ("LF_CONTROL_DB_PATH", std::env::var_os("LF_CONTROL_DB_PATH")),
        ];
        std::env::set_var("LF_HOME", home.path());
        std::env::set_var("LF_DB_PATH", home.path().join("registry.db"));
        std::env::remove_var("LF_CONTROL_HOME");
        std::env::remove_var("LF_CONTROL_DB_PATH");
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
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = std::sync::Arc::new(
                open_ephemeral_store(&StorageConfig::sqlite(home.path().join("registry.db")))
                    .await
                    .unwrap(),
            );
            let flow = store
                .create_flow(FlowSession {
                    invocation: QueuedInvocation::new("review", steps).unwrap(),
                    cursor: ExecutionCursor {
                        child: Some(Box::new(NestedCursor::Xor {
                            selected: "chosen".into(),
                            cursor: ExecutionCursor::default(),
                        })),
                        ..ExecutionCursor::default()
                    },
                    version: 0,
                    task_id: None,
                    wave_id: None,
                    cwd: home.path().to_path_buf(),
                    message: None,
                    model: None,
                    current_attempt: None,
                    pending_session_id: None,
                    ready_summary: None,
                    worker_generation: 0,
                    claim: None,
                    failure: None,
                    finished: false,
                    updated_at: time::OffsetDateTime::now_utc(),
                })
                .await
                .unwrap();
            assert_eq!(reserve(&store, &flow).await.unwrap(), None);
            let waiting = store.flow(flow.id()).await.unwrap().unwrap();
            let id = waiting.pending_session_id.clone().unwrap();
            assert!(id.starts_with("session_"));
            assert_eq!(reserve(&store, &waiting).await.unwrap(), None);
            let session = store.session(&id).await.unwrap().unwrap();
            let run = session.clone();
            assert_eq!(
                store.session_inputs(&id).await.unwrap(),
                std::slice::from_ref(&run.input_id)
            );
            assert_eq!(
                (run.flow_session_id.as_deref(), &run.task_id),
                (Some(flow.id()), &None)
            );
            assert_eq!(
                store.waiting_flow(&id).await.unwrap(),
                Some(("review".to_string(), "0/chosen/0".to_string()))
            );
            assert_eq!(
                pinned_skill(&id, "demo").unwrap().content.as_deref(),
                Some("Pinned demo instructions")
            );
            assert!(review_message(&waiting, &id).contains(&format!("lf session complete {id}")));

            assert!(complete(&store, &session).await.is_err());
            assert!(store
                .ready_session(&id, &RunId::new(), "wrong Run")
                .await
                .is_err());
            store
                .ready_session(&id, &session.input_id, "Use the revised design")
                .await
                .unwrap();
            complete(&store, &session).await.unwrap();
            assert!(complete(&store, &session).await.is_err());
            assert!(store
                .ready_session(&id, &session.input_id, "late rewrite")
                .await
                .is_err());
            let completed = store.flow(flow.id()).await.unwrap().unwrap();
            assert_eq!(
                reserve(&store, &completed).await.unwrap().as_deref(),
                Some("Use the revised design")
            );
            assert!(store
                .sessions(&crate::session::SessionFilter::default())
                .await
                .unwrap()
                .is_empty());
            assert_eq!(completed.cursor, waiting.cursor);
        });
        for (key, value) in previous {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}
