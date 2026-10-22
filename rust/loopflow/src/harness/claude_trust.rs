//! Claude folder trust for worktrees lf created. Claude records trust only in
//! its global config (`projects[<path>].hasTrustDialogAccepted`), so lf sets that
//! one field for the exact lf-created worktree, in the config the launched child
//! reads. Main checkouts and hand-made worktrees keep Claude's own dialog.
use std::fs;
use std::io::{ErrorKind, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime};

use anyhow::{anyhow, Context, Result};
use serde_json::{Map, Value};

use crate::provider_account::launch_env;

const TRUST_FIELD: &str = "hasTrustDialogAccepted";
const LOCK_WAIT: Duration = Duration::from_secs(2);
// Matches proper-lockfile's default staleness, which Claude uses for this lock.
const LOCK_STALE: Duration = Duration::from_secs(10);

/// Pre-accept trust when `command` launches Claude in an lf-created worktree.
pub(crate) fn accept_created_worktree(command: &Command) {
    let Some(cwd) = command.get_current_dir() else {
        return;
    };
    let Some(worktree) = crate::git::lf_created_worktree(cwd) else {
        return;
    };
    let Some(config) = _config_path(command) else {
        return;
    };
    if let Err(error) = _accept(&config, &worktree) {
        tracing::warn!(
            config = %config.display(),
            worktree = %worktree.display(),
            %error,
            "left Claude folder trust to Claude's dialog"
        );
    }
}

/// The global config Claude reads: a non-empty legacy `.config.json` in its
/// config dir, else `$CLAUDE_CONFIG_DIR/.claude.json`, else `$HOME/.claude.json`.
fn _config_path(command: &Command) -> Option<PathBuf> {
    let config_dir = launch_env(command, "CLAUDE_CONFIG_DIR")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from);
    let home = launch_env(command, "HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from);
    let claude_dir = config_dir
        .clone()
        .or_else(|| Some(home.clone()?.join(".claude")))?;
    let legacy = claude_dir.join(".config.json");
    if fs::metadata(&legacy).is_ok_and(|meta| meta.len() > 0) {
        return Some(legacy);
    }
    Some(config_dir.or(home)?.join(".claude.json"))
}

fn _accept(config: &Path, worktree: &Path) -> Result<()> {
    let key = worktree
        .to_str()
        .ok_or_else(|| anyhow!("worktree path is not UTF-8"))?;
    if _trusted(&_read(config)?, key)? {
        return Ok(());
    }
    let _lock = ConfigLock::acquire(config)?;
    let mut root = _read(config)?;
    if _trusted(&root, key)? {
        return Ok(());
    }
    let projects = root
        .entry("projects")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| anyhow!("`projects` is not an object"))?;
    let project = projects
        .entry(key)
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| anyhow!("project entry is not an object"))?;
    project.insert(TRUST_FIELD.into(), Value::Bool(true));
    _write(config, &Value::Object(root))
}

fn _read(config: &Path) -> Result<Map<String, Value>> {
    match fs::read(config) {
        Ok(bytes) => match serde_json::from_slice(&bytes)? {
            Value::Object(root) => Ok(root),
            _ => Err(anyhow!("config is not a JSON object")),
        },
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(Map::new()),
        Err(error) => Err(error.into()),
    }
}

fn _trusted(root: &Map<String, Value>, key: &str) -> Result<bool> {
    let Some(projects) = root.get("projects") else {
        return Ok(false);
    };
    let projects = projects
        .as_object()
        .ok_or_else(|| anyhow!("`projects` is not an object"))?;
    Ok(projects
        .get(key)
        .and_then(|project| project.get(TRUST_FIELD))
        .and_then(Value::as_bool)
        .unwrap_or(false))
}

fn _write(config: &Path, root: &Value) -> Result<()> {
    let dir = config.parent().context("config has no directory")?;
    fs::create_dir_all(dir)?;
    let mut temp = tempfile::NamedTempFile::new_in(dir)?;
    temp.write_all(&serde_json::to_vec_pretty(root)?)?;
    temp.as_file()
        .set_permissions(fs::Permissions::from_mode(0o600))?;
    temp.persist(config)?;
    Ok(())
}

/// The mkdir lock Claude's proper-lockfile takes on `<config>.lock`.
#[derive(Debug)]
struct ConfigLock(PathBuf);

impl ConfigLock {
    fn acquire(config: &Path) -> Result<Self> {
        let mut lock = config.as_os_str().to_owned();
        lock.push(".lock");
        let lock = PathBuf::from(lock);
        if let Some(dir) = lock.parent() {
            fs::create_dir_all(dir)?;
        }
        let deadline = Instant::now() + LOCK_WAIT;
        loop {
            match fs::create_dir(&lock) {
                Ok(()) => return Ok(Self(lock)),
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    if _stale(&lock) {
                        let _ = fs::remove_dir(&lock);
                        continue;
                    }
                    if Instant::now() >= deadline {
                        return Err(anyhow!("{} is held", lock.display()));
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
}

impl Drop for ConfigLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.0);
    }
}

fn _stale(lock: &Path) -> bool {
    fs::metadata(lock)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .is_some_and(|age| age > LOCK_STALE)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    use serde_json::{json, Value};
    use tempfile::TempDir;

    use super::accept_created_worktree;
    use crate::git::{git_stdout, worktree_add, WorktreeBranch};

    struct Fixture {
        _root: TempDir,
        repo: std::path::PathBuf,
        home: std::path::PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir().unwrap();
            let repo = root.path().join("kata");
            let home = root.path().join("home");
            fs::create_dir_all(&repo).unwrap();
            fs::create_dir_all(&home).unwrap();
            git_stdout(&repo, &["init", "-b", "main"]).unwrap();
            git_stdout(
                &repo,
                &[
                    "-c",
                    "user.email=lf@example.com",
                    "-c",
                    "user.name=lf",
                    "commit",
                    "--allow-empty",
                    "-m",
                    "init",
                ],
            )
            .unwrap();
            Self {
                _root: root,
                repo,
                home,
            }
        }

        fn lf_worktree(&self) -> std::path::PathBuf {
            let path = self.repo.with_file_name("kata.agent-1");
            worktree_add(
                &self.repo,
                &path,
                "agent-1",
                WorktreeBranch::New {
                    start_point: "HEAD",
                },
            )
            .unwrap();
            path
        }

        fn launch(&self, cwd: &Path, config_dir: Option<&Path>) {
            let mut command = Command::new("claude");
            command.current_dir(cwd).env("HOME", &self.home);
            match config_dir {
                Some(dir) => command.env("CLAUDE_CONFIG_DIR", dir),
                None => command.env_remove("CLAUDE_CONFIG_DIR"),
            };
            accept_created_worktree(&command);
        }

        fn config(&self) -> std::path::PathBuf {
            self.home.join(".claude.json")
        }
    }

    fn read(path: &Path) -> Value {
        serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
    }

    #[test]
    fn lf_created_worktree_gains_only_its_exact_trust_entry() {
        let fixture = Fixture::new();
        let worktree = fixture.lf_worktree();
        let sibling =
            json!({"numStartups": 3, "projects": {"/elsewhere": {"allowedTools": ["x"]}}});
        fs::write(fixture.config(), sibling.to_string()).unwrap();

        fixture.launch(&worktree.join("."), None);

        let key = worktree.canonicalize().unwrap().display().to_string();
        let mut expected = sibling;
        expected["projects"][&key] = json!({"hasTrustDialogAccepted": true});
        assert_eq!(read(&fixture.config()), expected);
    }

    #[test]
    fn main_checkout_and_hand_made_worktree_leave_config_alone() {
        let fixture = Fixture::new();
        let hand_made = fixture.repo.with_file_name("kata.mine");
        git_stdout(
            &fixture.repo,
            &[
                "worktree",
                "add",
                "-b",
                "mine",
                &hand_made.to_string_lossy(),
            ],
        )
        .unwrap();

        fixture.launch(&fixture.repo, None);
        fixture.launch(&hand_made, None);

        assert!(!fixture.config().exists());
    }

    #[test]
    fn missing_config_is_created_with_the_entry() {
        let fixture = Fixture::new();
        let worktree = fixture.lf_worktree();

        fixture.launch(&worktree, None);

        let key = worktree.canonicalize().unwrap().display().to_string();
        assert_eq!(
            read(&fixture.config()),
            json!({"projects": {key: {"hasTrustDialogAccepted": true}}})
        );
    }

    #[test]
    fn unparseable_config_is_left_untouched() {
        let fixture = Fixture::new();
        let worktree = fixture.lf_worktree();
        fs::write(fixture.config(), "{not json").unwrap();

        fixture.launch(&worktree, None);

        assert_eq!(fs::read_to_string(fixture.config()).unwrap(), "{not json");
    }

    #[test]
    fn isolated_config_dir_receives_the_entry_and_legacy_config_wins() {
        let fixture = Fixture::new();
        let worktree = fixture.lf_worktree();
        let key = worktree.canonicalize().unwrap().display().to_string();
        let isolated = fixture.home.join("accounts/one");

        fixture.launch(&worktree, Some(&isolated));
        assert_eq!(
            read(&isolated.join(".claude.json"))["projects"][&key]["hasTrustDialogAccepted"],
            json!(true)
        );
        assert!(!fixture.config().exists());

        let legacy = isolated.join(".config.json");
        fs::write(&legacy, "{\"userID\": \"u\"}").unwrap();
        fixture.launch(&worktree, Some(&isolated));
        assert_eq!(
            read(&legacy),
            json!({"userID": "u", "projects": {key: {"hasTrustDialogAccepted": true}}})
        );
    }

    #[test]
    fn trusted_config_is_not_rewritten() {
        let fixture = Fixture::new();
        let worktree = fixture.lf_worktree();
        let key = worktree.canonicalize().unwrap().display().to_string();
        let compact = json!({"projects": {key: {"hasTrustDialogAccepted": true}}}).to_string();
        fs::write(fixture.config(), &compact).unwrap();

        fixture.launch(&worktree, None);

        assert_eq!(fs::read_to_string(fixture.config()).unwrap(), compact);
    }
}
