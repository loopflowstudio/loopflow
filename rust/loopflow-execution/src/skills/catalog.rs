//! One source selection for execution, help, Graph capture and personal export.
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;

use crate::error::LoadError;
use crate::graph::Skill;
use loopflow_definitions::builtins;
use loopflow_definitions::name::resolve_name;
use loopflow_definitions::text::split_frontmatter;

static RETIRED_INTERACTIVE_WARNING: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SkillDialect {
    Loopflow,
    Claude,
    Codex,
}

/// File provenance and declarations retained with a resolved Graph skill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillOrigin {
    /// Older durable captures did not record authority; their path remains the locator.
    pub config_source: Option<crate::config::ConfigSource>,
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
    fn parse_skill(&self, content: &str) -> Result<Skill, LoadError> {
        let (frontmatter, body) = match split_frontmatter(content) {
            Some((frontmatter, body)) => (Some(frontmatter), body),
            None => (None, content),
        };
        let metadata = frontmatter
            .map(serde_yaml_ng::from_str::<Value>)
            .transpose();
        let loopflow = self.dialect == SkillDialect::Loopflow;
        let metadata = match metadata {
            Ok(value) => value,
            Err(error) if loopflow => return Err(LoadError::InvalidSkill(error.to_string())),
            Err(_) => None, // Unfamiliar native declarations remain instructions.
        };
        let metadata = metadata.as_ref().and_then(Value::as_mapping);
        let field = |name: &str| {
            metadata
                .and_then(|map| map.get(name))
                .and_then(Value::as_str)
                .map(str::to_string)
        };
        if metadata.is_some_and(|map| map.contains_key("interactive"))
            && !RETIRED_INTERACTIVE_WARNING.swap(true, Ordering::Relaxed)
        {
            eprintln!(
                "warning: skill {:?} uses retired `interactive` frontmatter; a graph marks the step `interactive: <skill>`, and -i or -b choose for a whole launch",
                self.name
            );
        }
        let mut settings = if loopflow {
            crate::config::settings::metadata(
                &metadata.cloned().map(Value::Mapping).unwrap_or(Value::Null),
            )
            .map_err(|error| LoadError::InvalidSkill(error.to_string()))?
        } else {
            BTreeMap::new()
        };
        settings.remove("agent");
        let docs = settings
            .get_mut("context")
            .and_then(Value::as_mapping_mut)
            .and_then(|context| context.remove("docs"))
            .map(|value| {
                serde_yaml_ng::from_value(value).expect("metadata validates document paths")
            });
        Ok(Skill {
            name: self.name.clone(),
            source: self.path.as_ref().map(|path| SkillOrigin {
                config_source: Some(crate::config::settings::definition_source(Some(path))),
                path: path.clone(),
                dialect: self.dialect,
                frontmatter: frontmatter.map(str::to_string),
            }),
            // Claude's `agent: Explore` describes its subagent, not an lf harness.
            agent: loopflow.then(|| field("agent")).flatten(),
            occurrence: None,
            settings,
            action_style: field("action_style"),
            docs,
            content: Some(body.to_string()),
        })
    }
}

#[derive(Clone, Copy)]
enum ExtendsTarget<'a> {
    Builtin(&'static str),
    Catalog(&'a SkillSource),
}

fn declared_extends(content: &str) -> Option<String> {
    let (frontmatter, _) = split_frontmatter(content)?;
    serde_yaml_ng::from_str::<serde_yaml_ng::Mapping>(frontmatter)
        .ok()?
        .get("extends")?
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn merge_extension(
    parent: &str,
    parent_dialect: SkillDialect,
    child_meta: serde_yaml_ng::Mapping,
    child_body: &str,
) -> Result<String, LoadError> {
    let (parent_frontmatter, parent_body) = split_frontmatter(parent).unwrap_or(("", parent));
    let mut merged = if parent_dialect == SkillDialect::Loopflow {
        let mut value = if parent_frontmatter.trim().is_empty() {
            Value::Mapping(serde_yaml_ng::Mapping::new())
        } else {
            match serde_yaml_ng::from_str::<Value>(parent_frontmatter)
                .map_err(|error| LoadError::InvalidSkill(error.to_string()))?
            {
                Value::Null => Value::Mapping(serde_yaml_ng::Mapping::new()),
                Value::Mapping(map) => Value::Mapping(map),
                _ => {
                    return Err(LoadError::InvalidSkill(
                        "skill frontmatter must be a YAML mapping".into(),
                    ))
                }
            }
        };
        crate::config::normalize_context(&mut value);
        value
    } else {
        // Native Claude/Codex frontmatter (e.g. `context: fork`, `agent: Explore`)
        // belongs to the provider's schema, not Loopflow launch settings.
        let map = serde_yaml_ng::from_str::<Value>(parent_frontmatter)
            .ok()
            .and_then(|value| {
                value
                    .get("description")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .map(|description| {
                serde_yaml_ng::Mapping::from_iter([("description".into(), description.into())])
            })
            .unwrap_or_default();
        Value::Mapping(map)
    };
    let mut child = Value::Mapping(child_meta);
    crate::config::normalize_context(&mut child);
    if let (Some(parent_map), Some(child_map)) = (merged.as_mapping_mut(), child.as_mapping()) {
        for (set, clear) in [("agent", "default_agent"), ("default_agent", "agent")] {
            if child_map.get(set).is_some_and(|value| !value.is_null()) {
                parent_map.remove(clear);
            }
        }
    }
    crate::config::files::overlay(&mut merged, &child);
    let merged = merged
        .as_mapping()
        .expect("normalized skill frontmatter is a mapping");
    let body = match (parent_body.trim_end(), child_body.trim_start()) {
        ("", child) => child.to_string(),
        (base, "") => format!("{base}\n"),
        (base, child) => format!("{base}\n\n{child}"),
    };
    if merged.is_empty() {
        return Ok(body);
    }
    let frontmatter = serde_yaml_ng::to_string(merged)
        .map_err(|error| LoadError::InvalidSkill(error.to_string()))?;
    Ok(format!("---\n{frontmatter}---\n{body}"))
}

#[derive(Debug)]
pub struct SkillCatalog {
    pub repo: Option<PathBuf>,
    sources: BTreeMap<String, SkillSource>,
}

impl SkillCatalog {
    pub fn discover(repo: Option<&Path>) -> Result<Arc<Self>, LoadError> {
        Self::load(repo, dirs::home_dir().as_deref(), true)
    }

    /// The skills under `repo` and `home`. Served readings share one catalog
    /// per set of directories until a file under them changes.
    pub fn load(
        repo: Option<&Path>,
        home: Option<&Path>,
        provider_overrides: bool,
    ) -> Result<Arc<Self>, LoadError> {
        // Where a provider keeps its own skills is read here, so the shared
        // catalog is kept under the directories it walked.
        let provider_home = |variable: &str| {
            provider_overrides
                .then(|| std::env::var_os(variable))
                .flatten()
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        };
        let claude = provider_home("CLAUDE_CONFIG_DIR");
        let codex = provider_home("CODEX_HOME");
        let key = crate::observe::Derived::SkillCatalog {
            repo: repo.map(Path::to_path_buf),
            home: home.map(Path::to_path_buf),
            claude: claude.clone(),
            codex: codex.clone(),
        };
        (*crate::observe::derived(key, || {
            Self::walk(repo, home, claude.as_deref(), codex.as_deref()).map(Arc::new)
        }))
        .clone()
    }

    fn walk(
        repo: Option<&Path>,
        home: Option<&Path>,
        claude: Option<&Path>,
        codex: Option<&Path>,
    ) -> Result<Self, LoadError> {
        let mut catalog = Self {
            repo: repo.map(Path::to_path_buf),
            sources: BTreeMap::new(),
        };
        if let Some(repo) = repo {
            catalog.collect_scope(repo, None, None)?;
        }
        if let Some(home) = home {
            catalog.collect_scope(home, claude, codex)?;
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

    pub fn load_skill(&self, name: &str) -> Result<Skill, LoadError> {
        let source = self
            .resolve(name)?
            .ok_or_else(|| LoadError::SkillNotFound(name.to_string()))?;
        self.load_source(source)
    }

    pub fn load_source(&self, source: &SkillSource) -> Result<Skill, LoadError> {
        let content = self.read_source(source)?;
        source.parse_skill(&content)
    }

    /// The skill's resolved text. A Loopflow file with `extends:` inherits its
    /// parent's frontmatter (child keys win, `extends` stripped) and appends
    /// its body after the parent's:
    /// - `extends: lf:<name>` targets Loopflow's builtin `<name>` directly.
    /// - `extends: <name>` resolves `<name>` through the catalog; when a file
    ///   extends its own name, it falls through to the builtin of that name.
    /// - `extends: builtin` is a temporary compatibility alias for released lf
    ///   0.16.0 (LOO-1107 removes it).
    pub fn read_source(&self, source: &SkillSource) -> Result<String, LoadError> {
        self.read_source_with(source, &mut Vec::new())
    }

    fn read_source_with<'a>(
        &'a self,
        source: &'a SkillSource,
        visiting: &mut Vec<&'a str>,
    ) -> Result<String, LoadError> {
        let Some(path) = &source.path else {
            return Ok(builtins::get_builtin_skill(&source.name)
                .expect("embedded catalog entry names a builtin")
                .to_string());
        };
        let content = crate::observe::read_to_string(path)?;
        if source.dialect != SkillDialect::Loopflow {
            return Ok(content);
        }
        let Some((frontmatter, addition)) = split_frontmatter(&content) else {
            return Ok(content);
        };
        let Ok(mut child_meta) = serde_yaml_ng::from_str::<serde_yaml_ng::Mapping>(frontmatter)
        else {
            return Ok(content);
        };
        let Some(extends) = child_meta.remove("extends") else {
            return Ok(content);
        };
        let Some(target) = extends.as_str().map(str::trim).filter(|s| !s.is_empty()) else {
            return Err(LoadError::InvalidSkill(format!(
                "{}: `extends` must name a skill or `lf:<name>`",
                path.display()
            )));
        };
        let (parent_content, parent_dialect) =
            self.resolve_parent(source, path, target, visiting)?;
        merge_extension(&parent_content, parent_dialect, child_meta, addition)
    }

    fn resolve_extends_target(
        &self,
        source: &SkillSource,
        path: &Path,
        target: &str,
    ) -> Result<ExtendsTarget<'_>, LoadError> {
        let resolve_builtin = |name: &str| {
            builtins::resolve_builtin_skill(name)
                .map(ExtendsTarget::Builtin)
                .ok_or_else(|| {
                    LoadError::InvalidSkill(format!(
                        "{}: no builtin skill is named {name:?}",
                        path.display()
                    ))
                })
        };
        if target == "builtin" {
            // Temporary compatibility alias for released lf 0.16.0; LOO-1107 removes
            // this and migrates `.lf/skills/*.md` to `extends: lf:<name>`.
            return resolve_builtin(&source.name);
        }
        if let Some(builtin_name) = target.strip_prefix("lf:") {
            let builtin_name = builtin_name.trim();
            if builtin_name.is_empty() {
                return Err(LoadError::InvalidSkill(format!(
                    "{}: `extends` must name a builtin after `lf:`",
                    path.display()
                )));
            }
            return resolve_builtin(builtin_name);
        }
        let parent = self.resolve(target)?.ok_or_else(|| {
            LoadError::InvalidSkill(format!("{}: no skill is named {target:?}", path.display()))
        })?;
        if parent.name == source.name {
            return resolve_builtin(&source.name);
        }
        Ok(ExtendsTarget::Catalog(parent))
    }

    fn resolve_parent<'a>(
        &'a self,
        source: &'a SkillSource,
        path: &Path,
        target: &str,
        visiting: &mut Vec<&'a str>,
    ) -> Result<(String, SkillDialect), LoadError> {
        match self.resolve_extends_target(source, path, target)? {
            ExtendsTarget::Builtin(canonical) => Ok((
                builtins::get_builtin_skill(canonical)
                    .expect("resolved builtin exists")
                    .to_string(),
                SkillDialect::Loopflow,
            )),
            ExtendsTarget::Catalog(parent) => {
                visiting.push(&source.name);
                if let Some(start) = visiting.iter().position(|name| *name == parent.name) {
                    let cycle = visiting[start..]
                        .iter()
                        .copied()
                        .chain(std::iter::once(parent.name.as_str()))
                        .collect::<Vec<_>>()
                        .join(" -> ");
                    return Err(LoadError::InvalidSkill(format!(
                        "{}: skill inheritance cycle: {cycle}",
                        path.display()
                    )));
                }
                let content = self.read_source_with(parent, visiting)?;
                visiting.pop();
                Ok((content, parent.dialect))
            }
        }
    }

    /// How `source` stands to the builtin of the same name, and its normalized `extends:` target.
    pub fn relations(
        &self,
        source: &SkillSource,
    ) -> (
        Option<crate::skills::listing::BuiltinRelation>,
        Option<String>,
    ) {
        use crate::skills::listing::BuiltinRelation;
        let Some(path) = source.path.as_ref() else {
            return (None, None);
        };
        let Ok(content) = crate::observe::read_to_string(path) else {
            return (None, None);
        };
        let raw_extends = (source.dialect == SkillDialect::Loopflow)
            .then(|| declared_extends(&content))
            .flatten();
        let first_step = raw_extends
            .as_deref()
            .and_then(|target| self.resolve_extends_target(source, path, target).ok());
        let extends = raw_extends.map(|target| match first_step {
            Some(ExtendsTarget::Builtin(canonical)) => format!("lf:{canonical}"),
            _ => target,
        });
        let builtin = builtins::resolve_builtin_skill(&source.name).and_then(|canonical| {
            let builtin = builtins::get_builtin_skill(canonical)?;
            let relation = if first_step
                .is_some_and(|step| self.chain_extends_builtin(&source.name, step, canonical))
            {
                BuiltinRelation::Extends
            } else if content.trim() == builtin.trim() {
                BuiltinRelation::Copies
            } else {
                BuiltinRelation::Replaces
            };
            Some(relation)
        });
        (builtin, extends)
    }

    fn chain_extends_builtin(
        &self,
        start: &str,
        first: ExtendsTarget<'_>,
        canonical: &str,
    ) -> bool {
        let mut visited = HashSet::from([start]);
        let mut next = Some(first);
        while let Some(step) = next {
            match step {
                ExtendsTarget::Builtin(name) => return name == canonical,
                ExtendsTarget::Catalog(parent) => {
                    if !visited.insert(parent.name.as_str()) {
                        return false;
                    }
                    let Some(parent_path) = parent.path.as_ref() else {
                        return parent.name == canonical;
                    };
                    if parent.dialect != SkillDialect::Loopflow {
                        return false;
                    }
                    next = crate::observe::read_to_string(parent_path)
                        .ok()
                        .and_then(|content| declared_extends(&content))
                        .and_then(|target| {
                            self.resolve_extends_target(parent, parent_path, &target)
                                .ok()
                        });
                }
            }
        }
        false
    }

    pub fn entries(&self) -> impl Iterator<Item = &SkillSource> {
        self.sources.values()
    }

    pub fn exact(&self, name: &str) -> Option<&SkillSource> {
        self.sources.get(name)
    }

    pub fn resolve(&self, name: &str) -> Result<Option<&SkillSource>, LoadError> {
        let selected =
            resolve_name(name, self.sources.keys().map(String::as_str)).map_err(|names| {
                LoadError::InvalidSkill(format!("ambiguous skill {name:?}: {}", names.join(", ")))
            })?;
        Ok(selected
            .and_then(|name| self.sources.get(name))
            .or_else(|| {
                builtins::resolve_builtin_skill(name).and_then(|key| self.sources.get(key))
            }))
    }

    fn collect_scope(
        &mut self,
        base: &Path,
        claude: Option<&Path>,
        codex: Option<&Path>,
    ) -> Result<(), LoadError> {
        let claude = claude.map_or_else(|| base.join(".claude"), Path::to_path_buf);
        let codex = codex.map_or_else(|| base.join(".codex"), Path::to_path_buf);
        for (root, dialect, files) in [
            (base.join(".lf/skills"), SkillDialect::Loopflow, true),
            (claude.join("skills"), SkillDialect::Claude, false),
            (claude.join("commands"), SkillDialect::Claude, true),
            (base.join(".agents/skills"), SkillDialect::Codex, false),
            (codex.join("skills"), SkillDialect::Codex, false),
            (codex.join("prompts"), SkillDialect::Codex, true),
        ] {
            let mut scope = Self {
                repo: self.repo.clone(),
                sources: BTreeMap::new(),
            };
            scope.collect(&root, &root, dialect, files, &mut HashSet::new())?;
            for (key, source) in scope.sources {
                self.sources.entry(key).or_insert(source);
            }
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
        if !crate::observe::is_dir(dir) {
            return Ok(());
        }
        let canonical = fs::canonicalize(dir)?;
        if !ancestors.insert(canonical.clone()) {
            return Ok(());
        }
        let bundle = dir.join("SKILL.md");
        if dir != root && crate::observe::is_file(&bundle) {
            self.insert(root, bundle, dialect, true)?;
        } else {
            let paths = crate::observe::read_dir(dir)?;
            for path in paths {
                if crate::observe::is_dir(&path) {
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
        if is_generated(&path) {
            return Ok(());
        }
        // Keep an absolute lexical path: a symlink's native name is part of its identity.
        let path = std::path::absolute(path)?;
        if let Some(previous) = self.sources.get(&name) {
            let previous = previous.path.as_ref().expect("scope contains file sources");
            if fs::canonicalize(previous)? == fs::canonicalize(&path)? {
                return Ok(());
            }
            return Err(LoadError::InvalidSkill(format!(
                "ambiguous skill {name:?}: {} and {}",
                previous.display(),
                path.display()
            )));
        }
        self.sources.insert(
            name.clone(),
            SkillSource {
                name,
                path: Some(path),
                dialect,
            },
        );
        Ok(())
    }
}

pub(crate) fn is_generated(path: &Path) -> bool {
    let Ok(content) = crate::observe::read_to_string(path) else {
        return false;
    };
    split_frontmatter(&content)
        .and_then(|(frontmatter, _)| serde_yaml_ng::from_str::<Value>(frontmatter).ok())
        .and_then(|value| value.get("loopflow").and_then(Value::as_bool))
        == Some(true)
}

#[cfg(test)]
mod tests {
    use super::{SkillCatalog, SkillDialect};
    use std::{fs, path::Path};
    use tempfile::TempDir;

    #[test]
    fn readings_share_one_catalog_until_a_skill_file_changes() {
        use crate::observe::{Cache, Derived};
        let repo = TempDir::new().unwrap();
        write(repo.path(), ".lf/skills/mine.md", "Mine.\n");
        let cache = std::sync::Arc::new(Cache::default());
        let load = || SkillCatalog::load(Some(repo.path()), None, false).unwrap();
        let walks = || {
            cache.computations(
                |key| matches!(key, Derived::SkillCatalog { repo: Some(path), .. } if path == repo.path()),
            )
        };
        let (first, dependencies) = cache.capture(load);
        let (second, _) = cache.capture(load);
        assert!(std::sync::Arc::ptr_eq(&first, &second));
        assert_eq!(walks(), 1);

        write(repo.path(), ".lf/skills/group/other/SKILL.md", "Other.\n");
        assert!(
            load().exact("group/other").is_some(),
            "a bare command walks"
        );
        assert!(cache.capture(load).0.exact("group/other").is_none());
        cache.refresh_all();
        assert!(dependencies.changed());
        assert!(cache.capture(load).0.exact("group/other").is_some());
        assert_eq!(walks(), 2);
    }

    fn write(root: &Path, relative: &str, content: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    #[test]
    fn a_repository_skill_can_append_to_the_builtin_it_names() {
        let repo = TempDir::new().unwrap();
        write(
            repo.path(),
            ".lf/skills/gate.md",
            "---\nextends: builtin\n---\n## Here\n\nRun the local suite first.\n",
        );
        write(
            repo.path(),
            ".lf/skills/mine.md",
            "---\nextends: builtin\n---\nNothing to extend.\n",
        );
        let catalog = SkillCatalog::load(Some(repo.path()), None, false).unwrap();
        let builtin = SkillCatalog::load(None, None, false)
            .unwrap()
            .load_skill("gate")
            .unwrap()
            .content
            .unwrap();
        let extended = catalog.load_skill("gate").unwrap();
        assert_eq!(
            extended.content.unwrap(),
            format!(
                "{}\n\n## Here\n\nRun the local suite first.\n",
                builtin.trim_end()
            )
        );
        assert!(
            !extended
                .source
                .unwrap()
                .frontmatter
                .unwrap_or_default()
                .contains("extends:"),
            "merged frontmatter strips extends"
        );
        let error = catalog.load_skill("mine").unwrap_err().to_string();
        assert!(error.contains("no builtin skill is named"), "{error}");
    }

    #[test]
    fn skills_can_extend_named_skills_and_explicit_builtins() {
        let repo = TempDir::new().unwrap();
        // Same-name `extends: implement` falls through to builtin `implement`.
        write(
            repo.path(),
            ".lf/skills/implement.md",
            "---\nextends: implement\ndefault_agent: codex\ncontext:\n  docs: [scripts/testing.md]\n---\n## Repo rules\n\nRun focused tests.\n",
        );
        // Named extension inherits the repo's resolved `implement` (builtin + repo overlay),
        // overrides `default_agent`, and keeps inherited `context.docs` and `action_style`.
        write(
            repo.path(),
            ".lf/skills/card-implement.md",
            "---\nextends: implement\ndefault_agent: claude:opus\n---\n## Card rules\n\nCheck card transitions.\n",
        );
        // Explicit `extends: lf:implement` targets raw builtin `implement`, skipping `.lf/skills/implement.md`.
        write(
            repo.path(),
            ".lf/skills/raw-implement.md",
            "---\nextends: lf:implement\n---\n## Raw rules\n\nSkip repo overlay.\n",
        );
        // Named extension falls back to builtin when no repo file overrides the target.
        write(
            repo.path(),
            ".lf/skills/fast-qa.md",
            "---\nextends: qa\n---\n## Fast QA\n",
        );

        let catalog = SkillCatalog::load(Some(repo.path()), None, false).unwrap();
        let builtin_implement = SkillCatalog::load(None, None, false)
            .unwrap()
            .load_skill("implement")
            .unwrap();
        let builtin_qa = SkillCatalog::load(None, None, false)
            .unwrap()
            .load_skill("qa")
            .unwrap();

        let card = catalog.load_skill("card-implement").unwrap();
        assert_eq!(
            card.content.as_deref(),
            Some(
                format!(
                    "{}\n\n## Repo rules\n\nRun focused tests.\n\n## Card rules\n\nCheck card transitions.\n",
                    builtin_implement.content.as_deref().unwrap().trim_end()
                )
                .as_str()
            )
        );
        assert_eq!(
            card.settings["default_agent"].as_str(),
            Some("claude:opus"),
            "child frontmatter key wins over parent"
        );
        assert_eq!(
            card.docs.as_deref(),
            Some(&["scripts/testing.md".to_string()][..]),
            "parent frontmatter keys carry through"
        );
        assert_eq!(
            card.action_style, builtin_implement.action_style,
            "grandparent builtin frontmatter carries through"
        );
        assert!(
            !card
                .source
                .as_ref()
                .unwrap()
                .frontmatter
                .as_deref()
                .unwrap_or_default()
                .contains("extends:"),
            "merged frontmatter strips extends"
        );

        let raw = catalog.load_skill("raw-implement").unwrap();
        assert_eq!(
            raw.content.as_deref(),
            Some(
                format!(
                    "{}\n\n## Raw rules\n\nSkip repo overlay.\n",
                    builtin_implement.content.as_deref().unwrap().trim_end()
                )
                .as_str()
            )
        );
        assert!(!raw.settings.contains_key("default_agent"));
        assert!(raw.docs.is_none());

        let fast_qa = catalog.load_skill("fast-qa").unwrap();
        assert_eq!(
            fast_qa.content.as_deref(),
            Some(
                format!(
                    "{}\n\n## Fast QA\n",
                    builtin_qa.content.as_deref().unwrap().trim_end()
                )
                .as_str()
            )
        );
    }

    #[test]
    fn skill_inheritance_rejects_missing_targets_and_cycles() {
        let repo = TempDir::new().unwrap();
        write(
            repo.path(),
            ".lf/skills/missing.md",
            "---\nextends: does-not-exist\n---\nBody.\n",
        );
        write(
            repo.path(),
            ".lf/skills/missing-builtin.md",
            "---\nextends: lf:does-not-exist\n---\nBody.\n",
        );
        write(
            repo.path(),
            ".lf/skills/self-cycle.md",
            "---\nextends: self-cycle\n---\nBody.\n",
        );
        write(
            repo.path(),
            ".lf/skills/alpha.md",
            "---\nextends: beta\n---\nAlpha.\n",
        );
        write(
            repo.path(),
            ".lf/skills/beta.md",
            "---\nextends: alpha\n---\nBeta.\n",
        );
        write(
            repo.path(),
            ".lf/skills/implement.md",
            "---\nextends: loop-helper\n---\nImpl.\n",
        );
        write(
            repo.path(),
            ".lf/skills/loop-helper.md",
            "---\nextends: implement\n---\nHelper.\n",
        );
        let catalog = SkillCatalog::load(Some(repo.path()), None, false).unwrap();

        let err = catalog.load_skill("missing").unwrap_err();
        assert!(matches!(err, crate::error::LoadError::InvalidSkill(_)));
        assert!(err.to_string().contains("no skill is named"), "{err}");

        let err = catalog.load_skill("missing-builtin").unwrap_err();
        assert!(matches!(err, crate::error::LoadError::InvalidSkill(_)));
        assert!(
            err.to_string().contains("no builtin skill is named"),
            "{err}"
        );

        let err = catalog.load_skill("self-cycle").unwrap_err();
        assert!(matches!(err, crate::error::LoadError::InvalidSkill(_)));
        assert!(
            err.to_string()
                .contains("no builtin skill is named \"self-cycle\""),
            "{err}"
        );

        let err = catalog.load_skill("alpha").unwrap_err();
        assert!(matches!(err, crate::error::LoadError::InvalidSkill(_)));
        assert!(err.to_string().contains("alpha -> beta -> alpha"), "{err}");

        let err = catalog.load_skill("implement").unwrap_err();
        assert!(matches!(err, crate::error::LoadError::InvalidSkill(_)));
        assert!(
            err.to_string()
                .contains("implement -> loop-helper -> implement"),
            "{err}"
        );

        write(
            repo.path(),
            ".lf/skills/broken-parent.md",
            "---\ndefault_agent: [\n---\nBroken parent.\n",
        );
        write(
            repo.path(),
            ".lf/skills/child-of-broken.md",
            "---\nextends: broken-parent\n---\nChild.\n",
        );
        write(
            repo.path(),
            ".lf/skills/list-parent.md",
            "---\n- not-a-mapping\n---\nList parent.\n",
        );
        write(
            repo.path(),
            ".lf/skills/child-of-list.md",
            "---\nextends: list-parent\n---\nChild.\n",
        );
        let catalog = SkillCatalog::load(Some(repo.path()), None, false).unwrap();
        assert!(matches!(
            catalog.load_skill("child-of-broken"),
            Err(crate::error::LoadError::InvalidSkill(_))
        ));
        assert!(matches!(
            catalog.load_skill("child-of-list"),
            Err(crate::error::LoadError::InvalidSkill(_))
        ));
    }

    #[test]
    fn extending_native_skill_inherits_only_body_and_description() {
        let repo = TempDir::new().unwrap();
        write(
            repo.path(),
            ".claude/skills/audit/SKILL.md",
            "---\ndescription: Audit diff\nagent: Explore\ncontext: fork\ndefault_agent: native-persona\nallowed-tools: Read\n---\nCheck every invariant.\n",
        );
        write(
            repo.path(),
            ".lf/skills/my-audit.md",
            "---\nextends: audit\n---\nCheck repo-specific rules.\n",
        );
        write(
            repo.path(),
            ".lf/skills/custom-audit.md",
            "---\nextends: audit\ndescription: Custom audit\n---\nCustom rules.\n",
        );
        write(
            repo.path(),
            ".claude/skills/unfamiliar/SKILL.md",
            "---\nallowed-tools: [\n---\nUnfamiliar native body.\n",
        );
        write(
            repo.path(),
            ".lf/skills/my-unfamiliar.md",
            "---\nextends: unfamiliar\n---\nExtra step.\n",
        );

        let catalog = SkillCatalog::load(Some(repo.path()), None, false).unwrap();

        let my_audit = catalog.load_skill("my-audit").unwrap();
        assert!(my_audit.agent.is_none());
        assert!(my_audit.settings.is_empty());
        assert_eq!(
            my_audit.content.as_deref(),
            Some("Check every invariant.\n\nCheck repo-specific rules.\n")
        );
        let my_audit_text = catalog
            .read_source(catalog.resolve("my-audit").unwrap().unwrap())
            .unwrap();
        assert_eq!(
            loopflow_definitions::text::skill_description(&my_audit_text).as_deref(),
            Some("Audit diff")
        );

        let custom_audit_text = catalog
            .read_source(catalog.resolve("custom-audit").unwrap().unwrap())
            .unwrap();
        assert_eq!(
            loopflow_definitions::text::skill_description(&custom_audit_text).as_deref(),
            Some("Custom audit")
        );

        let my_unfamiliar = catalog.load_skill("my-unfamiliar").unwrap();
        assert!(my_unfamiliar.agent.is_none());
        assert!(my_unfamiliar.settings.is_empty());
        assert_eq!(
            my_unfamiliar.content.as_deref(),
            Some("Unfamiliar native body.\n\nExtra step.\n")
        );
    }

    #[test]
    fn skill_extension_merges_settings_by_semantics() {
        let repo = TempDir::new().unwrap();
        write(
            repo.path(),
            ".lf/skills/base-docs.md",
            "---\ncontext:\n  docs: [a.md]\n  budgets:\n    memory_tokens: 1000\n    memory_bytes: 2000\nclaude:\n  model: sonnet\ncodex:\n  service_tier: default\n---\nBase.\n",
        );
        // Flat `docs:` normalizes into `context.docs` and overrides parent's `context.docs`.
        write(
            repo.path(),
            ".lf/skills/override-docs.md",
            "---\nextends: base-docs\ndocs: [b.md]\n---\nChild.\n",
        );
        // Nested `context:` and `context.budgets` merge recursively without wiping siblings.
        write(
            repo.path(),
            ".lf/skills/merge-context.md",
            "---\nextends: base-docs\ncontext:\n  diff_files: false\n  budgets:\n    memory_tokens: 500\ncodex:\n  service_tier: priority\n---\nChild.\n",
        );
        // Child `default_agent` clears inherited pinned `agent`, and vice versa.
        write(
            repo.path(),
            ".lf/skills/pinned-parent.md",
            "---\nagent: codex\n---\nPinned.\n",
        );
        write(
            repo.path(),
            ".lf/skills/default-child.md",
            "---\nextends: pinned-parent\ndefault_agent: claude:opus\n---\nDefault.\n",
        );
        write(
            repo.path(),
            ".lf/skills/default-parent.md",
            "---\ndefault_agent: codex\n---\nDefault.\n",
        );
        write(
            repo.path(),
            ".lf/skills/pinned-child.md",
            "---\nextends: default-parent\nagent: claude:opus\n---\nPinned.\n",
        );

        let catalog = SkillCatalog::load(Some(repo.path()), None, false).unwrap();

        let override_docs = catalog.load_skill("override-docs").unwrap();
        assert_eq!(
            override_docs.docs.as_deref(),
            Some(&["b.md".to_string()][..])
        );

        let merge_context = catalog.load_skill("merge-context").unwrap();
        assert_eq!(
            merge_context.docs.as_deref(),
            Some(&["a.md".to_string()][..])
        );
        assert_eq!(
            merge_context.settings["context"]["diff_files"].as_bool(),
            Some(false)
        );
        assert_eq!(
            merge_context.settings["context"]["budgets"]["memory_tokens"].as_u64(),
            Some(500)
        );
        assert_eq!(
            merge_context.settings["context"]["budgets"]["memory_bytes"].as_u64(),
            Some(2000)
        );
        assert_eq!(
            merge_context.settings["claude"]["model"].as_str(),
            Some("sonnet")
        );
        assert_eq!(
            merge_context.settings["codex"]["service_tier"].as_str(),
            Some("priority")
        );

        let default_child = catalog.load_skill("default-child").unwrap();
        assert!(default_child.agent.is_none());
        assert_eq!(
            default_child.settings["default_agent"].as_str(),
            Some("claude:opus")
        );

        let pinned_child = catalog.load_skill("pinned-child").unwrap();
        assert_eq!(pinned_child.agent.as_deref(), Some("claude:opus"));
        assert!(!pinned_child.settings.contains_key("default_agent"));
    }

    #[test]
    fn a_listing_tells_extensions_from_replacements_and_copies() {
        use crate::skills::listing::BuiltinRelation;
        let repo = TempDir::new().unwrap();
        write(
            repo.path(),
            ".lf/skills/gate.md",
            "---\nextends: lf:gate\n---\n## Here\n",
        );
        write(
            repo.path(),
            ".lf/skills/implement.md",
            "---\nextends: implement\n---\n## Repo\n",
        );
        write(
            repo.path(),
            ".lf/skills/polish.md",
            "---\nextends: builtin\n---\n## Compat\n",
        );
        write(
            repo.path(),
            ".lf/skills/card-implement.md",
            "---\nextends: implement\n---\n## Card\n",
        );
        write(
            repo.path(),
            ".lf/skills/base-qa.md",
            "---\nextends: lf:qa\n---\n## Base QA\n",
        );
        write(
            repo.path(),
            ".lf/skills/qa.md",
            "---\nextends: base-qa\n---\n## Repo QA\n",
        );
        write(
            repo.path(),
            ".lf/skills/verify.md",
            "---\nextends: implement\n---\nOur own verify.\n",
        );
        write(
            repo.path(),
            ".lf/skills/realign.md",
            "---\nextends: realign-cycle\n---\nCycle.\n",
        );
        write(
            repo.path(),
            ".lf/skills/realign-cycle.md",
            "---\nextends: realign\n---\nCycle.\n",
        );
        write(
            repo.path(),
            ".lf/skills/debug.md",
            loopflow_definitions::builtins::get_builtin_skill("debug").unwrap(),
        );
        write(repo.path(), ".lf/skills/mine.md", "Ours alone.\n");
        let catalog = SkillCatalog::load(Some(repo.path()), None, false).unwrap();
        let relations = |name: &str| catalog.relations(catalog.resolve(name).unwrap().unwrap());
        assert_eq!(
            relations("gate"),
            (Some(BuiltinRelation::Extends), Some("lf:gate".into()))
        );
        assert_eq!(
            relations("implement"),
            (Some(BuiltinRelation::Extends), Some("lf:implement".into()))
        );
        assert_eq!(
            relations("polish"),
            (Some(BuiltinRelation::Extends), Some("lf:polish".into()))
        );
        assert_eq!(
            relations("card-implement"),
            (None, Some("implement".into()))
        );
        assert_eq!(
            relations("qa"),
            (Some(BuiltinRelation::Extends), Some("base-qa".into()))
        );
        assert_eq!(
            relations("verify"),
            (Some(BuiltinRelation::Replaces), Some("implement".into()))
        );
        assert_eq!(
            relations("realign"),
            (
                Some(BuiltinRelation::Replaces),
                Some("realign-cycle".into())
            )
        );
        assert_eq!(relations("debug"), (Some(BuiltinRelation::Copies), None));
        assert_eq!(relations("mine"), (None, None));
        assert_eq!(relations("demo"), (None, None));
    }

    #[test]
    fn portable_literal_names_coexist_and_exact_names_win_across_scopes() {
        let repo = TempDir::new().unwrap();
        let home = TempDir::new().unwrap();
        write(home.path(), ".lf/skills/team/session.md", "Personal slash");
        write(home.path(), ".lf/skills/team-session.md", "Personal dash");
        write(repo.path(), ".lf/skills/team-session.md", "Repository dash");
        let catalog = SkillCatalog::load(Some(repo.path()), Some(home.path()), false).unwrap();
        assert_eq!(
            catalog
                .read_source(catalog.resolve("team/session").unwrap().unwrap())
                .unwrap(),
            "Personal slash"
        );
        assert_eq!(
            catalog
                .read_source(catalog.resolve("team-session").unwrap().unwrap())
                .unwrap(),
            "Repository dash"
        );
        write(
            repo.path(),
            ".lf/skills/team/session.md",
            "Repository slash",
        );
        let catalog = SkillCatalog::load(Some(repo.path()), Some(home.path()), false).unwrap();
        assert_eq!(
            catalog
                .read_source(catalog.resolve("team/session").unwrap().unwrap())
                .unwrap(),
            "Repository slash"
        );
        assert_eq!(
            catalog.load_skill("team/session").unwrap().name,
            "team/session"
        );
    }

    #[test]
    fn portable_fallback_is_unique_or_reports_candidates() {
        let repo = TempDir::new().unwrap();
        write(repo.path(), ".lf/skills/a/b-c.md", "First");
        let catalog = SkillCatalog::load(Some(repo.path()), None, false).unwrap();
        assert_eq!(catalog.resolve("a-b-c").unwrap().unwrap().name, "a/b-c");
        write(repo.path(), ".lf/skills/a-b/c.md", "Second");
        let catalog = SkillCatalog::load(Some(repo.path()), None, false).unwrap();
        let error = catalog.resolve("a-b-c").unwrap_err().to_string();
        assert!(error.contains("a/b-c") && error.contains("a-b/c"));
        assert_eq!(
            catalog
                .read_source(catalog.resolve("a/b-c").unwrap().unwrap())
                .unwrap(),
            "First"
        );
    }

    #[test]
    fn portable_names_preserve_native_declarations_and_do_not_invent_shortcuts() {
        let repo = TempDir::new().unwrap();
        write(
            repo.path(),
            ".claude/skills/team/KeepCase/SKILL.md",
            "---\nname: NativeName\nallowed-tools: Read\n---\nNative body",
        );
        let catalog = SkillCatalog::load(Some(repo.path()), None, false).unwrap();
        let skill = catalog.load_skill("team-KeepCase").unwrap();
        assert_eq!(skill.name, "team/KeepCase");
        assert!(skill
            .source
            .unwrap()
            .frontmatter
            .unwrap()
            .contains("NativeName"));
        assert!(catalog.resolve("KeepCase").unwrap().is_none());
        assert!(catalog.resolve("message").unwrap().is_none());
        assert_eq!(catalog.load_skill("operate").unwrap().name, "repo-operate");
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
            let selected = catalog.resolve("audit").unwrap().unwrap();
            assert_eq!(
                selected.path.as_deref(),
                Some(repo.path().join(path).as_path())
            );
            assert_eq!(catalog.read_source(selected).unwrap(), path);
            assert!(!catalog.entries().any(|entry| entry.name.contains("guide")));
            assert!(catalog
                .resolve("implement")
                .unwrap()
                .unwrap()
                .path
                .is_none());
            fs::remove_file(repo.path().join(path)).unwrap();
        }
        assert_eq!(
            SkillCatalog::load(Some(repo.path()), Some(home.path()), false)
                .unwrap()
                .resolve("audit")
                .unwrap()
                .unwrap()
                .path,
            Some(home.path().join(".lf/skills/audit.md"))
        );
    }

    #[test]
    fn agent_defaults_belong_only_to_loopflow_metadata() {
        let repo = TempDir::new().unwrap();
        let native = "---\nagent: Explore\ndefault_agent: native-persona\n---\nAudit.";
        write(repo.path(), ".claude/skills/native/SKILL.md", native);
        write(
            repo.path(),
            ".lf/skills/owned.md",
            "---\ndefault_agent: codex\n---\nAudit.",
        );
        let catalog = SkillCatalog::load(Some(repo.path()), None, false).unwrap();
        let skill = catalog.load_skill("native").unwrap();
        assert!(skill.agent.is_none());
        assert!(skill.settings.is_empty());
        assert_eq!(skill.source_text(), native);
        let skill = catalog.load_skill("owned").unwrap();
        assert!(skill.agent.is_none());
        assert_eq!(skill.settings["default_agent"].as_str(), Some("codex"));
    }

    #[test]
    fn unfamiliar_native_frontmatter_remains_plain_instructions() {
        let repo = TempDir::new().unwrap();
        let content = "---\nallowed-tools: [\n---\nRun this unfamiliar skill.";
        write(repo.path(), ".claude/skills/audit/SKILL.md", content);
        let catalog = SkillCatalog::load(Some(repo.path()), None, false).unwrap();
        let skill = catalog.load_skill("audit").unwrap();
        assert_eq!(skill.content.as_deref(), Some("Run this unfamiliar skill."));
        assert_eq!(skill.source_text(), content);
        write(repo.path(), ".lf/skills/audit.md", content);
        let catalog = SkillCatalog::load(Some(repo.path()), None, false).unwrap();
        assert!(matches!(
            catalog.load_skill("audit"),
            Err(crate::error::LoadError::InvalidSkill(_))
        ));
    }

    #[test]
    fn docs_frontmatter_names_one_target_or_a_list() {
        let repo = TempDir::new().unwrap();
        write(
            repo.path(),
            ".lf/skills/one.md",
            "---\ndocs: a.md\n---\nOne.",
        );
        write(
            repo.path(),
            ".lf/skills/two.md",
            "---\ndocs: [a.md, docs/]\n---\nTwo.",
        );
        write(
            repo.path(),
            ".lf/skills/bad.md",
            "---\ndocs: {a: b}\n---\nBad.",
        );
        let catalog = SkillCatalog::load(Some(repo.path()), None, false).unwrap();
        assert_eq!(catalog.load_skill("one").unwrap().docs.unwrap(), ["a.md"]);
        assert_eq!(
            catalog.load_skill("two").unwrap().docs.unwrap(),
            ["a.md", "docs/"]
        );
        assert!(matches!(
            catalog.load_skill("bad"),
            Err(crate::error::LoadError::InvalidSkill(_))
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
            catalog
                .read_source(catalog.resolve("audit").unwrap().unwrap())
                .unwrap(),
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
            catalog.resolve("team/audit").unwrap().unwrap().dialect,
            SkillDialect::Claude
        );
        assert!(!catalog
            .entries()
            .any(|entry| entry.name.contains("extra") || entry.name.contains("cycle")));
    }
}
