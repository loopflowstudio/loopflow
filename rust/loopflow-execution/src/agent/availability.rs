//! Launch choices on the Machine that runs the provider. Read each client's
//! status once per selection, including when several models share a harness.
use std::collections::BTreeMap;
use std::process::Command;
use std::time::Duration;

use loopflow_harness::HarnessKind;

use crate::config::{Agent, Config};
use crate::error::LoadError;

#[derive(Debug, Default)]
pub struct MachineAgents {
    checked: BTreeMap<HarnessKind, Option<String>>,
}

impl MachineAgents {
    pub(crate) fn failure(&mut self, kind: HarnessKind) -> Option<&str> {
        self.checked
            .entry(kind)
            .or_insert_with(|| unavailable(kind))
            .as_deref()
    }

    pub(crate) fn for_prompt(
        &mut self,
        config: &Config,
        surface: crate::prompt::Surface,
    ) -> Result<(Agent, Vec<Agent>), LoadError> {
        let (agent, rest) = self.choose(config)?;
        if surface == crate::prompt::Surface::Headless && crate::journal::graph_step() {
            // An explicit worker stays on its harness. Report a missing login
            // before admitting a worker that would just wait for sign-in.
            if let Some(reason) = self.failure(agent.harness) {
                return Err(LoadError::HarnessUnavailable(reason.to_owned()));
            }
            if let Some(first) = config.agent_order().first() {
                let first = Agent::parse(first)?;
                if let Some(reason) = self.failure(first.harness) {
                    super::_record_agent_move(&first, reason, reason, &agent);
                }
            }
        }
        Ok((agent, rest))
    }

    /// The agent a launch binds: the first usable one, else the configured
    /// choice, whose client then says why it cannot start.
    pub(crate) fn choose(&mut self, config: &Config) -> Result<(Agent, Vec<Agent>), LoadError> {
        match self.resolve(config) {
            Err(LoadError::HarnessUnavailable(_)) => config.resolve_agents(),
            resolved => resolved,
        }
    }

    /// Keep a pinned agent exact. With preferences, start at the first
    /// usable model and retain the configured quota fallbacks behind it,
    /// unchecked: a status probe runs the client, so only the start pays one.
    pub(crate) fn resolve(&mut self, config: &Config) -> Result<(Agent, Vec<Agent>), LoadError> {
        let (first, rest) = config.resolve_agents()?;
        if config.agent.is_some() {
            return Ok((first, rest));
        }
        let mut order = std::iter::once(first).chain(rest);
        if let Some(start) = order
            .by_ref()
            .find(|agent| self.failure(agent.harness).is_none())
        {
            return Ok((start, order.collect()));
        }
        // A Machine may have a native login outside the portable preferences.
        // This last resort is for initial selection only, never a quota move.
        for kind in HarnessKind::ALL {
            if self.failure(kind).is_none() {
                let agent = Agent {
                    harness: kind,
                    model: None,
                }
                .with_configured_model(config);
                return Ok((agent, Vec::new()));
            }
        }
        Err(LoadError::HarnessUnavailable(
            self.checked
                .values()
                .flatten()
                .cloned()
                .collect::<Vec<_>>()
                .join("; "),
        ))
    }
}

fn unavailable(kind: HarnessKind) -> Option<String> {
    if !crate::harness::launcher(kind).installed() {
        return Some(super::missing_agent_message(kind));
    }
    let (token, args) = match kind {
        HarnessKind::Claude => ("CLAUDE_CODE_OAUTH_TOKEN", ["auth", "status"]),
        HarnessKind::Codex => ("CODEX_ACCESS_TOKEN", ["login", "status"]),
        _ => return None,
    };
    if std::env::var_os(token).is_some_and(|value| !value.is_empty()) {
        return None;
    }
    // Let the client inspect its own file, keyring, or configured auth
    // backend. It sees the same ambient API-key policy as a launch.
    let mut command = Command::new(kind.as_str());
    command.args(args);
    for key in crate::provider_auth::api_key_env_names() {
        command.env_remove(key);
    }
    match crate::os_process::bounded_output(&mut command, Duration::from_secs(5)) {
        Ok(status) if status.status.success() => None,
        Ok(_) => Some(format!(
            "{} has no usable login on this Machine; {}",
            kind.as_str(),
            kind.harness().sign_in_advice()
        )),
        Err(error) => Some(format!(
            "could not check {} login on this Machine: {error}",
            kind.as_str()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::MachineAgents;
    use crate::config::Config;
    use loopflow_harness::HarnessKind;

    #[test]
    fn an_absent_preference_uses_the_configured_model_before_an_unlisted_harness() {
        let config = Config {
            default_agent: Some("agy".into()),
            agents: vec!["claude:haiku".into(), "codex:small".into()],
            ..Default::default()
        };
        let mut machine = MachineAgents::default();
        for kind in HarnessKind::ALL {
            machine
                .checked
                .insert(kind, Some(format!("{} unavailable", kind.as_str())));
        }
        let missing = machine.resolve(&config).unwrap_err().to_string();
        for kind in HarnessKind::ALL {
            assert!(missing.contains(&format!("{} unavailable", kind.as_str())));
        }
        machine.checked.insert(HarnessKind::Codex, None);
        assert_eq!(
            machine.resolve(&config).unwrap().0.to_string(),
            "codex:small"
        );
        machine.checked.insert(HarnessKind::Claude, None);
        let (agent, fallbacks) = machine.resolve(&config).unwrap();
        assert_eq!(agent.to_string(), "claude:haiku");
        assert_eq!(fallbacks[0].to_string(), "codex:small");
        let outside = Config {
            agents: Vec::new(),
            ..config
        };
        assert_eq!(machine.resolve(&outside).unwrap().0.to_string(), "claude");
    }
}
