//! A Wave's effective setup: what its Tasks run, where, and which logins pay,
//! each with the layer that chose it. Shared settings come from the layered
//! config (`~/.lf/config.yaml` → `.lf/config.yaml` → `.lf/wave/<name>/config.yaml`)
//! under your own `lf set` values; Machine and account routes are only yours,
//! stored on this Machine.
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config::{config_layers, ConfigSource};
use crate::durable::{MachineSetting, OwningMachineSource};
use crate::provider_auth::Provider;
use crate::store::SharedStore;
use crate::work::wave::Wave;

/// One effective setting; `value` is absent when nothing chooses one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetupValue {
    pub value: Option<String>,
    pub source: ConfigSource,
    pub scope: Option<crate::config::settings::SettingScope>,
    pub locator: Option<String>,
}

/// The accounts a provider's launches try in order; empty means no route, so
/// launches try every account eligible for automatic routing, else the
/// provider's own login.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountSetup {
    pub accounts: Vec<String>,
    pub source: ConfigSource,
    pub scope: Option<crate::config::settings::SettingScope>,
    pub locator: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaveSetup {
    pub graph: SetupValue,
    pub agent: SetupValue,
    /// Agent by skill name, for every skill some layer names.
    pub skill_agents: BTreeMap<String, SetupValue>,
    /// Where the Wave's new Tasks run.
    pub tasks_machine: SetupValue,
    /// Where the Wave's Session runs.
    pub session_machine: SetupValue,
    /// Account route by provider.
    pub accounts: BTreeMap<String, AccountSetup>,
}

/// Read the setup of `wave` from `checkout`'s config and this Machine's store.
pub async fn read_wave_setup(
    store: &SharedStore,
    wave: &Wave,
    checkout: &Path,
) -> anyhow::Result<WaveSetup> {
    let layers = config_layers(Some(checkout), Some(wave.slug()))?;
    let resolved = crate::config::settings::resolve_layers(&layers, None)?;
    let skills: BTreeSet<_> = layers
        .iter()
        .filter_map(|layer| layer.value.get("skills")?.as_mapping())
        .flat_map(|skills| skills.keys().filter_map(|key| key.as_str()))
        .collect();
    let mut skill_agents = BTreeMap::new();
    for skill in skills {
        let definition = crate::graph::load_skill(skill, checkout)
            .unwrap_or_else(|_| crate::graph::Skill::named(skill));
        let selected = crate::config::settings::resolve_layers(&layers, Some(&definition))?;
        skill_agents.insert(skill.to_owned(), effective_setting(&selected, "agent"));
    }
    let mut accounts = BTreeMap::new();
    for provider in [Provider::Claude, Provider::Codex] {
        let setting = resolved
            .settings
            .get(&format!("account.{}", provider.as_str()));
        let selected = setting.and_then(|setting| setting.selected());
        accounts.insert(
            provider.as_str().to_owned(),
            AccountSetup {
                accounts: resolved
                    .config
                    .account
                    .get(provider.as_str())
                    .into_iter()
                    .flat_map(|order| order.iter())
                    .map(ToString::to_string)
                    .collect(),
                source: selected.map_or(ConfigSource::Builtin, |candidate| candidate.source),
                scope: selected.map(|candidate| candidate.scope),
                locator: selected.map(|candidate| candidate.locator.clone()),
            },
        );
    }
    Ok(WaveSetup {
        graph: effective_setting(&resolved, "graph"),
        agent: effective_setting(&resolved, "agent"),
        skill_agents,
        tasks_machine: machine(store, wave, MachineSetting::Tasks)?,
        session_machine: machine(store, wave, MachineSetting::Session)?,
        accounts,
    })
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
    let (machine, source) = store.sqlite.wave_owning_machine(wave.id(), setting)?;
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
