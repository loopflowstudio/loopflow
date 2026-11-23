//! Policy for continuing a recorded conversation.
use super::AgentConfig;
use anyhow::Result;

/// Explicit continuation reloads policy at the saved checkout, retaining native identity.
pub fn resolve_session_config(
    session: &crate::session::LfSession,
    settings: serde_yaml_ng::Value,
    output_schema: Option<&serde_json::Value>,
) -> Result<AgentConfig> {
    let _selection = crate::config::settings::select_invocation(settings);
    let store = crate::store::open_sqlite(&crate::store::database_path_from_env()?)?;
    let wave = session
        .wave_id
        .as_ref()
        .map(|id| store.get_wave(id))
        .transpose()?
        .flatten();
    let current =
        crate::config::load_wave_config(Some(&session.cwd), wave.as_ref().map(|wave| wave.slug()))?
            .unwrap_or_default();
    let saved = if let Some(key) = session.artifact_key.as_deref() {
        match crate::session_record::resolve_manifest(&crate::store::lf_home_dir(), key) {
            Ok((_, saved)) => Some(saved),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        }
    } else {
        None
    };
    let input_harness = saved.as_ref().map(|saved| saved.harness.clone());
    let request = saved.and_then(|saved| saved.process);
    let skill = request
        .as_ref()
        .and_then(|request| request.skill_invocation.as_ref())
        .map(|invocation| invocation.skill.clone())
        .or_else(|| {
            session
                .skill
                .as_deref()
                .and_then(|name| crate::graph::load_skill(name, &session.cwd).ok())
        });
    let provider = session
        .provider
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("Session has no provider"))?;
    let resolved = crate::config::settings::resolve(&current, skill.as_ref())?;
    let current = &resolved.config;
    let candidate = resolved.settings["agent"].selected();
    let selected = candidate
        .filter(|candidate| {
            candidate.key == "agent"
                && matches!(
                    candidate.source,
                    crate::config::ConfigSource::CommandLine
                        | crate::config::ConfigSource::Environment
                )
        })
        .map(|candidate| candidate.value.as_str().expect("agent is a string"));
    let configured_agent = candidate
        .filter(|candidate| candidate.source != crate::config::ConfigSource::Builtin)
        .and_then(|_| resolved.settings["agent"].value.as_str());
    let model = if let Some(agent) = configured_agent {
        let (harness, model) = crate::config::parse_agent(agent);
        if harness == provider {
            model.or_else(|| session.model.clone())
        } else {
            session.model.clone()
        }
    } else {
        session.model.clone()
    };
    let (agent, importing) =
        match crate::config::resume_choice(provider, model.as_deref(), selected) {
            crate::config::Resume::Native(model) => (
                model.map_or_else(
                    || provider.to_string(),
                    |model| format!("{provider}:{model}"),
                ),
                false,
            ),
            crate::config::Resume::Import { agent } => (agent, true),
        };
    // The latest input's thread and account belong to the provider it ran on.
    let resumes_native = !importing
        && input_harness
            .as_deref()
            .is_none_or(|harness| harness == provider);
    let mut config = AgentConfig {
        cwd: Some(session.cwd.clone()),
        yolo: current.yolo,
        chrome: current.chrome,
        codex_service_tier: current.codex.service_tier,
        output_schema: output_schema.cloned(),
        ..AgentConfig::new(crate::config::Agent::parse(&agent)?)
    };
    if importing || selected.is_some_and(|agent| agent == "claude") {
        config.agent = config.agent.with_configured_model(current);
    }
    if let Some(request) = request {
        if !importing
            && crate::harness::launcher(config.agent.harness)
                .capabilities()
                .initial_prompt_on_resume
        {
            config.task_prompt = request.codex_initial_input();
        }
        config.system_prompt = request.system_prompt;
        config.conversation_context = request.conversation_context;
    }
    if resumes_native {
        if let Some(native) = session
            .artifact_key
            .as_deref()
            .map(|key| store.input_provider_session(key))
            .transpose()?
            .flatten()
        {
            config.resume_token = Some(native.agent_session);
        }
    }
    Ok(config)
}
