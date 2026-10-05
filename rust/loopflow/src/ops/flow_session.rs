//! Human review of a saved Flow. The review Session retains the Flow and Work
//! attribution; the Flow waits on it through `pending_session_id`.
use anyhow::{anyhow, bail, ensure, Result};

use crate::durable::FlowSession;
use crate::engine::{ConcreteSkill, ConcreteStep, Skill};
use crate::ops::flow_run::{self, ActiveStep};
use crate::ops::human_session;
use crate::session::{AgentSession, SessionKind, TitleSource, WorkSource};
use crate::session_record::{SessionFlowMembership, SessionFlowStep};
use crate::store::SharedStore;

fn current_skill(flow: &FlowSession) -> Result<&ConcreteSkill> {
    match flow.current_step() {
        Some(ConcreteStep::Skill(skill)) if skill.human => Ok(skill),
        _ => bail!("saved Flow position is not a human skill"),
    }
}

/// The Skill a review's agent runs: the one captured at the waiting step.
pub(crate) fn pinned_skill(session_id: &str, requested: &str) -> Result<Skill> {
    let store = crate::store::sqlite::SqliteStore::new(&crate::store::database_path_from_env()?)?;
    let flow = store.waiting_review(session_id)?;
    let skill = &current_skill(&flow)?.skill;
    ensure!(
        skill.name == requested,
        "human Flow Session Skill does not match requested Skill"
    );
    Ok(skill.clone())
}

pub(crate) async fn membership(
    store: &SharedStore,
    session_id: &str,
) -> Result<SessionFlowMembership> {
    Ok(SessionFlowMembership::Step(SessionFlowStep::of(
        &store.waiting_review(session_id).await?,
    )?))
}

/// Preserve the review input without consuming feedback as a Flow result.
pub(crate) async fn reserve(store: &SharedStore, flow: &FlowSession) -> Result<()> {
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
            let session = AgentSession {
                captured: None,
                caller_artifact_key: None,
                id,
                artifact_key: crate::session_record::new_artifact_key(),
                input_published: false,
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
            };
            store.create_session(session, Some(flow.clone())).await?
        }
    };
    if !session.input_published {
        human_session::publish_prepared_input(
            store,
            &session,
            SessionFlowMembership::Step(SessionFlowStep::of(flow)?),
        )?;
    }
    Ok(())
}

fn review_message(flow: &FlowSession, session_id: &str) -> String {
    format!(
        "Historical review {session_id} for Flow {}. Discuss feedback and preserve agreed changes and remaining work in scratch. The conversation remains open; inspect execution and effects before choosing further work.",
        flow.invocation.flow,
    )
}

/// The review's Exec: the Flow's Work, its pinned Skill and step.
pub(crate) async fn prepare_exec(
    store: &SharedStore,
    command: &mut tokio::process::Command,
    session_id: &str,
) -> Result<()> {
    let flow = store.waiting_review(session_id).await?;
    command
        .args(["--mode", "tui"])
        .env(flow_run::FLOW_STEP_ENV, ActiveStep::of(&flow).env_value()?)
        .env(
            crate::provider_account::lease::ACCOUNT_SELECTION_ENV,
            flow.invocation
                .accounts
                .clone()
                .unwrap_or_default()
                .env_value()?,
        )
        .envs(flow.invocation.isolation_env());
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
        "--",
        &current_skill(&flow)?.skill.name,
        &review_message(&flow, session_id),
    ]);
    Ok(())
}
