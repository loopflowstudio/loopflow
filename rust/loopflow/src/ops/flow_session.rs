//! Retained review capture reads. New operational Flows never create review Sessions.
use anyhow::{bail, ensure, Result};

use crate::durable::FlowSession;
use crate::engine::{ConcreteSkill, ConcreteStep, Skill};
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
