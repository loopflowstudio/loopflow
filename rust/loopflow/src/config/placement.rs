//! File locations for domain operations that name a retained repository or Wave.
use crate::durable::DefaultScope;
use anyhow::{Context, Result};
use rusqlite::Connection;
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
    pub id: crate::durable::MachineId,
}

pub(crate) fn home(conn: &Connection) -> Result<PathBuf> {
    Ok(Path::new(
        conn.path()
            .context("file preferences require a file-backed registry")?,
    )
    .parent()
    .context("registry has a parent")?
    .to_owned())
}

pub(crate) fn location(
    conn: &Connection,
    scope: Option<&DefaultScope>,
) -> Result<(PathBuf, Vec<String>)> {
    let Some(scope) = scope else {
        return Ok((home(conn)?.join("config.yaml"), vec![]));
    };
    let (repo, wave): (String, Option<String>) = match scope {
        DefaultScope::Repository(id) => (
            conn.query_row(
                "SELECT repo FROM repository_plans WHERE id=?1",
                [id.as_str()],
                |row| row.get(0),
            )?,
            None,
        ),
        DefaultScope::Wave(id) => conn.query_row(
            "SELECT repo,id FROM wave_addresses WHERE id=?1",
            [id.as_str()],
            |row| Ok((row.get(0)?, Some(row.get(1)?))),
        )?,
    };
    Ok((
        super::files::private_path(Path::new(&repo))?,
        wave.map(|name| vec!["waves".into(), name])
            .unwrap_or_default(),
    ))
}

pub(crate) fn read(conn: &Connection, scope: Option<&DefaultScope>) -> Result<Value> {
    let (path, prefix) = location(conn, scope)?;
    let mut value = super::files::read(&path)?;
    super::remote::apply_document(&path, &mut value)?;
    for part in prefix {
        value = value.get(part).cloned().unwrap_or(Value::Null);
    }
    Ok(value)
}

pub(crate) fn write(
    conn: &Connection,
    scope: Option<&DefaultScope>,
    key: &[&str],
    value: Option<Value>,
) -> Result<bool> {
    let (path, prefix) = location(conn, scope)?;
    let path_parts: Vec<_> = prefix
        .iter()
        .map(String::as_str)
        .chain(key.iter().copied())
        .collect();
    super::files::edit(&path, &path_parts, value)
}

pub(crate) fn route_location(
    conn: &Connection,
    scope: &crate::profile::RouteScope,
) -> Result<(PathBuf, Vec<String>)> {
    match scope {
        crate::profile::RouteScope::Default => location(conn, None),
        crate::profile::RouteScope::Wave(id) => {
            location(conn, Some(&DefaultScope::Wave(id.clone())))
        }
        crate::profile::RouteScope::Repo(repo) => {
            Ok((super::files::private_path(repo.as_path())?, vec![]))
        }
    }
}

pub(crate) fn global_settings(conn: &Connection) -> Result<super::Config> {
    let (path, _) = location(conn, None)?;
    let declarations = [super::ConfigDeclarations {
        source: super::ConfigSource::Personal,
        scope: super::settings::SettingScope::Global,
        path,
        value: read(conn, None)?,
    }];
    Ok(super::settings::resolve_layers(&declarations, None)?.config)
}

pub(crate) fn machine_declarations(
    conn: &Connection,
) -> Result<std::collections::BTreeMap<String, MachineDeclaration>> {
    Ok(global_settings(conn)?.machines)
}

pub(crate) fn scopes(conn: &Connection) -> Result<Vec<(DefaultScope, String)>> {
    let mut out = Vec::new();
    let mut query = conn.prepare("SELECT id,repo FROM repository_plans WHERE selected=1")?;
    for row in query.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })? {
        let (id, repo) = row?;
        out.push((
            DefaultScope::Repository(crate::durable::RepositoryId::from_raw(id)),
            repo,
        ));
    }
    let mut query = conn.prepare("SELECT id,slug FROM wave_addresses WHERE retired_at IS NULL")?;
    for row in query.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })? {
        let (id, slug) = row?;
        out.push((
            DefaultScope::Wave(crate::id::WaveId::parse(&id).map_err(anyhow::Error::msg)?),
            slug,
        ));
    }
    Ok(out)
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
    conn: &Connection,
    account: &crate::store::ProviderAccount,
) -> Result<()> {
    let declaration = AccountDeclaration {
        login_email: account.login_email.clone(),
        routing_state: Some(account.routing_state),
        plan: account.plan.clone(),
        paid_through: account.paid_through.map(|date| date.to_string()),
    };
    let (path, _) = location(conn, None)?;
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
    pub provider: crate::rental::Provider,
    pub hardware: crate::rental::Hardware,
    pub max_machines: u32,
    pub max_usd_per_hour: crate::rental::Usd,
    pub hours: f64,
}

impl Default for OnDemandDeclaration {
    fn default() -> Self {
        Self {
            provider: crate::rental::Provider::Daytona,
            hardware: crate::rental::Hardware {
                vcpus: 2,
                memory_gb: 4,
                disk_gb: 20,
                gpu: None,
                spot: false,
            },
            max_machines: 0,
            max_usd_per_hour: crate::rental::Usd::ZERO,
            hours: crate::rental::DEFAULT_HOURS,
        }
    }
}

#[derive(schemars::JsonSchema, Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AuthBrowsers {
    pub default: Vec<String>,
    pub accounts: std::collections::BTreeMap<String, Vec<String>>,
}
