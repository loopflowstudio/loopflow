//! File locations for domain operations that name a retained repository or Wave.
use anyhow::{Context, Result};
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

/// Ordered account references; exact IDs, login emails and unambiguous prefixes
/// are resolved against the executor's account observations at launch.
#[derive(schemars::JsonSchema, Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "AccountOrderInput", into = "Vec<String>")]
pub struct AccountOrder(pub Vec<String>);

#[derive(schemars::JsonSchema, Debug, Deserialize)]
#[serde(untagged)]
enum AccountOrderInput {
    List(Vec<String>),
    CommaSeparated(String),
}
impl TryFrom<AccountOrderInput> for AccountOrder {
    type Error = String;
    fn try_from(value: AccountOrderInput) -> Result<Self, Self::Error> {
        let values = match value {
            AccountOrderInput::List(values) => values,
            AccountOrderInput::CommaSeparated(value) => value
                .split(',')
                .map(|value| value.trim().to_owned())
                .collect(),
        };
        let mut seen = std::collections::HashSet::new();
        if values
            .iter()
            .any(|value| value.is_empty() || !seen.insert(value.clone()))
        {
            return Err("account references must be nonempty and unique".into());
        }
        Ok(Self(values))
    }
}
impl From<AccountOrder> for Vec<String> {
    fn from(value: AccountOrder) -> Self {
        value.0
    }
}
impl std::ops::Deref for AccountOrder {
    type Target = [String];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
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

pub(crate) fn read_at(
    home: &Path,
    scope: Option<&loopflow_storage::sqlite::scope_defaults::ScopeAddress>,
) -> Result<Value> {
    let (path, prefix) = location_at(home, scope)?;
    let mut value = super::files::read(&path)?;
    super::remote::apply_document(&path, &mut value)?;
    for part in prefix {
        value = value.get(part).cloned().unwrap_or(Value::Null);
    }
    Ok(value)
}

pub(crate) fn route_location(
    store: &loopflow_storage::sqlite::SqliteStore,
    scope: &crate::profile::RouteScope,
) -> Result<(PathBuf, Vec<String>)> {
    match scope {
        crate::profile::RouteScope::Default => Ok((store.home_dir()?.join("config.yaml"), vec![])),
        crate::profile::RouteScope::Wave(id) => {
            let wave = store.get_wave(id)?.context("Wave not found")?;
            Ok((
                super::files::private_path(Path::new(wave.repo()))?,
                vec!["waves".into(), id.to_string()],
            ))
        }
        crate::profile::RouteScope::Repo(repo) => {
            Ok((super::files::private_path(repo.as_path())?, vec![]))
        }
    }
}

pub(crate) fn write_global(home: &Path, key: &[&str], value: Option<Value>) -> Result<bool> {
    super::files::edit(
        &home.join("config.yaml"),
        key,
        value,
        super::keys::Root::General,
    )
}

pub(crate) fn global_settings_at(home: &Path) -> Result<super::Config> {
    let path = home.join("config.yaml");
    let mut value = super::files::read(&path)?;
    super::remote::apply_document(&path, &mut value)?;
    let declarations = [super::ConfigDeclarations {
        source: super::ConfigSource::Personal,
        scope: super::settings::SettingScope::Global,
        path,
        value,
    }];
    Ok(super::settings::resolve_layers(&declarations, None)?.config)
}

#[derive(schemars::JsonSchema, Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountDeclaration {
    pub login_email: Option<crate::profile::EmailAddress>,
    pub routing_state: Option<crate::store::RoutingState>,
    pub plan: Option<String>,
    pub paid_through: Option<String>,
}

pub(crate) fn account_preferences(
    config: &super::Config,
    mut account: crate::store::ProviderAccount,
) -> Result<crate::store::ProviderAccount> {
    if let Some(declaration) = config
        .accounts
        .get(&account.provider)
        .and_then(|accounts| accounts.get(account.account_id.as_str()))
    {
        account.login_email = declaration.login_email.clone();
        account.routing_state = declaration
            .routing_state
            .unwrap_or(crate::store::RoutingState::Automatic);
        account.plan = declaration.plan.clone();
        account.paid_through = declaration
            .paid_through
            .as_deref()
            .map(parse_date)
            .transpose()?;
    }
    Ok(account)
}

pub(crate) fn parse_date(value: &str) -> Result<time::Date> {
    let format = time::format_description::parse_borrowed::<2>("[year]-[month]-[day]")?;
    Ok(time::Date::parse(value, &format)?)
}

pub(crate) fn write_account_preferences(
    home: &Path,
    account: &crate::store::ProviderAccount,
) -> Result<()> {
    let declaration = AccountDeclaration {
        login_email: account.login_email.clone(),
        routing_state: Some(account.routing_state),
        plan: account.plan.clone(),
        paid_through: account.paid_through.map(|date| date.to_string()),
    };
    let path = home.join("config.yaml");
    super::files::update(&path, |document| {
        if let Some(accounts) = document
            .get("accounts")
            .and_then(|accounts| accounts.get(&account.provider))
            .and_then(Value::as_mapping)
        {
            for (id, value) in accounts {
                if id.as_str() == Some(account.account_id.as_str()) {
                    continue;
                }
                let existing: AccountDeclaration = serde_yaml_ng::from_value(value.clone())?;
                anyhow::ensure!(
                    account.login_email.is_none() || existing.login_email != account.login_email,
                    "{} login {} already belongs to account {}",
                    account.provider,
                    account
                        .login_email
                        .as_ref()
                        .expect("duplicate login is present"),
                    id.as_str().unwrap_or_default()
                );
            }
        }
        super::settings::insert(
            document,
            &["accounts", &account.provider, account.account_id.as_str()],
            serde_yaml_ng::to_value(declaration)?,
        );
        Ok(())
    })
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
