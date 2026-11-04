//! The file editor used by CLI and Desktop, including Appearance Apply.
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Catalog,
    File {
        path: PathBuf,
        key: String,
        value: Option<String>,
        unset: bool,
        scope: super::settings::SettingScope,
        shared: bool,
    },
}

pub fn run(request: Request) -> Result<serde_json::Value> {
    match request {
        Request::Catalog => Ok(serde_json::to_value(super::catalog::DEFINITIONS)?),
        Request::File {
            path,
            key,
            value,
            unset,
            scope,
            shared,
        } => {
            anyhow::ensure!(!(value.is_some() && unset), "choose a value or unset");
            let path =
                if path
                    .file_name()
                    .is_some_and(|name| name == super::files::PRIVATE_FILE)
                {
                    super::files::private_path(path.parent().ok_or_else(|| {
                        anyhow::anyhow!("private configuration needs a repository")
                    })?)?
                } else {
                    path
                };
            let key = if scope == super::settings::SettingScope::Wave && !shared {
                let (wave, suffix) = key
                    .strip_prefix("waves.")
                    .and_then(|key| key.split_once('.'))
                    .ok_or_else(|| {
                        anyhow::anyhow!("personal Wave keys begin waves.<name-or-id>.")
                    })?;
                let id = super::files::wave_key(
                    path.parent()
                        .ok_or_else(|| anyhow::anyhow!("Wave settings need a repository"))?,
                    wave,
                )?;
                format!("waves.{id}.{suffix}")
            } else {
                key
            };
            let effective_key = if scope == super::settings::SettingScope::Wave && !shared {
                key.strip_prefix("waves.")
                    .and_then(|rest| rest.split_once('.'))
                    .map(|(_, key)| key)
                    .ok_or_else(|| {
                        anyhow::anyhow!("personal Wave keys begin waves.<name-or-id>.")
                    })?
            } else {
                &key
            };
            super::settings::validate_scope(
                effective_key,
                if shared {
                    super::ConfigSource::Repository
                } else {
                    super::ConfigSource::Personal
                },
                scope,
            )?;
            let parsed = value
                .as_deref()
                .map(|value| super::settings::parse_value(effective_key, value))
                .transpose()?;
            let parts: Vec<_> = key.split('.').collect();
            if parsed.is_some() || unset {
                super::files::edit(&path, &parts, parsed)?;
            }
            let document = super::read_global_config_at(&path)?;
            let value = parts
                .iter()
                .try_fold(&document, |value, part| value.get(*part))
                .cloned()
                .unwrap_or(Value::Null);
            Ok(
                serde_json::json!({"path":path,"key":key,"value":value,"lifecycle":super::catalog::definition(super::settings::preference_path(effective_key)?[0]).expect("validated key").lifecycle}),
            )
        }
    }
}
