//! File locations for domain operations that name a retained repository or Wave.
use anyhow::Result;
use loopflow_model::durable::DefaultScope;
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;
use std::path::{Path, PathBuf};

#[derive(schemars::JsonSchema, Debug, Clone, Default, Serialize, Deserialize)]
#[serde(from = "PlacementInput")]
pub struct Placement {
    pub tasks: Option<String>,
    pub session: Option<String>,
    pub capacity: Option<String>,
}

#[derive(schemars::JsonSchema, Debug, Clone, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum PlacementInput {
    Machine(String),
    Fields {
        tasks: Option<String>,
        session: Option<String>,
        capacity: Option<String>,
    },
}
impl From<PlacementInput> for Placement {
    fn from(value: PlacementInput) -> Self {
        match value {
            PlacementInput::Machine(id) => Self {
                tasks: Some(id.clone()),
                session: Some(id),
                capacity: None,
            },
            PlacementInput::Fields {
                tasks,
                session,
                capacity,
            } => Self {
                tasks,
                session,
                capacity,
            },
        }
    }
}

/// Expand a shorthand before editing one field so the other destination survives.
pub(crate) fn expand_shorthand(value: &mut Value) -> Result<()> {
    if value.is_string() {
        *value = serde_yaml_ng::to_value(serde_yaml_ng::from_value::<Placement>(value.clone())?)?;
    }
    Ok(())
}

#[derive(schemars::JsonSchema, Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MachineDeclaration {
    pub id: loopflow_model::durable::MachineId,
}

pub(crate) fn location_at(
    home: &Path,
    scope: Option<&loopflow_storage::sqlite::scope_defaults::ScopeAddress>,
) -> Result<(PathBuf, Vec<String>)> {
    let Some(scope) = scope else {
        return Ok((home.join("config.yaml"), vec![]));
    };
    Ok((
        super::files::private_path(Path::new(&scope.repo))?,
        match &scope.scope {
            DefaultScope::Repository(_) => vec![],
            DefaultScope::Wave(id) => vec!["waves".into(), id.to_string()],
        },
    ))
}

pub fn read_at(
    home: &Path,
    scope: Option<&loopflow_storage::sqlite::scope_defaults::ScopeAddress>,
) -> Result<Value> {
    let (path, prefix) = location_at(home, scope)?;
    let mut value = super::files::read(&path)?;
    super::remote::apply_document(
        super::remote::incoming_text().as_deref(),
        home,
        &path,
        &mut value,
    )?;
    for part in prefix {
        value = value.get(part).cloned().unwrap_or(Value::Null);
    }
    Ok(value)
}

pub(crate) fn write_global(home: &Path, key: &[&str], value: Option<Value>) -> Result<bool> {
    super::files::edit(
        &home.join("config.yaml"),
        key,
        value,
        super::keys::Root::General,
    )
}

/// What your `config.yaml` under `home` declares alone, for the Machine facts
/// no checkout overrides. Served readings share one value until it changes.
pub(crate) fn global_settings_at(home: &Path) -> Result<super::Config> {
    // A store's home is the one read, whatever `LF_HOME` says.
    let mut outside = super::settings::outside()?;
    outside.home = home.to_owned();
    let key = crate::observe::Derived::GlobalSettings {
        outside: outside.clone(),
    };
    outside
        .shared(key, || {
            resolve_global_settings(&outside).map_err(|error| error.to_string())
        })
        .map_err(anyhow::Error::msg)
}

fn resolve_global_settings(outside: &super::settings::Outside) -> Result<super::Config> {
    let path = outside.home.join("config.yaml");
    let mut value = super::files::read(&path)?;
    super::remote::apply_document(outside.remote.as_deref(), &outside.home, &path, &mut value)?;
    let declarations = [super::ConfigDeclarations {
        source: super::ConfigSource::Personal,
        scope: super::settings::SettingScope::Global,
        path,
        value,
    }];
    Ok(super::settings::resolve_with(&declarations, None, outside)?.config)
}

pub(crate) fn historical_account_labels(home: &Path) -> Result<Value> {
    super::files::read(&home.join("config.yaml"))
}

/// Read old labels for history, without interpreting routing preferences.
pub(crate) fn historical_account(
    document: &Value,
    mut account: crate::store::ProviderAccount,
) -> Result<crate::store::ProviderAccount> {
    account.login_email = document
        .get("accounts")
        .and_then(|providers| providers.get(&account.provider))
        .and_then(|accounts| accounts.get(account.account_id.as_str()))
        .and_then(|account| account.get("login_email"))
        .and_then(Value::as_str)
        .map(crate::profile::EmailAddress::parse)
        .transpose()
        .map_err(anyhow::Error::msg)?;
    Ok(account)
}

#[derive(schemars::JsonSchema, Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoolDeclaration {
    pub off: bool,
}

#[derive(schemars::JsonSchema, Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OnDemandDeclaration {
    pub provider: loopflow_storage::rental::Provider,
    pub hardware: loopflow_storage::rental::Hardware,
    pub max_machines: u32,
    pub max_usd_per_hour: loopflow_storage::rental::Usd,
    pub hours: f64,
}

impl Default for OnDemandDeclaration {
    fn default() -> Self {
        Self {
            provider: loopflow_storage::rental::Provider::Daytona,
            hardware: loopflow_storage::rental::Hardware {
                vcpus: 2,
                memory_gb: 4,
                disk_gb: 20,
                gpu: None,
                spot: false,
                modal_sizing: None,
            },
            max_machines: 0,
            max_usd_per_hour: loopflow_storage::rental::Usd::ZERO,
            hours: loopflow_storage::rental::DEFAULT_HOURS,
        }
    }
}

#[derive(schemars::JsonSchema, Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AuthBrowsers {
    pub default: Vec<String>,
    pub accounts: std::collections::BTreeMap<String, Vec<String>>,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::global_settings_at;
    use crate::observe::{Cache, Derived};

    #[test]
    fn readings_share_global_settings_until_the_file_changes() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home =
            crate::config::environment::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let file = home.path().join("config.yaml");
        std::fs::write(&file, "default_agent: claude\n").unwrap();
        let cache = Arc::new(Cache::default());
        let read = || {
            global_settings_at(home.path())
                .unwrap()
                .agent()
                .unwrap()
                .to_owned()
        };
        let global = |computed: &Derived| matches!(computed, Derived::GlobalSettings { .. });
        let (first, dependencies) = cache.capture(read);
        assert_eq!(first, "claude");
        assert_eq!(cache.capture(read).0, "claude");
        assert_eq!(cache.computations(global), 1);

        std::fs::write(&file, "default_agent: codex\n").unwrap();
        cache.refresh_all();
        assert!(dependencies.changed());
        assert_eq!(cache.capture(read).0, "codex");
        assert_eq!(cache.computations(global), 2);
    }
}
