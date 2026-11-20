//! A Wave's effective setup: what its Tasks run, where, and which logins pay,
//! each with the layer that chose it. Shared settings come from the layered
//! config (`~/.lf/config.yaml` → `.lf/config.yaml` → `.lf/wave/<name>/config.yaml`)
//! under your own `lf set` values; Machine and account routes are only yours,
//! stored on this Machine.
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config::settings::Outside;
use crate::config::ConfigSource;
use crate::durable::OwningMachineSource;
use crate::error::LoadError;
use crate::observe::Derived;
use crate::store::SharedStore;
use crate::work::wave::Wave;
use loopflow_model::durable::MachineSetting;

/// One effective setting; `value` is absent when nothing chooses one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetupValue {
    pub value: Option<String>,
    pub source: ConfigSource,
    pub scope: Option<crate::config::settings::SettingScope>,
    pub locator: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WaveSetup {
    pub graph: SetupValue,
    /// `on` when the Wave's Tasks turn auto on as they take up their Graph.
    pub auto: SetupValue,
    pub agent: SetupValue,
    /// Agent by skill name, for every skill some layer names.
    pub skill_agents: BTreeMap<String, SetupValue>,
    /// Where the Wave's new Tasks run.
    pub tasks_machine: SetupValue,
    /// Where the Wave's Session runs.
    pub session_machine: SetupValue,
}

/// Read the setup of `wave` from `checkout`'s config and this Machine's store.
pub async fn read_wave_setup(
    store: &SharedStore,
    wave: &Wave,
    checkout: &Path,
) -> anyhow::Result<WaveSetup> {
    let outside = crate::config::settings::outside()?;
    let resolved =
        crate::config::resolved_with(Some(checkout), Some(checkout), Some(wave.slug()), &outside)?;
    Ok(WaveSetup {
        graph: effective_setting(&resolved, "graph"),
        auto: effective_setting(&resolved, "auto"),
        agent: effective_setting(&resolved, "agent"),
        skill_agents: skill_agents(checkout, wave.slug(), &outside)?,
        tasks_machine: machine(store, wave, MachineSetting::Tasks)?,
        session_machine: machine(store, wave, MachineSetting::Session)?,
    })
}

/// The agent of every skill some layer names, shared by served readings.
fn skill_agents(
    checkout: &Path,
    wave: &str,
    outside: &Outside,
) -> Result<BTreeMap<String, SetupValue>, LoadError> {
    let key = Derived::SkillAgents {
        checkout: checkout.to_owned(),
        wave: wave.to_owned(),
        outside: outside.clone(),
    };
    outside.shared(key, || resolve_skill_agents(checkout, wave, outside))
}

/// One resolution and one skill load per skill.
fn resolve_skill_agents(
    checkout: &Path,
    wave: &str,
    outside: &Outside,
) -> Result<BTreeMap<String, SetupValue>, LoadError> {
    let resolved =
        crate::config::resolved_with(Some(checkout), Some(checkout), Some(wave), outside)?;
    let layers = resolved
        .config
        .setting_layers
        .as_deref()
        .expect("resolved settings keep their layers");
    let skills: BTreeSet<_> = layers
        .iter()
        .filter_map(|layer| layer.value.get("skills")?.as_mapping())
        .flat_map(|skills| skills.keys().filter_map(|key| key.as_str()))
        .collect();
    let mut agents = BTreeMap::new();
    for skill in skills {
        let definition = crate::graph::load_skill(skill, checkout)
            .unwrap_or_else(|_| crate::graph::Skill::named(skill));
        let selected = crate::config::settings::resolve_with(layers, Some(&definition), outside)?;
        agents.insert(skill.to_owned(), effective_setting(&selected, "agent"));
    }
    Ok(agents)
}

fn effective_setting(
    resolved: &crate::config::settings::ResolvedSettings,
    key: &str,
) -> SetupValue {
    let setting = resolved.settings.get(key);
    SetupValue {
        value: setting
            .and_then(|setting| setting.value.as_str())
            .map(str::to_owned),
        source: setting
            .and_then(|setting| setting.selected())
            .map_or(ConfigSource::Builtin, |candidate| candidate.source),
        scope: setting
            .and_then(|setting| setting.selected())
            .map(|candidate| candidate.scope),
        locator: setting
            .and_then(|setting| setting.selected())
            .map(|candidate| candidate.locator.clone()),
    }
}

fn machine(
    store: &SharedStore,
    wave: &Wave,
    setting: MachineSetting,
) -> anyhow::Result<SetupValue> {
    let (machine, source) =
        crate::store::placement::wave_owning_machine(&store.sqlite, wave.id(), setting)?;
    let selected = match &source {
        OwningMachineSource::Preference(selected) => Some(selected.as_ref()),
        OwningMachineSource::ThisMachine => None,
    };
    Ok(SetupValue {
        value: Some(machine.to_string()),
        source: selected.map_or(ConfigSource::Builtin, |selected| selected.source),
        scope: selected.map(|selected| selected.scope),
        locator: selected.map(|selected| selected.locator.clone()),
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{skill_agents, Derived};
    use crate::observe::Cache;

    #[test]
    fn readings_share_a_waves_skill_agents_until_a_settings_file_changes() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home =
            crate::config::environment::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let repo = tempfile::tempdir().unwrap();
        let shared = repo.path().join(".lf/config.yaml");
        std::fs::create_dir_all(shared.parent().unwrap()).unwrap();
        std::fs::write(&shared, "skills: {review: {agent: claude}}\n").unwrap();
        let outside = crate::config::settings::outside().unwrap();
        let key = Derived::SkillAgents {
            checkout: repo.path().to_owned(),
            wave: "product".into(),
            outside: outside.clone(),
        };
        let cache = Arc::new(Cache::default());
        let read = || {
            let agents = skill_agents(repo.path(), "product", &outside).unwrap();
            agents["review"].value.clone().unwrap()
        };
        let (first, dependencies) = cache.capture(read);
        assert_eq!(first, "claude");
        assert_eq!(cache.capture(read).0, "claude");
        assert_eq!(cache.computations(|computed| *computed == key), 1);

        // The Wave's own file did not exist; it reaches the value through the
        // settings the skill agents were resolved from.
        let wave = repo.path().join(".lf/wave/product/config.yaml");
        std::fs::create_dir_all(wave.parent().unwrap()).unwrap();
        std::fs::write(&wave, "skills: {review: {agent: codex}}\n").unwrap();
        cache.refresh_all();
        assert!(dependencies.changed());
        assert_eq!(cache.capture(read).0, "codex");
        assert_eq!(cache.computations(|computed| *computed == key), 2);
    }
}
