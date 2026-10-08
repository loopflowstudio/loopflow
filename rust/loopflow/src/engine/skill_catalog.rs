//! One source selection for execution, help, Flow capture and personal export.
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::engine::{builtins, flow::split_frontmatter, LoadError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SkillDialect {
    Loopflow,
    Claude,
    Codex,
}

/// File provenance and declarations retained with a resolved Flow skill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillOrigin {
    pub path: PathBuf,
    pub dialect: SkillDialect,
    pub frontmatter: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SkillSource {
    pub name: String,
    pub path: Option<PathBuf>,
    pub dialect: SkillDialect,
}

impl SkillSource {
    pub fn load(&self) -> Result<crate::engine::Skill, LoadError> {
        crate::engine::flow::skill_from_source(self)
    }

    pub fn read(&self) -> Result<String, LoadError> {
        match &self.path {
            Some(path) => Ok(fs::read_to_string(path)?),
            None => Ok(builtins::get_builtin_skill(&self.name)
                .expect("embedded catalog entry names a builtin")
                .to_string()),
        }
    }
}

#[derive(Debug)]
pub struct SkillCatalog {
    sources: BTreeMap<String, SkillSource>,
}

impl SkillCatalog {
    pub fn discover(repo: Option<&Path>) -> Result<Self, LoadError> {
        Self::load(repo, dirs::home_dir().as_deref(), true)
    }

    pub(crate) fn load(
        repo: Option<&Path>,
        home: Option<&Path>,
        provider_overrides: bool,
    ) -> Result<Self, LoadError> {
        let mut catalog = Self {
            sources: BTreeMap::new(),
        };
        if let Some(repo) = repo {
            catalog.collect_scope(repo, false)?;
        }
        if let Some(home) = home {
            catalog.collect_scope(home, provider_overrides)?;
        }
        for name in builtins::builtin_skill_names() {
            catalog
                .sources
                .entry(name.to_string())
                .or_insert_with(|| SkillSource {
                    name: name.to_string(),
                    path: None,
                    dialect: SkillDialect::Loopflow,
                });
        }
        Ok(catalog)
    }

    pub fn entries(&self) -> impl Iterator<Item = &SkillSource> {
        self.sources.values()
    }

    pub fn resolve(&self, name: &str) -> Option<&SkillSource> {
        self.sources
            .get(name)
            .or_else(|| builtins::resolve_builtin_skill(name).and_then(|key| self.sources.get(key)))
    }

    fn collect_scope(&mut self, base: &Path, overrides: bool) -> Result<(), LoadError> {
        let provider_home = |variable: &str, folder: &str| {
            overrides
                .then(|| std::env::var_os(variable))
                .flatten()
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| base.join(folder))
        };
        let claude = provider_home("CLAUDE_CONFIG_DIR", ".claude");
        let codex = provider_home("CODEX_HOME", ".codex");
        for (root, dialect, files) in [
            (base.join(".lf/skills"), SkillDialect::Loopflow, true),
            (claude.join("skills"), SkillDialect::Claude, false),
            (claude.join("commands"), SkillDialect::Claude, true),
            (base.join(".agents/skills"), SkillDialect::Codex, false),
            (codex.join("skills"), SkillDialect::Codex, false),
            (codex.join("prompts"), SkillDialect::Codex, true),
        ] {
            self.collect(&root, &root, dialect, files, &mut HashSet::new())?;
        }
        Ok(())
    }

    fn collect(
        &mut self,
        root: &Path,
        dir: &Path,
        dialect: SkillDialect,
        files: bool,
        ancestors: &mut HashSet<PathBuf>,
    ) -> Result<(), LoadError> {
        if !dir.is_dir() {
            return Ok(());
        }
        let canonical = fs::canonicalize(dir)?;
        if !ancestors.insert(canonical.clone()) {
            return Ok(());
        }
        let bundle = dir.join("SKILL.md");
        if dir != root && bundle.is_file() {
            self.insert(root, bundle, dialect, true)?;
        } else {
            let mut paths = fs::read_dir(dir)?
                .map(|entry| entry.map(|entry| entry.path()))
                .collect::<Result<Vec<_>, _>>()?;
            paths.sort();
            for path in paths {
                if path.is_dir() {
                    self.collect(root, &path, dialect, files, ancestors)?;
                } else if files
                    && path.extension().is_some_and(|extension| extension == "md")
                    && path.file_name().is_some_and(|name| name != "SKILL.md")
                {
                    self.insert(root, path, dialect, false)?;
                }
            }
        }
        ancestors.remove(&canonical);
        Ok(())
    }

    fn insert(
        &mut self,
        root: &Path,
        path: PathBuf,
        dialect: SkillDialect,
        bundle: bool,
    ) -> Result<(), LoadError> {
        if is_generated(&path) {
            return Ok(());
        }
        let relative = path
            .strip_prefix(root)
            .expect("catalog paths stay beneath their source root");
        let name = if bundle {
            relative
                .parent()
                .expect("bundle has a directory")
                .to_path_buf()
        } else {
            relative.with_extension("")
        };
        let name = name
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        // Keep an absolute lexical path: a symlink's native name is part of its identity.
        let path = std::path::absolute(path)?;
        self.sources.entry(name.clone()).or_insert(SkillSource {
            name,
            path: Some(path),
            dialect,
        });
        Ok(())
    }
}

pub(crate) fn is_generated(path: &Path) -> bool {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| split_frontmatter(&content))
        .and_then(|(frontmatter, _)| {
            serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&frontmatter).ok()
        })
        .and_then(|value| {
            value
                .get("loopflow")
                .and_then(serde_yaml_ng::Value::as_bool)
        })
        == Some(true)
}

#[cfg(test)]
mod tests {
    use super::{SkillCatalog, SkillDialect};
    use crate::engine::skill_invocation::SkillInvocation;
    use std::{fs, path::Path};
    use tempfile::TempDir;

    fn write(root: &Path, relative: &str, content: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    #[test]
    fn precedence_bundles_and_generated_exports_share_one_selection() {
        let repo = TempDir::new().unwrap();
        let home = TempDir::new().unwrap();
        let sources = [
            ".lf/skills/audit.md",
            ".claude/skills/audit/SKILL.md",
            ".claude/commands/audit.md",
            ".agents/skills/audit/SKILL.md",
            ".codex/skills/audit/SKILL.md",
            ".codex/prompts/audit.md",
        ];
        for root in [repo.path(), home.path()] {
            for path in sources {
                write(root, path, path);
            }
        }
        write(
            repo.path(),
            ".claude/skills/audit/references/guide.md",
            "not a skill",
        );
        write(
            repo.path(),
            ".agents/skills/implement/SKILL.md",
            "---\nloopflow: true\n---\nStale",
        );
        for path in sources {
            let catalog = SkillCatalog::load(Some(repo.path()), Some(home.path()), false).unwrap();
            let selected = catalog.resolve("audit").unwrap();
            assert_eq!(
                selected.path.as_deref(),
                Some(repo.path().join(path).as_path())
            );
            assert_eq!(selected.read().unwrap(), path);
            assert!(!catalog.entries().any(|entry| entry.name.contains("guide")));
            assert!(catalog.resolve("implement").unwrap().path.is_none());
            fs::remove_file(repo.path().join(path)).unwrap();
        }
        assert_eq!(
            SkillCatalog::load(Some(repo.path()), Some(home.path()), false)
                .unwrap()
                .resolve("audit")
                .unwrap()
                .path,
            Some(home.path().join(".lf/skills/audit.md"))
        );
    }

    #[test]
    fn unfamiliar_native_frontmatter_remains_plain_instructions() {
        let repo = TempDir::new().unwrap();
        let content = "---\nallowed-tools: [\n---\nRun this unfamiliar skill.";
        write(repo.path(), ".claude/skills/audit/SKILL.md", content);
        let catalog = SkillCatalog::load(Some(repo.path()), None, false).unwrap();
        let skill = catalog.resolve("audit").unwrap().load().unwrap();
        assert_eq!(skill.content.as_deref(), Some("Run this unfamiliar skill."));
        assert_eq!(
            SkillInvocation {
                skill,
                arguments: String::new(),
            }
            .source_text(),
            content
        );
        write(repo.path(), ".lf/skills/audit.md", content);
        let catalog = SkillCatalog::load(Some(repo.path()), None, false).unwrap();
        assert!(matches!(
            catalog.resolve("audit").unwrap().load(),
            Err(crate::engine::LoadError::InvalidSkill(_))
        ));
    }

    #[test]
    fn provider_home_overrides_select_the_native_files() {
        let _lock = crate::journal::test_env_lock();
        let home = TempDir::new().unwrap();
        let provider = TempDir::new().unwrap();
        let _environment =
            crate::test_ambient::EnvGuard::clear(&["CLAUDE_CONFIG_DIR", "CODEX_HOME"]);
        std::env::set_var("CLAUDE_CONFIG_DIR", provider.path());
        std::env::set_var("CODEX_HOME", provider.path());
        write(provider.path(), "skills/audit/SKILL.md", "Override");
        write(
            home.path(),
            ".claude/skills/audit/SKILL.md",
            "Ignored default",
        );
        let catalog = SkillCatalog::load(None, Some(home.path()), true).unwrap();
        assert_eq!(
            catalog.resolve("audit").unwrap().read().unwrap(),
            "Override"
        );
    }

    #[test]
    fn namespaces_and_symlink_cycles_terminate_without_exposing_assets() {
        let repo = TempDir::new().unwrap();
        write(repo.path(), ".claude/skills/team/audit/SKILL.md", "Audit");
        write(
            repo.path(),
            ".claude/skills/team/audit/extra/SKILL.md",
            "bundled example",
        );
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            repo.path().join(".claude/skills"),
            repo.path().join(".claude/skills/cycle"),
        )
        .unwrap();
        let catalog = SkillCatalog::load(Some(repo.path()), None, false).unwrap();
        assert_eq!(
            catalog.resolve("team/audit").unwrap().dialect,
            SkillDialect::Claude
        );
        assert!(!catalog
            .entries()
            .any(|entry| entry.name.contains("extra") || entry.name.contains("cycle")));
    }
}
