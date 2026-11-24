//! Conversation identity and execution history. An invocation never owns feedback.

pub use loopflow_storage::session::*;

pub fn conversation(
    cwd: &std::path::Path,
    skill: &str,
    title: String,
    wave: Option<&str>,
) -> anyhow::Result<LfSession> {
    let skill = crate::graph::load_skill(skill, cwd)?;
    let agent = match conversation_agent(cwd, wave, &skill, |config| {
        crate::agent::MachineAgents::default().resolve(config)
    }) {
        Ok(agent) => Some(agent),
        // A Task can acquire its checkout before a login reaches the Machine.
        Err(crate::error::LoadError::HarnessUnavailable(_)) => None,
        Err(error) => return Err(error.into()),
    };
    Ok(LfSession {
        captured: None,
        id: uuid::Uuid::new_v4().simple().to_string().into(),
        artifact_key: None,
        caller_artifact_key: None,
        input_published: false,
        cwd: cwd.to_path_buf(),
        skill: Some(skill.name),
        provider: agent
            .as_ref()
            .map(|agent| agent.harness.as_str().to_owned()),
        model: agent.and_then(|agent| agent.model),
        node: None,
        iterations: None,
        task_id: None,
        wave_id: None,
        graph_lf_process_id: None,
        work_source: None,
        bound_at: None,
        interactive: true,
        repo: None,
        title,
        title_source: TitleSource::Generated,
        request: None,
        completed_at: None,
        created_at: loopflow_storage::rows::now_unix(),
    })
}

fn conversation_agent(
    cwd: &std::path::Path,
    wave: Option<&str>,
    skill: &crate::graph::Skill,
    select: impl FnOnce(&crate::config::Config) -> AgentChoice,
) -> Result<crate::config::Agent, crate::error::LoadError> {
    let shared = crate::config::resolved_for_task(cwd, cwd, wave)?;
    let resolved = crate::config::settings::resolve(&shared.config, Some(skill))?;
    Ok(select(&resolved.config)?.0)
}

type AgentChoice =
    Result<(crate::config::Agent, Vec<crate::config::Agent>), crate::error::LoadError>;

/// Choose the first provider for a conversation prepared without a usable login.
pub(crate) fn pending_agent(
    store: &loopflow_storage::sqlite::SqliteStore,
    session: &LfSession,
    select: impl FnOnce(&crate::config::Config) -> AgentChoice,
) -> anyhow::Result<crate::config::Agent> {
    let name = session
        .skill
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("Session {} has no skill or provider", session.id))?;
    let skill = crate::graph::load_skill(name, &session.cwd)?;
    let wave = session
        .wave_id
        .as_ref()
        .map(|id| store.get_wave(id))
        .transpose()?
        .flatten();
    Ok(conversation_agent(
        &session.cwd,
        wave.as_ref().map(|wave| wave.slug()),
        &skill,
        select,
    )?)
}

#[cfg(test)]
mod title_tests {
    #[test]
    fn generated_task_title_repairs_tokens_and_bounds_unicode_purpose() {
        let task =
            loopflow_model::durable::TaskId::from_raw("task_abcdef10000000000000000000000001");
        for title in [
            "abcdef Fix login",
            "lf-abcdef1 Fix login",
            "task_abcdef10000000000000000000000001 Fix login",
            "ENG-1 Fix login",
        ] {
            assert_eq!(
                super::task_session_title(title, &task, "ENG-2", Some("ENG-1")),
                "ENG-2 Fix login"
            );
        }
        let title = super::task_session_title(&"λ".repeat(100), &task, "ENG-2", None);
        assert!(title.starts_with("ENG-2 λ"));
        assert_eq!(title.chars().count(), 80);
    }
}
