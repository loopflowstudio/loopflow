//! Effective settings for selected work alongside the declarations that can be edited.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::Serialize;
use serde_yaml_ng::Value;

use super::{catalog, graph_steps, promotion, target_options};
use crate::config::catalog::Applicability;
use crate::config::files::{edit, Destination};
use crate::config::settings::{EffectiveSetting, ResolvedSettings};
use crate::graph::Skill;

/// One resolver reading for the selected work and each editable declaration.
#[derive(Debug, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct Scope {
    pub settings: BTreeMap<String, EffectiveSetting>,
    pub files: Vec<ScopeFile>,
}

#[derive(Debug, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ScopeFile {
    pub destination: Destination,
    pub path: PathBuf,
    /// Includes absent slots. Omitted keys do not apply at this destination.
    pub slots: BTreeMap<String, Slot>,
}

#[derive(Debug, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct Slot {
    pub key: String,
    pub declared: Authored,
    pub entries: Vec<edit::Entry>,
    pub promotion: promotion::Availability,
}

#[derive(Debug, Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Authored {
    Absent,
    Present { value: serde_json::Value },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScopeTarget {
    General,
    Skill { name: String },
    Step { graph: String, step: String },
}

impl ScopeTarget {
    pub fn from_options(
        skill: Option<&str>,
        graph: Option<&str>,
        step: Option<&str>,
    ) -> Result<Self> {
        Ok(match (skill, graph, step) {
            (None, None, None) => Self::General,
            (Some(name), None, None) => Self::Skill { name: name.into() },
            (None, Some(graph), Some(step)) => Self::Step {
                graph: graph.into(),
                step: step.into(),
            },
            _ => anyhow::bail!("select a Skill or a Graph and step"),
        })
    }

    fn skill(&self, repo: Option<&Path>) -> Result<Option<Skill>> {
        let checkout = || repo.context("select a checkout for the target");
        match self {
            Self::General => Ok(None),
            Self::Skill { name } => Ok(Some(crate::graph::load_skill(name, checkout()?)?)),
            Self::Step { graph, step } => Ok(graph_steps(checkout()?, graph, Some(step))?
                .into_iter()
                .next()),
        }
    }
}

fn destinations(repository: Option<&Path>, wave: Option<&str>) -> Result<Vec<Destination>> {
    let mut destinations = vec![Destination::PersonalGlobal {}];
    if let Some(repo) = repository {
        destinations.push(Destination::SharedRepository {
            repository: repo.to_owned(),
        });
        destinations.push(Destination::PersonalRepository {
            repository: repo.to_owned(),
        });
        if let Some(wave) = wave {
            destinations.push(Destination::SharedWave {
                repository: repo.to_owned(),
                wave: wave.into(),
            });
            if crate::config::files::wave_key(repo, wave)?.is_some() {
                destinations.push(Destination::PersonalWave {
                    repository: repo.to_owned(),
                    wave: wave.into(),
                });
            }
        }
    } else {
        anyhow::ensure!(wave.is_none(), "select a checkout for the Wave");
    }
    Ok(destinations)
}

pub fn scope(directory: Option<&Path>, wave: Option<&str>, target: &ScopeTarget) -> Result<Scope> {
    crate::observe::reading(|| {
        let repository = directory
            .map(|directory| -> Result<_> {
                anyhow::ensure!(
                    crate::observe::is_dir(directory),
                    "Checkout {} is unavailable",
                    directory.display()
                );
                crate::repo::working_directory_in(directory)
            })
            .transpose()?;
        let definitions = catalog();
        let skill = target.skill(repository.as_deref())?;
        let effective = EffectiveSettings::read(repository.as_deref(), wave, skill.as_ref())?;
        let settings = definitions
            .iter()
            .map(|definition| {
                Ok((
                    definition.canonical_key(),
                    effective.setting(definition.key)?,
                ))
            })
            .collect::<Result<_>>()?;
        let prefix = match target {
            ScopeTarget::General => Some(String::new()),
            ScopeTarget::Skill { name } => Some(format!("skill.{name}.")),
            ScopeTarget::Step { .. } => skill
                .as_ref()
                .and_then(|s| s.occurrence.as_ref())
                .and_then(|occ| {
                    occ.id
                        .as_ref()
                        .map(|id| format!("graphs.{}.steps.{id}.", occ.graph))
                }),
        };
        let mut documents = BTreeMap::new();
        let mut files = Vec::new();
        for destination in destinations(repository.as_deref(), wave)? {
            let address = destination.resolve()?;
            let (raw, normalized) = match documents.entry(address.path.clone()) {
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::btree_map::Entry::Vacant(entry) => {
                    let (_, raw) = edit::read_document(&address)?;
                    let mut normalized = raw.clone();
                    crate::config::normalize_preferences(&mut normalized);
                    entry.insert((raw, normalized))
                }
            };
            let mut slots = BTreeMap::new();
            for definition in &definitions {
                // Personal preferences keep their global address even when inspecting a Skill.
                let prefix = if definition.applicability == Applicability::Personal {
                    ""
                } else if let Some(prefix) = &prefix {
                    prefix
                } else {
                    continue;
                };
                if !definition.applies(address.source, address.scope, !prefix.is_empty()) {
                    continue;
                }
                let key = definition.canonical_key();
                let edit_key = format!("{prefix}{key}");
                let setting = edit::Setting::catalogued(&address, &edit_key);
                let entries = setting.entries(raw)?;
                let declared = match setting.declared(normalized, &entries)? {
                    Some(value) => Authored::Present {
                        value: serde_json::to_value(value)?,
                    },
                    None => Authored::Absent,
                };
                slots.insert(
                    key,
                    Slot {
                        key: edit_key,
                        declared,
                        entries,
                        promotion: promotion::Availability::None,
                    },
                );
            }
            files.push(ScopeFile {
                destination,
                path: address.path,
                slots,
            });
        }
        promotion::offer_moves(&mut files)?;
        Ok(Scope { settings, files })
    })
}

struct EffectiveSettings {
    resolved: Arc<ResolvedSettings>,
    values: Value,
}

impl EffectiveSettings {
    fn read(repository: Option<&Path>, wave: Option<&str>, skill: Option<&Skill>) -> Result<Self> {
        let general = crate::config::resolved(repository, wave)?;
        let resolved = match skill {
            Some(skill) => Arc::new(crate::config::settings::resolve(
                &general.config,
                Some(skill),
            )?),
            None => general,
        };
        let values = serde_yaml_ng::to_value(&resolved.config)?;
        Ok(Self { resolved, values })
    }

    fn setting(&self, key: &str) -> Result<EffectiveSetting> {
        let group = matches!(key, "default_agent" | "agents");
        let lookup = if group { "agent" } else { key };
        let mut effective = self
            .resolved
            .settings
            .get(lookup)
            .cloned()
            .unwrap_or_else(|| {
                let prefix = format!("{key}.");
                EffectiveSetting {
                    value: key
                        .split('.')
                        .try_fold(&self.values, |node, part| node.get(part))
                        .cloned()
                        .unwrap_or(Value::Null),
                    candidates: self
                        .resolved
                        .settings
                        .range(prefix.clone()..)
                        .take_while(|(k, _)| k.starts_with(&prefix))
                        .flat_map(|(_, row)| row.candidates.iter().cloned())
                        .collect(),
                }
            });
        if group {
            effective.value = serde_yaml_ng::to_value(self.resolved.config.agent_order())?;
        }
        if key == "planning" {
            effective.value = crate::config::keys::tracker_value(&self.values);
        }
        Ok(effective)
    }
}

/// A move previews the same result as the scoped row, for just the moved key.
pub(super) fn projected_setting(
    repository: &Path,
    wave: Option<&str>,
    key: &str,
) -> Result<EffectiveSetting> {
    let typed = crate::config::keys::typed_key(key);
    let parts = typed.split('.').collect::<Vec<_>>();
    let preference = crate::config::settings::preference_parts(&parts)?;
    let (skill, graph, step) = target_options(&parts[..parts.len() - preference.len()]);
    let target = ScopeTarget::from_options(skill, graph, step)?;
    let skill = target.skill(Some(repository))?;
    EffectiveSettings::read(Some(repository), wave, skill.as_ref())?.setting(&preference.join("."))
}
