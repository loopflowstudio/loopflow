//! A Wave's effective setup: what its Tasks run, where, and which logins pay,
//! each with the layer that chose it. Shared settings come from the layered
//! config (`~/.lf/config.yaml` → `.lf/config.yaml` → `.lf/wave/<name>/config.yaml`)
//! under your own `lf set` values; Machine and account routes are only yours,
//! stored on this Machine.
use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config::{config_layers, graph_setting, layer_setting, ConfigLayer};
use crate::durable::{MachineSetting, OwningMachineSource};
use crate::profile::RouteScope;
use crate::provider_account::nearest_provider_route;
use crate::provider_auth::Provider;
use crate::repository::RepoId;
use crate::store::SharedStore;
use crate::work::wave::Wave;

/// Which layer chose a setting. `Task` is a Task's own choice, which outranks
/// the rest; a Wave's setup never reports it. `Personal` is your own `lf set`
/// value for the Wave or repository, above the committed `Wave` and `Repo`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SetupSource {
    Task,
    Personal,
    Wave,
    Repo,
    Global,
    Default,
}

impl SetupSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Task => "task",
            Self::Personal => "personal",
            Self::Wave => "wave",
            Self::Repo => "repo",
            Self::Global => "global",
            Self::Default => "default",
        }
    }
}

impl From<ConfigLayer> for SetupSource {
    fn from(layer: ConfigLayer) -> Self {
        match layer {
            ConfigLayer::Personal => Self::Personal,
            ConfigLayer::Wave => Self::Wave,
            ConfigLayer::Repo => Self::Repo,
            ConfigLayer::Global => Self::Global,
        }
    }
}

/// One effective setting; `value` is absent when nothing chooses one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetupValue {
    pub value: Option<String>,
    pub source: SetupSource,
}

/// The accounts a provider's launches try in order; empty means no route, so
/// launches try every account eligible for automatic routing, else the
/// provider's own login.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountSetup {
    pub accounts: Vec<String>,
    pub source: SetupSource,
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
    let agent = layer_setting(&layers, &["agent"])
        .map(|(layer, value)| setting(layer, value))
        .unwrap_or_else(|| SetupValue {
            value: Some(crate::config::default_agent().to_owned()),
            source: SetupSource::Default,
        });
    let mut skill_agents = BTreeMap::new();
    for layer in &layers {
        if let Some(skills) = layer.value.get("skill_agents").and_then(|v| v.as_mapping()) {
            for (skill, value) in skills {
                if let Some(skill) = skill.as_str().filter(|_| !value.is_null()) {
                    skill_agents.insert(skill.to_owned(), setting(layer.layer, value));
                }
            }
        }
    }
    let repo = RepoId::discover(checkout).ok();
    let mut accounts = BTreeMap::new();
    for provider in [Provider::Claude, Provider::Codex] {
        let route = nearest_provider_route(store, repo.as_ref(), Some(wave.id()), provider).await?;
        let setup = match route {
            Some(route) => AccountSetup {
                accounts: route.accounts.iter().map(ToString::to_string).collect(),
                source: match route.scope {
                    RouteScope::Wave(_) | RouteScope::Repo(_) => SetupSource::Personal,
                    RouteScope::Default => SetupSource::Global,
                },
            },
            None => AccountSetup {
                accounts: Vec::new(),
                source: SetupSource::Default,
            },
        };
        accounts.insert(provider.as_str().to_owned(), setup);
    }
    let (graph, graph_layer) = graph_setting(&layers)?;
    Ok(WaveSetup {
        graph: SetupValue {
            value: Some(graph),
            source: graph_layer.map_or(SetupSource::Default, SetupSource::from),
        },
        agent,
        skill_agents,
        tasks_machine: machine(store, wave, MachineSetting::Tasks)?,
        session_machine: machine(store, wave, MachineSetting::Session)?,
        accounts,
    })
}

fn setting(layer: ConfigLayer, value: &serde_yaml_ng::Value) -> SetupValue {
    SetupValue {
        value: value.as_str().map(str::to_owned),
        source: layer.into(),
    }
}

fn machine(
    store: &SharedStore,
    wave: &Wave,
    setting: MachineSetting,
) -> anyhow::Result<SetupValue> {
    let (machine, source) = store.sqlite.wave_owning_machine(wave.id(), setting)?;
    Ok(SetupValue {
        value: Some(machine.to_string()),
        source: match source {
            OwningMachineSource::Wave(_) | OwningMachineSource::Repository => SetupSource::Personal,
            OwningMachineSource::ThisMachine => SetupSource::Default,
        },
    })
}
