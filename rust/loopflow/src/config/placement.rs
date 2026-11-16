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
    super::files::edit(&path, &path_parts, value, super::keys::Root::General)
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

pub(crate) fn historical_account_labels(conn: &Connection) -> Result<Value> {
    let (path, _) = location(conn, None)?;
    super::files::read(&path)
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
