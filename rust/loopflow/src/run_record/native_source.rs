//! Native locations are captured by the launcher, never inferred from a watcher environment.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum NativeSource {
    Jsonl { path: PathBuf },
    OpenCode { path: PathBuf },
}

impl NativeSource {
    pub(crate) fn path(&self) -> &Path {
        match self {
            Self::Jsonl { path } | Self::OpenCode { path } => path,
        }
    }
}

fn effective_env(command: &Command, key: &str) -> Option<OsString> {
    command
        .get_envs()
        .find(|(name, _)| *name == key)
        .map(|(_, value)| value.map(OsString::from))
        .unwrap_or_else(|| std::env::var_os(key))
}

pub(crate) fn source_for_command(
    command: &Command,
    provider: &str,
    session: &str,
) -> io::Result<Option<NativeSource>> {
    let cwd = command
        .get_current_dir()
        .map(Path::to_path_buf)
        .unwrap_or(std::env::current_dir()?);
    source_from_env(provider, session, &cwd, |key| effective_env(command, key))
}

pub(crate) fn source_for_callback(
    provider: &str,
    session: &str,
    payload: &serde_json::Value,
) -> io::Result<Option<NativeSource>> {
    let cwd = std::env::current_dir()?;
    if matches!(provider, "claude" | "codex") {
        if let Some(path) = payload
            .get("transcript_path")
            .and_then(serde_json::Value::as_str)
            .filter(|p| !p.is_empty())
        {
            return Ok(Some(NativeSource::Jsonl {
                path: absolute_path(PathBuf::from(path), &cwd),
            }));
        }
    }
    source_from_env(provider, session, &cwd, |key| std::env::var_os(key))
}

fn absolute_path(path: PathBuf, cwd: &Path) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

fn source_from_env(
    provider: &str,
    session: &str,
    cwd: &Path,
    env: impl Fn(&str) -> Option<OsString>,
) -> io::Result<Option<NativeSource>> {
    let home = env("HOME").map(PathBuf::from);
    match provider {
        "opencode" => {
            let root = env("XDG_DATA_HOME")
                .map(PathBuf::from)
                .or_else(|| home.map(|home| home.join(".local/share")));
            Ok(root.map(|root| NativeSource::OpenCode {
                path: absolute_path(root.join("opencode/opencode.db"), cwd),
            }))
        }
        "claude" | "codex" => {
            let (key, default) = if provider == "claude" {
                ("CLAUDE_CONFIG_DIR", ".claude")
            } else {
                ("CODEX_HOME", ".codex")
            };
            let root = env(key)
                .map(PathBuf::from)
                .or_else(|| home.map(|home| home.join(default)));
            root.map(|root| find_jsonl(&absolute_path(root, cwd), provider, session))
                .transpose()
                .map(Option::flatten)
        }
        _ => Ok(None),
    }
}

pub(crate) fn find_jsonl(
    home: &Path,
    provider: &str,
    session: &str,
) -> io::Result<Option<NativeSource>> {
    if session.is_empty() || session.contains(['/', '\\']) {
        return Ok(None);
    }
    let roots = match provider {
        "claude" => vec![home.join("projects")],
        "codex" => vec![home.join("sessions"), home.join("archived_sessions")],
        _ => return Ok(None),
    };
    let mut pending = roots.into_iter().map(|p| (p, 0)).collect::<Vec<_>>();
    let mut found = None;
    while let Some((dir, depth)) = pending.pop() {
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        for entry in entries {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() && depth < 4 {
                pending.push((entry.path(), depth + 1));
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if kind.is_file()
                && (name == format!("{session}.jsonl")
                    || (provider == "codex" && name.ends_with(&format!("-{session}.jsonl"))))
            {
                if found.is_some() {
                    return Err(io::Error::other(
                        "multiple native transcripts match the recorded Session",
                    ));
                }
                found = Some(NativeSource::Jsonl { path: entry.path() });
            }
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::{source_for_command, NativeSource};
    use std::process::Command;

    #[test]
    fn launch_source_uses_effective_home_and_custom_data_root() {
        let temp = tempfile::tempdir().unwrap();
        let account = temp.path().join("account");
        std::fs::create_dir_all(account.join("projects/project")).unwrap();
        let path = account.join("projects/project/session.jsonl");
        std::fs::write(&path, "").unwrap();
        let mut command = Command::new("claude");
        command.env("CLAUDE_CONFIG_DIR", &account);
        assert_eq!(
            source_for_command(&command, "claude", "session").unwrap(),
            Some(NativeSource::Jsonl { path })
        );
        command.env("XDG_DATA_HOME", temp.path().join("custom"));
        assert_eq!(
            source_for_command(&command, "opencode", "session").unwrap(),
            Some(NativeSource::OpenCode {
                path: temp.path().join("custom/opencode/opencode.db")
            })
        );
        assert_eq!(
            source_for_command(&command, "claude", "missing").unwrap(),
            None
        );
    }
}

#[cfg(test)]
mod account_tests {
    use crate::provider_auth::Provider;
    use crate::run_record::{CaptureHandle, RunSpec};
    use crate::store::{open_ephemeral_store, ProviderAccountId, StorageConfig};

    #[tokio::test]
    async fn passive_resolution_never_borrows_another_account_or_selects_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            open_ephemeral_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
                .await
                .unwrap(),
        );
        let other_home = dir.path().join("other");
        std::fs::create_dir_all(other_home.join("projects/project")).unwrap();
        std::fs::write(other_home.join("projects/project/session.jsonl"), "").unwrap();
        let other = crate::provider_account::new_account(
            Provider::Claude,
            ProviderAccountId::parse("other").unwrap(),
            other_home,
            None,
        );
        store.upsert_provider_account(&other).await.unwrap();
        let capture = CaptureHandle::begin_at(
            dir.path(),
            RunSpec {
                harness: "claude".into(),
                model: None,
                surface: "tui".into(),
                cwd: dir.path().to_path_buf(),
                repo: None,
                worktree: None,
                skill: None,
                subjects: vec![],
            },
        )
        .unwrap();
        let account_id = ProviderAccountId::parse("recorded").unwrap();
        crate::run_record::write_provider_session(
            &capture.artifact_dir(),
            "session",
            Some(account_id.clone()),
            None,
        )
        .unwrap();
        let receipt = crate::run_record::read_provider_session(&capture.artifact_dir())
            .unwrap()
            .unwrap();
        let manifest = crate::run_record::read_manifest(&capture.artifact_dir()).unwrap();
        assert!(
            crate::run_record::output::resolve_native(&store, &manifest, &receipt)
                .await
                .unwrap()
                .is_none()
        );
        let home = dir.path().join("recorded");
        std::fs::create_dir_all(home.join("projects/project")).unwrap();
        let path = home.join("projects/project/session.jsonl");
        std::fs::write(&path, "").unwrap();
        let account =
            crate::provider_account::new_account(Provider::Claude, account_id, home, None);
        store.upsert_provider_account(&account).await.unwrap();
        let before = store.list_provider_accounts(None).await.unwrap();
        assert_eq!(
            crate::run_record::output::resolve_native(&store, &manifest, &receipt)
                .await
                .unwrap()
                .unwrap()
                .path(),
            path
        );
        assert_eq!(store.list_provider_accounts(None).await.unwrap(), before);
    }
}
