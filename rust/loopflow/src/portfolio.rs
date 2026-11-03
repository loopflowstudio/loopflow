//! Machine-local repository membership. Reading never registers Work or starts Sessions.
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::git::worktree_root;
use crate::git::worktrees::main_repo_root;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortfolioMode {
    Folder,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioRepo {
    pub path: String,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Portfolio {
    pub configured: bool,
    pub mode: PortfolioMode,
    pub root: String,
    pub repos: Vec<PortfolioRepo>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Selection {
    mode: PortfolioMode,
    paths: BTreeSet<PathBuf>,
}

#[derive(Debug, clap::Subcommand)]
pub enum PortfolioCommand {
    /// Discover immediate main checkouts in a folder (defaults to the saved folder)
    Folder { path: Option<PathBuf> },
    /// Use only explicitly selected repositories
    Manual,
    /// Add repositories to the saved manual selection
    Add {
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },
    /// Remove manual membership, preserving files and open workspaces
    Remove { path: PathBuf },
    /// Import previous Desktop membership only when no selection has been saved
    Import {
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },
}

fn absolute(path: &Path) -> Result<PathBuf> {
    let path = if let Ok(rest) = path.strip_prefix("~") {
        dirs::home_dir()
            .context("Home directory unavailable")?
            .join(rest)
    } else if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    Ok(path.canonicalize().unwrap_or(path))
}

fn main_repository(path: &Path) -> Result<PathBuf> {
    let path = absolute(path)?;
    if worktree_root(&path)?.canonicalize()? != path {
        bail!("{} is not a repository root", path.display());
    }
    let main = main_repo_root(&path)?.canonicalize()?;
    if worktree_root(&main)?.canonicalize()? != main {
        bail!("{} has no main checkout", path.display());
    }
    Ok(main)
}

fn selection(config: &serde_yaml_ng::Value) -> Result<Option<Selection>> {
    config
        .get("portfolio")
        .cloned()
        .map(serde_yaml_ng::from_value)
        .transpose()
        .context("Invalid portfolio configuration")
}

fn reading(config: &serde_yaml_ng::Value) -> Result<Portfolio> {
    let selected = selection(config)?;
    let root: PathBuf = config
        .get("repo_root")
        .cloned()
        .map(serde_yaml_ng::from_value)
        .transpose()?
        .unwrap_or(
            dirs::home_dir()
                .context("Home directory unavailable")?
                .join("src"),
        );
    let root = absolute(&root)?;
    let mode = selected.as_ref().map_or(PortfolioMode::Folder, |s| s.mode);
    let mut result = Portfolio {
        configured: selected.is_some(),
        mode,
        root: root.to_string_lossy().into_owned(),
        repos: vec![],
        error: None,
    };
    match mode {
        PortfolioMode::Folder => match fs::read_dir(&root) {
            Ok(entries) => {
                let mut paths = BTreeSet::new();
                for entry in entries {
                    let entry = entry.with_context(|| format!("Read {}", root.display()))?;
                    let path = entry.path();
                    if path
                        .file_name()
                        .is_some_and(|name| name.to_string_lossy().starts_with('.'))
                    {
                        continue;
                    }
                    match fs::symlink_metadata(path.join(".git")) {
                        Ok(_) => {}
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                        Err(error) => {
                            result.repos.push(PortfolioRepo {
                                path: path.to_string_lossy().into_owned(),
                                error: Some(error.to_string()),
                            });
                            continue;
                        }
                    }
                    match main_repository(&path) {
                        Ok(main) if path.canonicalize()? == main => {
                            paths.insert(main);
                        }
                        Ok(_) => {}
                        Err(error) => result.repos.push(PortfolioRepo {
                            path: path.to_string_lossy().into_owned(),
                            error: Some(error.to_string()),
                        }),
                    }
                }
                result
                    .repos
                    .extend(paths.into_iter().map(|path| PortfolioRepo {
                        path: path.to_string_lossy().into_owned(),
                        error: None,
                    }));
            }
            Err(error) => result.error = Some(format!("Read {}: {error}", root.display())),
        },
        PortfolioMode::Manual => {
            for path in selected.expect("manual mode requires a selection").paths {
                let error = main_repository(&path).err().map(|error| error.to_string());
                result.repos.push(PortfolioRepo {
                    path: path.to_string_lossy().into_owned(),
                    error,
                });
            }
        }
    }
    result.repos.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(result)
}

pub fn read() -> Result<Portfolio> {
    reading(&crate::config::read_global_config_at(
        &crate::store::lf_home_dir().join("config.yaml"),
    )?)
}

pub fn change(command: &PortfolioCommand) -> Result<Portfolio> {
    change_at(&crate::store::lf_home_dir(), command)
}

fn change_at(home: &Path, command: &PortfolioCommand) -> Result<Portfolio> {
    let config = crate::config::update_global_config_at(home, |config| {
        change_config(config, command)?;
        Ok(config.clone())
    })?;
    reading(&config)
}

fn change_config(config: &mut serde_yaml_ng::Value, command: &PortfolioCommand) -> Result<()> {
    let previous = selection(config)?;
    if matches!(command, PortfolioCommand::Import { .. }) && previous.is_some() {
        bail!("Portfolio already configured; existing selection was preserved");
    }
    let mut selected = previous.unwrap_or(Selection {
        mode: PortfolioMode::Folder,
        paths: BTreeSet::new(),
    });
    match command {
        PortfolioCommand::Folder { path } => {
            if let Some(path) = path {
                config["repo_root"] = serde_yaml_ng::to_value(absolute(path)?)?;
            }
            selected.mode = PortfolioMode::Folder;
        }
        PortfolioCommand::Manual => selected.mode = PortfolioMode::Manual,
        PortfolioCommand::Add { paths } => {
            for path in paths {
                selected.paths.insert(main_repository(path)?);
            }
        }
        PortfolioCommand::Import { paths } => {
            for path in paths {
                // Old entries may be unavailable. Preserve them for explicit removal.
                selected
                    .paths
                    .insert(main_repository(path).unwrap_or(absolute(path)?));
            }
            selected.mode = PortfolioMode::Manual;
        }
        PortfolioCommand::Remove { path } => {
            selected
                .paths
                .remove(&main_repository(path).unwrap_or(absolute(path)?));
        }
    }
    config["portfolio"] = serde_yaml_ng::to_value(selected)?;
    Ok(())
}

pub fn run(command: Option<&PortfolioCommand>, json: bool) -> Result<()> {
    let portfolio = match command {
        Some(command) => change(command)?,
        None => read()?,
    };
    if json {
        println!("{}", serde_json::to_string(&portfolio)?);
    } else {
        println!("{:?}: {}", portfolio.mode, portfolio.root);
        if let Some(error) = portfolio.error {
            eprintln!("{error}");
        }
        for repo in portfolio.repos {
            println!(
                "{}{}",
                repo.path,
                repo.error
                    .map(|error| format!(" — {error}"))
                    .unwrap_or_default()
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::{change_at, reading, Portfolio, PortfolioCommand, PortfolioMode};

    fn git(path: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn repo(path: &Path) -> PathBuf {
        fs::create_dir_all(path).unwrap();
        git(path, &["init", "-q"]);
        git(
            path,
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.test",
                "commit",
                "--allow-empty",
                "-qm",
                "fixture",
            ],
        );
        path.canonicalize().unwrap()
    }

    #[test]
    fn portfolio_modes_share_root_and_preserve_manual_membership() {
        let fixture = tempfile::tempdir().unwrap();
        let home = fixture.path().join("home");
        let root = fixture.path().join("src");
        let first = repo(&root.join("first"));
        let second = repo(&root.join("second"));
        let nested = repo(&root.join("group/nested"));
        let outside = repo(&fixture.path().join("elsewhere/repo"));
        let linked = root.join("linked");
        git(
            &first,
            &["worktree", "add", "-qb", "linked", linked.to_str().unwrap()],
        );
        let alias = fixture.path().join("alias");
        std::os::unix::fs::symlink(&first, &alias).unwrap();
        fs::create_dir_all(&home).unwrap();
        fs::write(home.join("config.yaml"), "agent: codex\n").unwrap();
        let folder = change_at(
            &home,
            &PortfolioCommand::Folder {
                path: Some(root.clone()),
            },
        )
        .unwrap();
        assert_eq!(
            folder
                .repos
                .iter()
                .map(|r| PathBuf::from(&r.path))
                .collect::<Vec<_>>(),
            [first.clone(), second]
        );
        change_at(
            &home,
            &PortfolioCommand::Add {
                paths: vec![
                    first.clone(),
                    linked,
                    alias,
                    nested.clone(),
                    outside.clone(),
                ],
            },
        )
        .unwrap();
        let manual = change_at(&home, &PortfolioCommand::Manual).unwrap();
        assert_eq!(manual.repos.len(), 3);
        assert!(manual.repos.iter().any(|r| Path::new(&r.path) == nested));
        let changed = change_at(
            &home,
            &PortfolioCommand::Folder {
                path: Some(fixture.path().join("elsewhere")),
            },
        )
        .unwrap();
        assert_eq!(changed.repos.len(), 1);
        assert_eq!(Path::new(&changed.repos[0].path), outside);
        assert_eq!(
            change_at(&home, &PortfolioCommand::Manual)
                .unwrap()
                .repos
                .len(),
            3
        );
        fs::remove_dir_all(&outside).unwrap();
        let unavailable =
            reading(&crate::config::read_global_config_at(&home.join("config.yaml")).unwrap())
                .unwrap();
        assert!(unavailable
            .repos
            .iter()
            .find(|r| Path::new(&r.path) == outside)
            .unwrap()
            .error
            .is_some());
        assert_eq!(
            change_at(&home, &PortfolioCommand::Remove { path: outside })
                .unwrap()
                .repos
                .len(),
            2
        );
        assert!(first.exists());
        assert_eq!(
            crate::config::read_global_config_at(&home.join("config.yaml")).unwrap()["agent"],
            "codex"
        );
        assert!(!home.join("state.db").exists());
    }

    #[test]
    fn portfolio_import_is_explicit_and_does_not_replace_cli_selection() {
        let fixture = tempfile::tempdir().unwrap();
        let home = fixture.path().join("home");
        let missing = fixture.path().join("missing");
        let empty =
            reading(&crate::config::read_global_config_at(&home.join("config.yaml")).unwrap())
                .unwrap();
        assert!(!empty.configured);
        assert_eq!(empty.mode, PortfolioMode::Folder);
        assert!(!home.exists());
        let imported = change_at(
            &home,
            &PortfolioCommand::Import {
                paths: vec![missing.clone()],
            },
        )
        .unwrap();
        assert_eq!(imported.mode, PortfolioMode::Manual);
        assert!(imported.repos[0].error.is_some());
        assert!(change_at(&home, &PortfolioCommand::Import { paths: vec![] }).is_err());
        let broken_folder = change_at(
            &home,
            &PortfolioCommand::Folder {
                path: Some(missing),
            },
        )
        .unwrap();
        assert!(broken_folder.error.is_some());
        assert!(broken_folder.repos.is_empty());
    }

    #[test]
    fn portfolio_dto_matches_shared_fixture() {
        let value: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/dto/portfolio.json"))
                .unwrap();
        let portfolio: Portfolio = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(portfolio).unwrap(), value);
    }
}
