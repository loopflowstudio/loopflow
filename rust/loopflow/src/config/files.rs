//! One editor for personal and shared preference files. Locks protect LF writers;
//! a content comparison also detects hand edits made while an edit is prepared.
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{ensure, Context, Result};
use serde_yaml_ng::Value;
use yaml_edit::{Mapping, YamlFile};

pub const PRIVATE_FILE: &str = "lf.config.local.yaml";

pub fn private_path(repo: &Path) -> Result<PathBuf> {
    let root = match crate::git::worktree_root(repo) {
        Ok(_) => crate::git::worktrees::main_repo_root(repo)?,
        Err(_) => repo.to_owned(),
    };
    Ok(root.join(PRIVATE_FILE))
}

pub(crate) fn wave_key(repo: &Path, name: &str) -> Result<String> {
    if let Ok(id) = crate::id::WaveId::parse(name) {
        return Ok(id.to_string());
    }
    let config = crate::work::wave::config::try_read_wave_config(repo, name)?;
    config.and_then(|config| config.id).map(|id| id.to_string())
        .with_context(|| format!("Wave {name} needs its authored id before private settings can follow it through renames"))
}

pub fn destination(
    repo: &Path,
    wave: Option<&str>,
    global: bool,
    shared: bool,
) -> Result<(PathBuf, Vec<String>)> {
    ensure!(
        !(global && (wave.is_some() || shared)),
        "--global selects personal global settings"
    );
    if global {
        return Ok((crate::store::lf_home_dir().join("config.yaml"), vec![]));
    }
    if shared {
        return Ok((
            match wave {
                Some(wave) => {
                    ensure!(
                        !wave.split('/').any(|part| part == "..") && !Path::new(wave).is_absolute(),
                        "invalid Wave name"
                    );
                    repo.join(".lf/wave").join(wave).join("config.yaml")
                }
                None => repo.join(".lf/config.yaml"),
            },
            vec![],
        ));
    }
    let path = private_path(repo)?;
    protect_private_file(&path)?;
    Ok((
        path,
        wave.map(|name| wave_key(repo, name).map(|id| vec!["waves".into(), id]))
            .transpose()?
            .unwrap_or_default(),
    ))
}

fn git(repo: &Path, args: &[&str]) -> Result<std::process::Output> {
    Ok(std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()?)
}

pub fn diagnose_private_file(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let repo = path.parent().context("private file has a parent")?;
    if !git(repo, &["rev-parse", "--git-dir"])?.status.success() {
        return Ok(());
    }
    ensure!(
        !git(repo, &["ls-files", "--error-unmatch", PRIVATE_FILE])?
            .status
            .success(),
        "{} is tracked; remove it from the index explicitly before using private settings",
        path.display()
    );
    if !git(repo, &["check-ignore", "-q", PRIVATE_FILE])?
        .status
        .success()
    {
        tracing::warn!(path = %path.display(), "private settings are not ignored; the next lf set will add the common Git exclude rule");
    }
    Ok(())
}

pub(crate) fn protect_private_file(path: &Path) -> Result<()> {
    diagnose_private_file(path)?;
    let repo = path.parent().context("private file has a parent")?;
    let output = git(
        repo,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    if !output.status.success() {
        return Ok(());
    }
    let exclude = PathBuf::from(String::from_utf8(output.stdout)?.trim()).join("info/exclude");
    let _lock = lock(&exclude)?;
    let mut text = read_text(&exclude)?;
    if !text.lines().any(|line| line == format!("/{PRIVATE_FILE}")) {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&format!("/{PRIVATE_FILE}\n"));
        atomic_write(&exclude, &text)?;
    }
    Ok(())
}

pub(crate) fn lock(path: &Path) -> Result<File> {
    fs::create_dir_all(path.parent().context("configuration has a parent")?)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path.with_extension("yaml.lock"))?;
    fs2::FileExt::lock_exclusive(&lock)?;
    Ok(lock)
}

pub(crate) fn read_text(path: &Path) -> Result<String> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error).with_context(|| format!("Read {}", path.display())),
    }
}

pub(crate) fn atomic_write(path: &Path, text: &str) -> Result<()> {
    let parent = path.parent().context("configuration has a parent")?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    if let Ok(metadata) = fs::metadata(path) {
        file.as_file().set_permissions(metadata.permissions())?;
    }
    file.write_all(text.as_bytes())?;
    file.as_file().sync_all()?;
    file.persist(path)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn patch(mapping: &Mapping, before: &Value, after: &Value) -> Result<()> {
    let before = before.as_mapping();
    let after = after
        .as_mapping()
        .context("configuration must be a mapping")?;
    if let Some(before) = before {
        for key in before.keys().filter(|key| !after.contains_key(*key)) {
            mapping.remove(key.as_str().context("configuration keys must be strings")?);
        }
    }
    for (key, value) in after {
        let name = key.as_str().context("configuration keys must be strings")?;
        let old = before.and_then(|before| before.get(key));
        if old == Some(value) {
            continue;
        }
        if let Some(child) = mapping
            .get(name)
            .and_then(|node| node.as_mapping().cloned())
            .filter(|_| value.is_mapping())
        {
            patch(&child, old.unwrap_or(&Value::Null), value)?;
        } else {
            let fragment = yaml_edit::Document::from_str(&serde_yaml_ng::to_string(value)?)?;
            mapping.set(name, fragment);
        }
    }
    Ok(())
}

pub fn update<T>(path: &Path, change: impl FnOnce(&mut Value) -> Result<T>) -> Result<T> {
    if path.file_name().is_some_and(|name| name == PRIVATE_FILE) {
        protect_private_file(path)?;
    }
    let _lock = lock(path)?;
    let original = read_text(path)?;
    let before: Value = if original.trim().is_empty() {
        Value::Mapping(Default::default())
    } else {
        serde_yaml_ng::from_str(&original)?
    };
    let before = if before.is_null() {
        Value::Mapping(Default::default())
    } else {
        before
    };
    ensure!(
        before.is_mapping(),
        "{} must contain a mapping",
        path.display()
    );
    let mut after = before.clone();
    let result = change(&mut after)?;
    if after == before {
        return Ok(result);
    }
    let text = edited_text(&original, &after)?;
    ensure!(
        read_text(path)? == original,
        "{} changed during this edit; retry with the current file",
        path.display()
    );
    atomic_write(path, &text)?;
    Ok(result)
}

pub(crate) fn edited_text(original: &str, after: &Value) -> Result<String> {
    let before: Value = serde_yaml_ng::from_str(original)?;
    let file = YamlFile::from_str(original)?;
    let document = file.ensure_document();
    if document.as_mapping().is_none() {
        // A comment-only document has no root yet. Setting its first key creates it.
        for (key, value) in after
            .as_mapping()
            .context("configuration must remain a mapping")?
        {
            document.set(
                key.as_str().context("configuration keys must be strings")?,
                yaml_edit::Document::from_str(&serde_yaml_ng::to_string(value)?)?,
            );
        }
    } else {
        patch(
            &document.as_mapping().expect("checked mapping"),
            &before,
            after,
        )?;
    }
    let text = file.to_string();
    ensure!(
        serde_yaml_ng::from_str::<Value>(&text)? == *after,
        "configuration edit did not preserve its typed value"
    );
    Ok(text)
}

// Copy authored fields into a document. This does not resolve precedence or
// combine preference lists; those policies belong to settings::resolve_layers.
pub(crate) fn overlay(target: &mut Value, source: &Value) {
    if let (Some(target), Some(source)) = (target.as_mapping_mut(), source.as_mapping()) {
        for (key, value) in source {
            overlay(target.entry(key.clone()).or_insert(Value::Null), value);
        }
    } else {
        *target = source.clone();
    }
}

pub fn remove(root: &mut Value, path: &[&str]) -> bool {
    let Some((key, rest)) = path.split_first() else {
        return false;
    };
    let Some(map) = root.as_mapping_mut() else {
        return false;
    };
    if rest.is_empty() {
        return map.remove(*key).is_some();
    }
    let Some(child) = map.get_mut(*key) else {
        return false;
    };
    let removed = remove(child, rest);
    if child.as_mapping().is_some_and(|map| map.is_empty()) {
        map.remove(*key);
    }
    removed
}

pub(crate) fn edit(path: &Path, parts: &[&str], value: Option<Value>) -> Result<()> {
    update(path, |document| {
        match value {
            Some(value) => {
                if parts.len() >= 2 && parts[parts.len() - 2] == "machine" {
                    let sibling = match parts.last() {
                        Some(&"tasks") => Some("capacity"),
                        Some(&"capacity") => Some("tasks"),
                        _ => None,
                    };
                    if let Some(sibling) = sibling {
                        let mut path = parts[..parts.len() - 1].to_vec();
                        path.push(sibling);
                        remove(document, &path);
                    }
                }
                super::settings::insert(document, parts, value)
            }
            None => {
                remove(document, parts);
            }
        }
        let offset = if parts.first() == Some(&"waves") {
            2
        } else {
            0
        };
        let settings = &parts[offset..];
        let preference = super::settings::preference_path(&settings.join("."))?.len();
        let root_index = parts.len() - preference;
        if let Some(value) = parts[..=root_index]
            .iter()
            .try_fold(&*document, |value, key| value.get(*key))
        {
            super::settings::validate_value(&[parts[root_index]], value)?;
        }
        Ok(())
    })
}

pub fn set(
    repo: &Path,
    wave: Option<&str>,
    global: bool,
    shared: bool,
    key: &str,
    value: Option<&str>,
) -> Result<PathBuf> {
    let scope = if global {
        super::settings::SettingScope::Global
    } else if wave.is_some() {
        super::settings::SettingScope::Wave
    } else {
        super::settings::SettingScope::Repository
    };
    super::settings::validate_scope(
        key,
        if shared {
            super::ConfigSource::Repository
        } else {
            super::ConfigSource::Personal
        },
        scope,
    )?;
    let value = value
        .map(|text| super::settings::parse_value(key, text))
        .transpose()?;
    let (file, mut prefix) = destination(repo, wave, global, shared)?;
    prefix.extend(key.split('.').map(str::to_owned));
    let path: Vec<_> = prefix.iter().map(String::as_str).collect();
    edit(&file, &path, value)?;
    Ok(file)
}

/// Explicitly adopt authored recovery declarations into the addressed clone or global file.
pub fn import(
    repo: &Path,
    wave: Option<&str>,
    global: bool,
    source: &Path,
    pointer: &str,
    replace: bool,
) -> Result<PathBuf> {
    let document = super::read_global_config_at(source)?;
    let json = serde_json::to_value(document)?;
    let incoming: Value = serde_json::from_value(
        json.pointer(pointer)
            .context("recovery pointer not found")?
            .clone(),
    )?;
    super::settings::validate_preferences(&incoming, false, true)?;
    let incoming = incoming
        .as_mapping()
        .context("import selects a settings mapping")?;
    let scope = if global {
        super::settings::SettingScope::Global
    } else if wave.is_some() {
        super::settings::SettingScope::Wave
    } else {
        super::settings::SettingScope::Repository
    };
    for (key, value) in incoming {
        let key = key.as_str().context("setting key must be text")?;
        if key == "skills" || key == "graphs" {
            continue;
        }
        super::settings::validate_scope(key, super::ConfigSource::Personal, scope)?;
        super::settings::validate_value(&[key], value)?;
    }
    let (path, prefix) = destination(repo, wave, global, false)?;
    update(&path, |document| {
        for (key, value) in incoming {
            let key = key.as_str().context("setting key must be text")?;
            let parts: Vec<_> = prefix.iter().map(String::as_str).chain([key]).collect();
            let existing = parts
                .iter()
                .try_fold(&*document, |value, key| value.get(*key));
            ensure!(replace || existing.is_none_or(|old| old == value), "{key} conflicts with saved settings; use --replace to adopt the imported declaration");
            super::settings::insert(document, &parts, value.clone());
        }
        Ok(())
    })?;
    Ok(path)
}

/// Copy a relocating Wave's declarations before its address changes. Cleanup
/// happens only after the domain move succeeds, so either address keeps a copy.
pub(crate) fn copy_wave(
    source: &Path,
    target: &Path,
    id: &crate::id::WaveId,
) -> Result<Option<Value>> {
    let source = private_path(source)?;
    let target = private_path(target)?;
    if source == target || !source.exists() {
        return Ok(None);
    }
    let value = super::read_global_config_at(&source)?
        .get("waves")
        .and_then(|waves| waves.get(id.as_str()))
        .cloned();
    if let Some(value) = &value {
        update(&target, |document| {
            let existing = document
                .get("waves")
                .and_then(|waves| waves.get(id.as_str()));
            ensure!(
                existing.is_none_or(|existing| existing == value),
                "Wave {} has independent preferences in {}; resolve them before relocation",
                id,
                target.display()
            );
            super::settings::insert(document, &["waves", id.as_str()], value.clone());
            Ok(())
        })?;
    }
    Ok(value)
}

pub(crate) fn finish_wave_move(
    source: &Path,
    id: &crate::id::WaveId,
    copied: &Value,
) -> Result<()> {
    let path = private_path(source)?;
    update(&path, |document| {
        if document
            .get("waves")
            .and_then(|waves| waves.get(id.as_str()))
            == Some(copied)
        {
            remove(document, &["waves", id.as_str()]);
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::{private_path, set, update};
    #[test]
    fn recovery_import_keeps_order_and_requires_explicit_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let recovery = dir.path().join("recovery.yaml");
        std::fs::write(&recovery, "repo: {old: {account: {codex: [one, two]}}}\n").unwrap();
        let repo = dir.path().join("clone");
        std::fs::create_dir(&repo).unwrap();
        super::import(&repo, None, false, &recovery, "/repo/old", false).unwrap();
        let path = private_path(&repo).unwrap();
        assert_eq!(
            crate::config::read_global_config_at(&path).unwrap()["account"]["codex"][0],
            "one"
        );
        set(
            &repo,
            None,
            false,
            false,
            "account.codex",
            Some("[two, one]"),
        )
        .unwrap();
        assert!(super::import(&repo, None, false, &recovery, "/repo/old", false).is_err());
        super::import(&repo, None, false, &recovery, "/repo/old", true).unwrap();
        assert_eq!(
            crate::config::read_global_config_at(&path).unwrap()["account"]["codex"][0],
            "one"
        );
    }

    #[test]
    fn edits_preserve_comments_unrelated_keys_and_false() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.yaml");
        std::fs::write(
            &file,
            "# retained\nagent: claude # choice\nyolo: true\nunknown: 'keep this'\n",
        )
        .unwrap();
        update(&file, |value| {
            value["yolo"] = false.into();
            Ok(())
        })
        .unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains("# retained"));
        assert!(text.contains("agent: claude # choice"));
        assert!(text.contains("unknown: 'keep this'"));
        assert_eq!(
            serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&text).unwrap()["yolo"],
            false
        );
    }
    #[test]
    fn linked_worktrees_share_private_settings_and_unset_reveals_lower_values() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home =
            crate::lf::commands::graph::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let repo = loopflow_test_support::TestRepo::new();
        repo.create_file(".lf/config.yaml", "agent: claude\n");
        let child = home.path().join("child");
        assert!(std::process::Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(["worktree", "add", "-b", "child"])
            .arg(&child)
            .status()
            .unwrap()
            .success());
        set(&child, None, false, false, "agent", Some("codex")).unwrap();
        assert_eq!(
            private_path(repo.path()).unwrap(),
            private_path(&child).unwrap()
        );
        assert_eq!(
            crate::config::load_config(Some(repo.path()))
                .unwrap()
                .unwrap()
                .agent(),
            "codex"
        );
        assert!(std::process::Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(["check-ignore", "-q", super::PRIVATE_FILE])
            .status()
            .unwrap()
            .success());
        set(repo.path(), None, false, false, "agent", None).unwrap();
        assert_eq!(
            crate::config::load_config(Some(repo.path()))
                .unwrap()
                .unwrap()
                .agent(),
            "claude"
        );
    }
    #[test]
    fn private_account_order_is_independent_between_clones_without_registration() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home =
            crate::lf::commands::graph::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let first = loopflow_test_support::TestRepo::new();
        let second = loopflow_test_support::TestRepo::new();
        for repo in [&first, &second] {
            assert!(std::process::Command::new("git")
                .arg("-C")
                .arg(repo.path())
                .args([
                    "remote",
                    "set-url",
                    "origin",
                    "https://github.com/example/project.git"
                ])
                .status()
                .unwrap()
                .success());
        }
        set(
            first.path(),
            None,
            false,
            false,
            "account.codex",
            Some("[one, two]"),
        )
        .unwrap();
        set(
            second.path(),
            None,
            false,
            false,
            "account.codex",
            Some("[two, one]"),
        )
        .unwrap();
        let accounts = |repo| {
            crate::config::load_config(Some(repo))
                .unwrap()
                .unwrap()
                .account["codex"]
                .clone()
        };
        assert_eq!(accounts(first.path())[0].as_str(), "one");
        assert_eq!(accounts(second.path())[0].as_str(), "two");
        assert!(!home.path().join("loopflow.db").exists());
    }

    #[test]
    fn wave_preferences_follow_identity_through_rename_and_old_name_reuse() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home =
            crate::lf::commands::graph::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let repo = loopflow_test_support::TestRepo::new();
        let first = crate::id::WaveId::new();
        repo.create_file(".lf/wave/old/config.yaml", &format!("id: {first}\n"));
        set(
            repo.path(),
            Some("old"),
            false,
            false,
            "agent",
            Some("claude:wave"),
        )
        .unwrap();
        std::fs::rename(
            repo.path().join(".lf/wave/old"),
            repo.path().join(".lf/wave/new"),
        )
        .unwrap();
        repo.create_file(
            ".lf/wave/old/config.yaml",
            &format!("id: {}\n", crate::id::WaveId::new()),
        );
        let selected = |name| {
            crate::config::load_wave_config(Some(repo.path()), Some(name))
                .unwrap()
                .unwrap()
                .agent
        };
        assert_eq!(selected("new").as_deref(), Some("claude:wave"));
        assert_eq!(selected("old"), None);
    }

    #[test]
    fn relocating_wave_preferences_preserves_conflicts_and_concurrent_edits() {
        let source = loopflow_test_support::TestRepo::new();
        let target = loopflow_test_support::TestRepo::new();
        let id = crate::id::WaveId::new();
        let key = format!("waves.{id}.agent");
        let source_path = private_path(source.path()).unwrap();
        let target_path = private_path(target.path()).unwrap();
        let parts: Vec<_> = key.split('.').collect();
        super::edit(&source_path, &parts, Some("codex".into())).unwrap();
        super::edit(&target_path, &parts, Some("claude".into())).unwrap();
        assert!(super::copy_wave(source.path(), target.path(), &id).is_err());
        super::edit(&target_path, &parts, None).unwrap();
        let copied = super::copy_wave(source.path(), target.path(), &id)
            .unwrap()
            .unwrap();
        assert_eq!(
            crate::config::read_global_config_at(&target_path).unwrap()["waves"][id.as_str()]
                ["agent"],
            "codex"
        );
        super::edit(&source_path, &parts, Some("claude".into())).unwrap();
        super::finish_wave_move(source.path(), &id, &copied).unwrap();
        assert_eq!(
            crate::config::read_global_config_at(&source_path).unwrap()["waves"][id.as_str()]
                ["agent"],
            "claude"
        );
        super::edit(&source_path, &parts, Some("codex".into())).unwrap();
        super::finish_wave_move(source.path(), &id, &copied).unwrap();
        assert!(
            crate::config::read_global_config_at(&source_path).unwrap()["waves"]
                .get(id.as_str())
                .is_none()
        );
    }

    #[test]
    fn hand_edit_during_write_is_preserved_and_concurrent_writers_keep_both_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        std::fs::write(&path, "agent: codex\n").unwrap();
        let result = update(&path, |value| {
            value["chrome"] = true.into();
            std::fs::write(&path, "agent: claude\n")?;
            Ok(())
        });
        assert!(result.unwrap_err().to_string().contains("changed during"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "agent: claude\n");
        std::thread::scope(|scope| {
            for key in ["yolo", "chrome"] {
                let path = &path;
                scope.spawn(move || {
                    update(path, |value| {
                        value[key] = false.into();
                        Ok(())
                    })
                    .unwrap()
                });
            }
        });
        let value = crate::config::read_global_config_at(&path).unwrap();
        assert_eq!(value["yolo"], false);
        assert_eq!(value["chrome"], false);
        assert_eq!(value["agent"], "claude");
    }

    #[test]
    fn desktop_and_cli_edits_share_validation_and_preserve_tagged_planning() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home =
            crate::lf::commands::graph::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let repo = loopflow_test_support::TestRepo::new();
        let path = set(
            repo.path(),
            None,
            false,
            false,
            "planning",
            Some("{provider: git, remote: origin}"),
        )
        .unwrap();
        let before = std::fs::read_to_string(&path).unwrap();
        let request = crate::config::api::Request::File {
            path: path.clone(),
            key: "planning.provider".into(),
            value: Some("linear".into()),
            unset: false,
            scope: crate::config::settings::SettingScope::Repository,
            shared: false,
        };
        assert!(crate::config::api::run(request).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
        crate::config::api::run(crate::config::api::Request::File {
            path: path.clone(),
            key: "chrome".into(),
            value: Some("false".into()),
            unset: false,
            scope: crate::config::settings::SettingScope::Repository,
            shared: false,
        })
        .unwrap();
        assert!(
            !crate::config::load_config(Some(repo.path()))
                .unwrap()
                .unwrap()
                .chrome
        );
    }
}
