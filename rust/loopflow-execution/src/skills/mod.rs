pub mod catalog;
pub mod invocation;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::LoadError;
use crate::graph::instructions::render_graph_instructions;
use crate::graph::{compile_graph_with_catalog, DefinitionLoader};
use crate::harness::launcher;
use crate::skills::catalog::{is_generated, SkillCatalog, SkillDialect, SkillSource};
use loopflow_definitions::name::portable_name;
use loopflow_definitions::text::{skill_description, split_frontmatter};
use loopflow_harness::HarnessKind;

const SKILL_FILE_NAME: &str = "SKILL.md";
const LOOPFLOW_MARKER: &str = "loopflow: true";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillSyncOptions {
    pub prune: bool,
    pub global_home: Option<PathBuf>,
    pub repo: Option<PathBuf>,
}

impl Default for SkillSyncOptions {
    fn default() -> Self {
        Self {
            prune: true,
            global_home: None,
            repo: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillSyncReport {
    pub written: Vec<PathBuf>,
    pub pruned: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

#[derive(Debug, Clone, Default)]
struct Export {
    content: String,
    inputs: BTreeMap<String, String>,
}

/// What `lf skill customize` writes for a builtin: the builtin stays in force
/// and the repository's section is appended to it.
const EXTENSION: &str = "---\nextends: builtin\n---\n## In this repository\n";

/// Give the repository its own file for a builtin skill when it has none: an
/// extension of the builtin, or with `full` the builtin's whole text.
pub fn customize(name: &str, repo: &Path, full: bool) -> Result<PathBuf, LoadError> {
    let local = SkillCatalog::load(Some(repo), None, false)?;
    if let Some(path) = local.resolve(name)?.and_then(|source| source.path.clone()) {
        return Ok(path);
    }
    let name = loopflow_definitions::builtins::resolve_builtin_skill(name)
        .ok_or_else(|| LoadError::SkillNotFound(name.to_string()))?;
    let content = loopflow_definitions::builtins::get_builtin_skill(name)
        .ok_or_else(|| LoadError::SkillNotFound(name.to_string()))?;
    let path = repo.join(format!(".lf/skills/{name}.md"));
    fs::create_dir_all(path.parent().expect("definition has a parent"))?;
    fs::write(&path, if full { content } else { EXTENSION })?;
    Ok(path)
}

/// Where sync writes, relative to the home directory or, with `repository`,
/// to the repository root: each harness's declared directory, once.
pub fn installed_skill_directories(repository: bool) -> Vec<&'static str> {
    let mut directories = Vec::new();
    for kind in HarnessKind::ALL {
        let installed = launcher(kind).capabilities().installed_skills;
        let directory = if repository {
            installed.repository
        } else {
            installed.global
        };
        if !directories.contains(&directory) {
            directories.push(directory);
        }
    }
    directories
}

/// Config files global sync adds its directory to, relative to the home directory.
pub fn installed_skill_registries() -> Vec<&'static str> {
    HarnessKind::ALL
        .into_iter()
        .filter_map(|kind| {
            launcher(kind)
                .capabilities()
                .installed_skills
                .global_registry
        })
        .collect()
}

/// Export global or repository definitions without replacing third-party bundles.
pub fn sync_skills(options: &SkillSyncOptions) -> Result<SkillSyncReport, LoadError> {
    let home = options.global_home.clone().or_else(dirs::home_dir);
    let base = options
        .repo
        .as_ref()
        .or(home.as_ref())
        .ok_or_else(|| LoadError::InvalidSkill("home directory not found".into()))?;
    let catalog = SkillCatalog::load(
        options.repo.as_deref(),
        home.as_deref(),
        options.global_home.is_none(),
    )?;
    let graphs = match &options.repo {
        Some(repo) => crate::graph::repo_graph_names(repo)?,
        None => loopflow_definitions::builtins::builtin_graph_names()
            .into_iter()
            .map(str::to_string)
            .collect(),
    };
    let repo = options.repo.as_ref().map(std::path::absolute).transpose()?;
    let mut compiled_graphs = BTreeMap::new();
    let mut targets = Vec::new();
    let mut report = SkillSyncReport::default();
    // Resolve and validate the entire export set before touching any directory.
    // A skill renders the same everywhere; a Graph recipe names its input files
    // under the directory it lands in, so Graphs render per directory.
    let skills = project_names(catalog.entries().map(|skill| skill.name.as_str()));
    let mut skill_exports = BTreeMap::new();
    for (name, source) in &skills {
        let skill = catalog.exact(source).expect("projected source exists");
        if repo.as_ref().is_some_and(|repo| {
            !skill
                .path
                .as_ref()
                .is_some_and(|path| path.starts_with(repo))
        }) {
            continue;
        }
        skill_exports.insert(
            name.clone(),
            Export {
                content: render_skill(skill, name)?,
                ..Export::default()
            },
        );
    }
    for folder in installed_skill_directories(options.repo.is_some()) {
        let root = base.join(folder);
        let mut exports = skill_exports.clone();
        let mut sources = skills.clone();
        for (name, graph) in project_names(graphs.iter().map(String::as_str)) {
            if skills.contains_key(&name) {
                let path = root.join(&name).join(SKILL_FILE_NAME);
                tracing::debug!(%graph, path = %path.display(), "skill owns the Graph export name");
                report.skipped.push(path);
                continue;
            }
            let steps = match compiled_graphs.entry(graph.clone()) {
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::btree_map::Entry::Vacant(entry) => {
                    let definition = DefinitionLoader::new(options.repo.as_deref(), &catalog)
                        .load_graph(&graph)?;
                    let steps = compile_graph_with_catalog(&definition, &catalog)?;
                    entry.insert(steps)
                }
            };
            let mut inputs = BTreeMap::new();
            let directory = std::path::absolute(root.join(&name))?;
            let recipe = render_graph_instructions(&graph, steps, &mut |skill| {
                let invocation = invocation::SkillInvocation {
                    skill: crate::graph::Skill {
                        occurrence: skill.occurrence.clone(),
                        ..crate::graph::Skill::named(&skill.name)
                    },
                    arguments: String::new(),
                };
                let content = serde_json::to_string(&invocation)
                    .map_err(|error| LoadError::InvalidGraph(error.to_string()))?;
                let filename = input_filename(&content);
                inputs.insert(filename.clone(), content);
                Ok(directory.join(filename))
            })?;
            sources.insert(name.clone(), graph.clone());
            exports.insert(
                name.clone(),
                Export {
                    content: render_graph_skill(&graph, &name, &recipe, &inputs),
                    inputs,
                },
            );
        }
        let blocked = exports
            .iter()
            .filter(|(name, export)| !writable_target(base, &root, name, &export.inputs))
            .map(|(name, _)| name.clone())
            .collect::<BTreeSet<_>>();
        let blocked_sources = blocked.iter().map(|name| sources[name].clone()).collect();
        targets.push((root, exports, blocked, blocked_sources));
    }
    for (root, exports, blocked, _) in &targets {
        write_targets(exports, blocked, root, &mut report)
            .map_err(|error| in_directory(root, error))?;
    }
    // A failed replacement in any directory must not erase working old exports.
    if options.prune {
        for (root, exports, _, blocked_sources) in &targets {
            prune_targets(exports, blocked_sources, base, root, &mut report)
                .map_err(|error| in_directory(root, error))?;
        }
    }
    if options.repo.is_none() {
        for kind in HarnessKind::ALL {
            let installed = launcher(kind).capabilities().installed_skills;
            if let Some(registry) = installed.global_registry {
                let registry = base.join(registry);
                if register_directory(
                    &registry,
                    &std::path::absolute(base.join(installed.global))?,
                )
                .map_err(|error| in_directory(&registry, error))?
                {
                    report.written.push(registry);
                }
            }
        }
    }
    report.written.sort();
    report.pruned.sort();
    report.skipped.sort();
    Ok(report)
}

/// Add `directory` to a `skills.json`'s `entries`, keeping everything else in
/// it; true when the file changed.
fn register_directory(registry: &Path, directory: &Path) -> Result<bool, LoadError> {
    let directory = directory.to_string_lossy();
    let mut config = match fs::read_to_string(registry) {
        Ok(text) => serde_json::from_str::<serde_json::Value>(&text)
            .map_err(|error| LoadError::Io(format!("not JSON, left as is: {error}")))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => serde_json::json!({}),
        Err(error) => return Err(error.into()),
    };
    let entries = config
        .as_object_mut()
        .ok_or_else(|| LoadError::Io("not a JSON object, left as is".into()))?
        .entry("entries")
        .or_insert_with(|| serde_json::json!([]))
        .as_array_mut()
        .ok_or_else(|| LoadError::Io("`entries` is not a list, left as is".into()))?;
    if entries
        .iter()
        .any(|entry| entry["path"].as_str() == Some(directory.as_ref()))
    {
        return Ok(false);
    }
    entries.push(serde_json::json!({ "path": directory }));
    fs::create_dir_all(registry.parent().expect("registry has a parent"))?;
    let text = serde_json::to_string_pretty(&config).expect("JSON value serializes");
    fs::write(registry, format!("{text}\n"))?;
    Ok(true)
}

/// Sync writes several tools' directories; a failure says which one.
fn in_directory(root: &Path, error: LoadError) -> LoadError {
    match error {
        LoadError::Io(message) => LoadError::Io(format!("{}: {message}", root.display())),
        other => other,
    }
}

/// One portable export per source; exact names keep priority over flattened names.
fn project_names<'a>(names: impl Iterator<Item = &'a str>) -> BTreeMap<String, String> {
    let mut candidates = BTreeMap::<String, BTreeSet<String>>::new();
    for name in names {
        candidates
            .entry(export_name(name))
            .or_default()
            .insert(name.to_string());
    }
    let mut occupied = candidates.keys().cloned().collect::<BTreeSet<_>>();
    let mut selected = BTreeMap::new();
    for (target, sources) in candidates {
        let winner = if sources.contains(&target) {
            Some(target.as_str())
        } else if sources.len() == 1 {
            sources.first().map(String::as_str)
        } else {
            None
        };
        for source in &sources {
            let name = if Some(source.as_str()) == winner {
                target.clone()
            } else {
                let hash = format!("{:x}", Sha256::digest(source.as_bytes()));
                let mut name = suffixed_name(&target, &hash[..8]);
                let mut index = 2;
                while !occupied.insert(name.clone()) {
                    name = suffixed_name(&target, &format!("{}-{index}", &hash[..8]));
                    index += 1;
                }
                name
            };
            selected.insert(name, source.clone());
        }
    }
    selected
}

fn export_name(name: &str) -> String {
    let slug = |value: &str| {
        value
            .split(|ch: char| !ch.is_ascii_alphanumeric())
            .filter(|part| !part.is_empty())
            .map(str::to_ascii_lowercase)
            .collect::<Vec<_>>()
            .join("-")
    };
    let portable = slug(name);
    if !portable.is_empty() && portable.len() <= 64 {
        return portable;
    }
    let leaf = slug(name.rsplit('/').next().unwrap_or(name));
    let hash = format!("{:x}", Sha256::digest(name.as_bytes()));
    suffixed_name(if leaf.is_empty() { "skill" } else { &leaf }, &hash[..8])
}

fn suffixed_name(name: &str, suffix: &str) -> String {
    let prefix = &name[..name.len().min(63 - suffix.len())];
    format!("{}-{suffix}", prefix.trim_end_matches('-'))
}

fn writable_target(
    base: &Path,
    root: &Path,
    name: &str,
    inputs: &BTreeMap<String, String>,
) -> bool {
    let directory = root.join(name);
    let path = directory.join(SKILL_FILE_NAME);
    // The selected home/repository may use filesystem aliases (e.g. macOS /var).
    // Protect provider destinations beneath it, not the path used to reach it.
    if directory
        .ancestors()
        .take_while(|dir| *dir != base)
        .any(|dir| dir.is_symlink() || (dir.exists() && !dir.is_dir()))
        || directory
            .parent()
            .into_iter()
            .flat_map(Path::ancestors)
            .take_while(|ancestor| *ancestor != root)
            .any(|ancestor| ancestor.join(SKILL_FILE_NAME).exists())
        || path.is_symlink()
    {
        return false;
    }
    if inputs.iter().any(|(name, content)| {
        let input = directory.join(name);
        input.is_symlink()
            || (input.exists() && fs::read_to_string(input).ok().as_ref() != Some(content))
    }) {
        return false;
    }
    if path.exists() {
        return path.is_file() && is_generated(&path);
    }
    // A new bundle must not claim foreign assets. An interrupted export may
    // have published only its content-addressed captures; finish that write.
    !directory.is_dir()
        || fs::read_dir(&directory).is_ok_and(|mut entries| {
            entries.all(|entry| {
                entry.is_ok_and(|entry| {
                    inputs.contains_key(entry.file_name().to_string_lossy().as_ref())
                })
            })
        })
}

fn render_graph_skill(
    graph: &str,
    name: &str,
    recipe: &str,
    inputs: &BTreeMap<String, String>,
) -> String {
    format!("---\nname: {name}\ndescription: Follow the {name} Graph in this conversation.\nloopflow: true\nloopflow-kind: graph\nloopflow-graph: {}\nloopflow-inputs: {}\ndisable-model-invocation: true\n---\n{recipe}", yaml_string(graph), serde_json::to_string(&inputs.keys().collect::<Vec<_>>()).expect("input names serialize"))
}

fn write_targets(
    exports: &BTreeMap<String, Export>,
    blocked: &BTreeSet<String>,
    target_root: &Path,
    report: &mut SkillSyncReport,
) -> Result<(), LoadError> {
    for (name, export) in exports {
        let content = &export.content;
        let path = target_root.join(name).join(SKILL_FILE_NAME);
        if blocked.contains(name) {
            tracing::debug!(path = %path.display(), "skill export destination is occupied");
            report.skipped.push(path);
            continue;
        }
        let previous = owned_inputs(&path);
        let directory = path.parent().expect("skill destination has a parent");
        fs::create_dir_all(directory)?;
        // Publish captures first: a recipe must never reference an absent input.
        for (name, input) in &export.inputs {
            let target = directory.join(name);
            if !target.exists() {
                write_export(&target, input, target_root)?;
            }
        }
        if fs::read_to_string(&path).ok().as_deref() != Some(content.as_str()) {
            write_export(&path, content, target_root)?;
            report.written.push(path.clone());
        }
        for old in previous {
            if !export.inputs.contains_key(&old) {
                remove_owned_input(directory, &old)?;
            }
        }
    }
    Ok(())
}

fn write_export(path: &Path, content: &str, root: &Path) -> Result<(), LoadError> {
    let temporary = tempfile::NamedTempFile::new_in(root)?;
    fs::write(temporary.path(), content)?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}

fn prune_targets(
    exports: &BTreeMap<String, Export>,
    blocked_sources: &BTreeSet<String>,
    base: &Path,
    target_root: &Path,
    report: &mut SkillSyncReport,
) -> Result<(), LoadError> {
    if target_root
        .ancestors()
        .take_while(|dir| *dir != base)
        .any(Path::is_symlink)
    {
        return Ok(());
    }
    for path in generated_skill_files(target_root)? {
        let Some(name) = synced_skill_name_from_path(target_root, &path) else {
            continue;
        };
        if exports.contains_key(&name)
            || blocked_sources
                .iter()
                .any(|source| portable_name(source) == portable_name(&name))
        {
            continue;
        }
        for input in owned_inputs(&path) {
            remove_owned_input(path.parent().expect("skill has a directory"), &input)?;
        }
        fs::remove_file(&path)?;
        report.pruned.push(path.clone());
        prune_empty_skill_dir(&path, target_root)?;
    }
    Ok(())
}

// Content-addressed captures belong to the generated recipe's manifest. Never
// overwrite or prune a foreign file, including a replaced capture or symlink.
fn input_filename(content: &str) -> String {
    format!(
        "loopflow-input-{:x}.json",
        Sha256::digest(content.as_bytes())
    )
}

fn owned_inputs(path: &Path) -> Vec<String> {
    let Some(content) = fs::read_to_string(path).ok() else {
        return Vec::new();
    };
    let Some((header, _)) = split_frontmatter(&content) else {
        return Vec::new();
    };
    let Ok(fields) = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(header) else {
        return Vec::new();
    };
    fields
        .get("loopflow-inputs")
        .and_then(serde_yaml_ng::Value::as_sequence)
        .into_iter()
        .flatten()
        .filter_map(serde_yaml_ng::Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn remove_owned_input(directory: &Path, name: &str) -> Result<(), LoadError> {
    if name.contains('/') || name.contains('\\') {
        return Ok(());
    }
    let path = directory.join(name);
    if !path.is_symlink()
        && fs::read_to_string(&path).is_ok_and(|content| input_filename(&content) == name)
    {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn render_skill(skill: &SkillSource, name: &str) -> Result<String, LoadError> {
    let content = skill.read()?;
    let (original_frontmatter, body) = split_frontmatter(&content).unwrap_or(("", &content));
    let description = skill_description(&content)
        .unwrap_or_else(|| format!("Run the loopflow {} skill.", skill.name));
    let source = skill.path.as_ref().map_or_else(String::new, |path| {
        format!(
            "Base directory for this skill: {}\n\n",
            path.parent().expect("skill source has a parent").display()
        )
    });
    let mut declarations = String::new();
    let mut frontmatter = Vec::new();
    frontmatter.push(format!("name: {}", yaml_string(name)));
    frontmatter.push(format!("description: {}", yaml_string(&description)));
    frontmatter.push(LOOPFLOW_MARKER.to_string());
    frontmatter.push("loopflow-kind: skill".into());
    frontmatter.push(format!("loopflow-skill: {}", yaml_string(&skill.name)));

    if skill.dialect != SkillDialect::Loopflow {
        // Preserve authored declarations; the destination provider decides which it supports.
        if let Ok(serde_yaml_ng::Value::Mapping(mut metadata)) =
            serde_yaml_ng::from_str(original_frontmatter)
        {
            for field in [
                "name",
                "description",
                "loopflow",
                "loopflow-skill",
                "loopflow-kind",
            ] {
                metadata.remove(serde_yaml_ng::Value::String(field.into()));
            }
            if !metadata.is_empty() {
                frontmatter.push(
                    serde_yaml_ng::to_string(&metadata)
                        .map_err(|error| LoadError::InvalidSkill(error.to_string()))?,
                );
            }
        } else if !original_frontmatter.is_empty() {
            declarations = format!("Original native declarations (not translated):\n```yaml\n{original_frontmatter}\n```\n\n");
        }
    }
    Ok(format!(
        "---\n{}\n---\n{source}{declarations}{body}",
        frontmatter.join("\n")
    ))
}

fn yaml_string(value: &str) -> String {
    serde_yaml_ng::to_string(value)
        .unwrap_or_else(|_| format!("\"{}\"", value.replace('"', "\\\"")))
        .trim()
        .trim_start_matches("---\n")
        .trim_end_matches("\n...")
        .to_string()
}

fn generated_skill_files(root: &Path) -> Result<Vec<PathBuf>, LoadError> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    collect_skill_files(root, &mut files)?;
    files.retain(|path| is_generated(path));
    Ok(files)
}

fn collect_skill_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), LoadError> {
    let skill = dir.join(SKILL_FILE_NAME);
    if skill.is_symlink() {
        return Ok(());
    }
    if skill.is_file() {
        // A bundle owns everything below it, including example skills.
        files.push(skill);
    } else {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() && !path.is_symlink() {
                collect_skill_files(&path, files)?;
            }
        }
    }
    Ok(())
}

fn synced_skill_name_from_path(root: &Path, skill_file: &Path) -> Option<String> {
    let dir = skill_file.parent()?;
    Some(
        dir.strip_prefix(root)
            .ok()?
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
    )
}

fn prune_empty_skill_dir(skill_file: &Path, target_root: &Path) -> Result<(), LoadError> {
    let mut current = match skill_file.parent() {
        Some(parent) => parent.to_path_buf(),
        None => return Ok(()),
    };

    while current != target_root {
        match fs::remove_dir(&current) {
            Ok(()) => {
                let Some(parent) = current.parent() else {
                    break;
                };
                current = parent.to_path_buf();
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => break,
            Err(err) if err.kind() == std::io::ErrorKind::DirectoryNotEmpty => break,
            Err(err) => return Err(err.into()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{customize, sync_skills, SkillSyncOptions, SKILL_FILE_NAME};
    use loopflow_definitions::builtins;
    use loopflow_definitions::text::split_frontmatter;
    use std::fs;
    use tempfile::TempDir;

    fn options_for(home: &TempDir) -> SkillSyncOptions {
        SkillSyncOptions {
            prune: true,
            global_home: Some(home.path().to_path_buf()),
            repo: None,
        }
    }

    #[test]
    fn global_sync_registers_agents_skills_in_antigravity_config_once() {
        let home = TempDir::new().unwrap();
        let registry = home.path().join(".gemini/config/skills.json");
        fs::create_dir_all(registry.parent().unwrap()).unwrap();
        fs::write(
            &registry,
            r#"{"inherits":[{"path":"/team/skills.json"}],"entries":[{"path":"/mine"}]}"#,
        )
        .unwrap();

        sync_skills(&options_for(&home)).unwrap();
        let second = sync_skills(&options_for(&home)).unwrap();

        let config: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&registry).unwrap()).unwrap();
        let agents = std::path::absolute(home.path().join(".agents/skills")).unwrap();
        assert_eq!(config["inherits"][0]["path"], "/team/skills.json");
        assert_eq!(
            config["entries"],
            serde_json::json!([{"path": "/mine"}, {"path": agents.to_string_lossy()}])
        );
        assert!(!second.written.contains(&registry));
        assert!(!home.path().join(".gemini/config/skills").exists());
    }

    #[test]
    fn global_sync_leaves_an_unreadable_antigravity_config_alone() {
        let home = TempDir::new().unwrap();
        let registry = home.path().join(".gemini/config/skills.json");
        fs::create_dir_all(registry.parent().unwrap()).unwrap();
        fs::write(&registry, "{ not json").unwrap();

        let error = sync_skills(&options_for(&home)).unwrap_err().to_string();

        assert!(error.contains("skills.json"), "{error}");
        assert_eq!(fs::read_to_string(&registry).unwrap(), "{ not json");
    }

    #[test]
    fn customize_extends_a_builtin_once_then_returns_the_repository_file() {
        let repo = TempDir::new().unwrap();
        let loaded = |name: &str| {
            super::SkillCatalog::load(Some(repo.path()), None, false)
                .unwrap()
                .load_skill(name)
                .unwrap()
        };
        let builtin = loaded("implement");
        let path = customize("implement", repo.path(), false).unwrap();
        assert_eq!(path, repo.path().join(".lf/skills/implement.md"));
        let extended = loaded("implement");
        assert_eq!(
            extended.content.unwrap(),
            format!(
                "{}\n\n## In this repository\n",
                builtin.content.unwrap().trim_end()
            )
        );
        // The builtin's frontmatter still applies to the extension.
        assert_eq!(extended.action_style.as_deref(), Some("procedural"));
        assert_eq!(extended.action_style, builtin.action_style);
        fs::write(&path, "Repository copy.").unwrap();
        assert_eq!(customize("implement", repo.path(), true).unwrap(), path);
        assert_eq!(fs::read_to_string(&path).unwrap(), "Repository copy.");

        let full = customize("pr-message", repo.path(), true).unwrap();
        assert_eq!(
            fs::read_to_string(full).unwrap(),
            builtins::get_builtin_skill("pr-message").unwrap()
        );
    }

    #[cfg(unix)]
    #[test]
    fn sync_skills_accepts_home_aliases_but_preserves_provider_symlinks() {
        let fixture = TempDir::new().unwrap();
        let home = fixture.path().join("home");
        let alias = fixture.path().join("alias");
        let vendor = fixture.path().join("vendor");
        let stale = "skills/obsolete/SKILL.md";
        for root in [home.join(".agents"), vendor.clone()] {
            let path = root.join(stale);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "---\nloopflow: true\n---\nOld export").unwrap();
        }
        std::os::unix::fs::symlink(&home, &alias).unwrap();
        std::os::unix::fs::symlink(&vendor, home.join(".claude")).unwrap();
        let options = SkillSyncOptions {
            global_home: Some(alias.clone()),
            ..SkillSyncOptions::default()
        };
        let report = sync_skills(&options).unwrap();
        assert!(alias.join(".agents/skills/implement/SKILL.md").is_file());
        assert!(report.pruned.contains(&alias.join(".agents").join(stale)));
        assert!(report
            .skipped
            .contains(&alias.join(".claude/skills/implement/SKILL.md")));
        assert_eq!(
            fs::read_to_string(vendor.join(stale)).unwrap(),
            "---\nloopflow: true\n---\nOld export"
        );
        assert!(!vendor.join("skills/implement").exists());
        let again = sync_skills(&options).unwrap();
        assert!(again.written.is_empty() && again.pruned.is_empty());
    }

    #[test]
    fn portable_exports_do_not_turn_third_party_namespaces_into_bundles() {
        let home = TempDir::new().unwrap();
        fs::create_dir_all(home.path().join(".lf/skills")).unwrap();
        fs::write(home.path().join(".lf/skills/team.md"), "Team skill").unwrap();
        let native = home.path().join(".agents/skills/team/vendor/SKILL.md");
        fs::create_dir_all(native.parent().unwrap()).unwrap();
        fs::write(&native, "Third-party nested skill").unwrap();
        let report = sync_skills(&options_for(&home)).unwrap();
        let destination = home.path().join(".agents/skills/team/SKILL.md");
        assert!(report.skipped.contains(&destination));
        assert!(!destination.exists());
        assert_eq!(
            fs::read_to_string(native).unwrap(),
            "Third-party nested skill"
        );
    }

    #[test]
    fn portable_graph_export_yields_to_third_party_skills_in_any_scope() {
        let home = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        let native = home.path().join(".claude/skills/team/ship/SKILL.md");
        fs::create_dir_all(native.parent().unwrap()).unwrap();
        fs::write(&native, "Third-party owns team-ship").unwrap();
        fs::create_dir_all(repo.path().join(".lf/graphs")).unwrap();
        fs::write(
            repo.path().join(".lf/graphs/team-ship.yaml"),
            "- implement\n",
        )
        .unwrap();
        let options = SkillSyncOptions {
            repo: Some(repo.path().to_path_buf()),
            ..options_for(&home)
        };
        let report = sync_skills(&options).unwrap();
        for provider in [".claude", ".agents"] {
            let destination = repo.path().join(provider).join("skills/team-ship/SKILL.md");
            assert!(report.skipped.contains(&destination));
            assert!(!destination.exists());
        }
        assert_eq!(
            fs::read_to_string(native).unwrap(),
            "Third-party owns team-ship"
        );
    }

    #[test]
    fn portable_exports_replace_nested_paths_only_after_unblocked_replacement() {
        let home = TempDir::new().unwrap();
        let old = home.path().join(".agents/skills/wave/session/SKILL.md");
        fs::create_dir_all(old.parent().unwrap()).unwrap();
        fs::write(&old, "---\nloopflow: true\n---\nPrevious generated body").unwrap();
        let replacement = home.path().join(".agents/skills/wave-session/SKILL.md");
        fs::create_dir_all(replacement.parent().unwrap()).unwrap();
        fs::write(&replacement, "Third-party body").unwrap();
        let report = sync_skills(&options_for(&home)).unwrap();
        assert!(report.skipped.contains(&replacement));
        assert!(old.exists());
        fs::remove_file(&replacement).unwrap();
        let report = sync_skills(&options_for(&home)).unwrap();
        assert!(report.written.contains(&replacement));
        assert!(report.pruned.contains(&old));
        assert!(!old.exists());
    }

    #[test]
    fn portable_graph_exports_are_scoped_and_idempotent() {
        let home = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        fs::create_dir_all(repo.path().join(".lf/graphs/team")).unwrap();
        fs::write(
            repo.path().join(".lf/graphs/team/ship.yaml"),
            "- implement\n",
        )
        .unwrap();
        fs::create_dir_all(repo.path().join(".lf/skills/team")).unwrap();
        fs::write(repo.path().join(".lf/skills/team/check.md"), "Repo check").unwrap();
        sync_skills(&options_for(&home)).unwrap();
        assert!(home.path().join(".agents/skills/pursue/SKILL.md").exists());
        assert!(!home
            .path()
            .join(".agents/skills/team-ship/SKILL.md")
            .exists());
        let options = SkillSyncOptions {
            repo: Some(repo.path().to_path_buf()),
            ..options_for(&home)
        };
        sync_skills(&options).unwrap();
        for provider in [".agents", ".claude"] {
            let root = repo.path().join(provider).join("skills");
            let wrapper = fs::read_to_string(root.join("team-ship/SKILL.md")).unwrap();
            assert!(wrapper.contains("# Graph: team/ship"));
            assert!(wrapper.contains(" skill -- implement`."));
            assert!(wrapper.contains("loopflow-kind: graph"));
            assert!(!wrapper.contains("--instructions"));
            assert!(root.join("team-check/SKILL.md").exists());
            assert!(!root.join("pursue/SKILL.md").exists());
        }
        assert!(sync_skills(&options).unwrap().written.is_empty());
        fs::remove_file(repo.path().join(".lf/graphs/team/ship.yaml")).unwrap();
        assert_eq!(sync_skills(&options).unwrap().pruned.len(), 2);
    }

    #[test]
    fn global_recipe_selects_repository_skill_and_keeps_authored_occurrence() {
        let home = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        sync_skills(&options_for(&home)).unwrap();
        let recipe = home.path().join(".agents/skills/code/SKILL.md");
        let inputs = super::owned_inputs(&recipe);
        let path = inputs
            .iter()
            .map(|name| recipe.parent().unwrap().join(name))
            .find(|path| {
                let input: crate::skills::invocation::SkillInvocation =
                    serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
                input.skill.name == "implement"
            })
            .unwrap();
        fs::create_dir_all(repo.path().join(".lf/skills")).unwrap();
        let source = repo.path().join(".lf/skills/implement.md");
        fs::write(
            &source,
            "---\nagent: claude:repo\n---\nRepository implementation.",
        )
        .unwrap();
        let selected = crate::skills::invocation::SkillInvocation::read(
            &path,
            repo.path(),
            Some("Exact request"),
        )
        .unwrap();
        assert_eq!(
            selected.skill.content.as_deref(),
            Some("Repository implementation.")
        );
        assert_eq!(selected.skill.agent.as_deref(), Some("claude:repo"));
        assert_eq!(selected.skill.occurrence.as_ref().unwrap().graph, "build");
        assert_eq!(selected.arguments, "Exact request");
        // A runtime Graph's already resolved capture retains its body/arguments.
        let captured = home.path().join("runtime.json");
        fs::write(&captured, serde_json::to_vec(&selected).unwrap()).unwrap();
        fs::write(source, "Changed after runtime capture.").unwrap();
        let runtime = crate::skills::invocation::SkillInvocation::read(
            &captured,
            repo.path(),
            Some("Correction direction"),
        )
        .unwrap();
        assert_eq!(runtime, selected);
    }

    #[test]
    fn graph_captures_refresh_and_prune_with_their_recipe_preserving_foreign_assets() {
        let home = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        fs::create_dir_all(repo.path().join(".lf/graphs")).unwrap();
        let graph = repo.path().join(".lf/graphs/proof.yaml");
        fs::write(
            &graph,
            "- step: {name: implement, id: work, agent: claude}\n",
        )
        .unwrap();
        let options = SkillSyncOptions {
            repo: Some(repo.path().to_owned()),
            ..options_for(&home)
        };
        sync_skills(&options).unwrap();
        let directory = repo.path().join(".agents/skills/proof");
        let recipe = directory.join(SKILL_FILE_NAME);
        let original = super::owned_inputs(&recipe).pop().unwrap();
        // A crash between publishing inputs and the recipe is recoverable.
        let before = fs::read_to_string(&recipe).unwrap();
        fs::remove_file(&recipe).unwrap();
        sync_skills(&options).unwrap();
        assert_eq!(fs::read_to_string(&recipe).unwrap(), before);
        let input = crate::skills::invocation::SkillInvocation::read(
            &directory.join(&original),
            repo.path(),
            None,
        )
        .unwrap();
        let occurrence = input.skill.occurrence.unwrap();
        assert_eq!(occurrence.graph, "proof");
        assert_eq!(occurrence.id.as_deref(), Some("work"));
        assert_eq!(occurrence.values["agent"].as_str(), Some("claude"));
        fs::write(directory.join("notes.txt"), "User asset").unwrap();
        fs::write(
            &graph,
            "- step: {name: implement, id: work, agent: codex}\n",
        )
        .unwrap();
        sync_skills(&options).unwrap();
        assert!(!directory.join(original).exists());
        let current = super::owned_inputs(&recipe).pop().unwrap();
        assert!(directory.join(&current).exists());
        // A missing capture is repaired even when the checklist itself is unchanged.
        fs::remove_file(directory.join(&current)).unwrap();
        sync_skills(&options).unwrap();
        assert!(directory.join(&current).exists());
        // A replaced capture is no longer owned; neither refresh nor pruning touches it.
        fs::write(directory.join(&current), "User replacement").unwrap();
        assert!(sync_skills(&options).unwrap().skipped.contains(&recipe));
        fs::remove_file(graph).unwrap();
        sync_skills(&options).unwrap();
        assert!(!recipe.exists());
        assert_eq!(
            fs::read_to_string(directory.join(current)).unwrap(),
            "User replacement"
        );
        assert_eq!(
            fs::read_to_string(directory.join("notes.txt")).unwrap(),
            "User asset"
        );
        assert!(!repo.path().join(".claude/skills/proof").exists());
    }

    #[test]
    fn chat_recipes_refresh_repository_overrides_and_keep_global_scope() {
        let home = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        fs::create_dir_all(home.path().join(".lf/skills")).unwrap();
        fs::write(
            home.path().join(".lf/skills/realign.md"),
            "Personal realign method",
        )
        .unwrap();
        sync_skills(&options_for(&home)).unwrap();
        let global = home.path().join(".claude/skills/pursue/SKILL.md");
        let original = fs::read_to_string(&global).unwrap();
        assert!(original.contains("Execute the skill `realign` in this conversation."));
        assert!(original.contains(
            &home
                .path()
                .join(".lf/skills/realign.md")
                .display()
                .to_string()
        ));
        assert!(!original.contains("Personal realign method"));

        fs::create_dir_all(repo.path().join(".lf/graphs")).unwrap();
        let graph = repo.path().join(".lf/graphs/pursue.yaml");
        fs::write(&graph, "- interactive: {name: demo, id: review}\n").unwrap();
        let options = SkillSyncOptions {
            repo: Some(repo.path().to_path_buf()),
            ..options_for(&home)
        };
        sync_skills(&options).unwrap();
        let local = repo.path().join(".claude/skills/pursue/SKILL.md");
        assert!(fs::read_to_string(&local)
            .unwrap()
            .contains("1. Execute the skill `demo` in this conversation."));
        fs::write(&graph, "- compress\n").unwrap();
        sync_skills(&options).unwrap();
        let updated = fs::read_to_string(local).unwrap();
        assert!(updated.contains(" skill -- compress`."));
        assert!(!updated.contains("Execute the skill `demo`"));
        assert_eq!(fs::read_to_string(global).unwrap(), original);
    }

    #[test]
    fn chat_recipes_preserve_native_user_only_controls() {
        let home = TempDir::new().unwrap();
        let source = home.path().join(".claude/skills/restricted/SKILL.md");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        let body = "---\nname: restricted\ndisable-model-invocation: true\n---\nOnly on request.\n";
        fs::write(&source, body).unwrap();
        sync_skills(&options_for(&home)).unwrap();
        assert_eq!(fs::read_to_string(source).unwrap(), body);
        let exported =
            fs::read_to_string(home.path().join(".agents/skills/restricted/SKILL.md")).unwrap();
        assert!(exported.contains("disable-model-invocation: true"));
    }

    #[test]
    fn portable_exports_preserve_colliding_skills_and_graphs() {
        let home = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        for (kind, extension) in [("skills", "md"), ("graphs", "yaml")] {
            for (name, body) in [("team/check", "Slash"), ("team-check", "Dash")] {
                let name = if kind == "graphs" {
                    name.replace("check", "ship")
                } else {
                    name.to_string()
                };
                let path = repo.path().join(format!(".lf/{kind}/{name}.{extension}"));
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                let content = if kind == "graphs" {
                    format!(
                        "- {}\n",
                        if name.contains('/') {
                            "team/check"
                        } else {
                            "team-check"
                        }
                    )
                } else {
                    body.to_string()
                };
                fs::write(path, content).unwrap();
            }
        }
        let options = SkillSyncOptions {
            repo: Some(repo.path().to_path_buf()),
            ..options_for(&home)
        };
        let report = sync_skills(&options).unwrap();
        assert!(report.skipped.is_empty());
        for provider in [".agents", ".claude"] {
            let root = repo.path().join(provider).join("skills");
            let files = super::generated_skill_files(&root).unwrap();
            assert_eq!(files.len(), 4);
            for (source, body) in [
                ("team/check", "Slash"),
                ("team-check", "Dash"),
                ("team/ship", "skill -- team/check"),
                ("team-ship", "skill -- team-check"),
            ] {
                let contents = files
                    .iter()
                    .map(|path| fs::read_to_string(path).unwrap())
                    .find(|content| {
                        content.contains(&format!(
                            "loopflow-{}: {source}\n",
                            if source.contains("ship") {
                                "graph"
                            } else {
                                "skill"
                            }
                        ))
                    })
                    .unwrap();
                assert!(contents.contains(body));
                let metadata: serde_yaml_ng::Value =
                    serde_yaml_ng::from_str(split_frontmatter(&contents).unwrap().0).unwrap();
                let name = metadata["name"].as_str().unwrap();
                assert!(!name.contains('/'));
                assert!(root.join(name).join(SKILL_FILE_NAME).is_file());
                if !source.contains('/') {
                    assert_eq!(name, source);
                }
            }
        }
        let again = sync_skills(&options).unwrap();
        assert!(again.written.is_empty() && again.pruned.is_empty());
    }

    #[test]
    fn portable_graph_collisions_yield_to_skills_without_prefixes() {
        let home = TempDir::new().unwrap();
        fs::create_dir_all(home.path().join(".lf/skills")).unwrap();
        fs::write(
            home.path().join(".lf/skills/pursue.md"),
            "Skill owns this name",
        )
        .unwrap();
        let report = sync_skills(&options_for(&home)).unwrap();
        for provider in [".claude", ".agents"] {
            let root = home.path().join(provider).join("skills");
            let path = root.join("pursue/SKILL.md");
            assert!(report.skipped.contains(&path));
            let body = fs::read_to_string(path).unwrap();
            assert!(body.contains("Skill owns this name"));
            assert!(!body.contains("loopflow-kind: graph"));
            assert!(!root.join("graph-pursue").exists());
        }
    }

    #[test]
    fn portable_exports_shorten_synced_names_and_preserve_resources() {
        let home = TempDir::new().unwrap();
        let names = [
            "synced/0c06be3a-1d03-4ec8-ac23-9f803fe9cc63_41c12349-be37-44e0-ae4b-793d9715af6f/pdf",
            "synced/451254b4-1dec-42d0-94cb-93056f841a15_7e00561f-1060-4bc9-9606-8d1fb3afb92c/pdf",
            "UPPER_--Case",
        ];
        for name in names {
            let source = home
                .path()
                .join(".codex/skills")
                .join(name)
                .join(SKILL_FILE_NAME);
            fs::create_dir_all(source.parent().unwrap()).unwrap();
            fs::write(&source, "---\nname: original\ndescription: Read a document\nallowed-tools: Read\n---\nRead rules.md.\n").unwrap();
            fs::write(source.parent().unwrap().join("rules.md"), "Original rules").unwrap();
            let old = home
                .path()
                .join(".agents/skills")
                .join(name)
                .join(SKILL_FILE_NAME);
            fs::create_dir_all(old.parent().unwrap()).unwrap();
            fs::write(old, "---\nloopflow: true\n---\nOld export").unwrap();
        }
        let report = sync_skills(&options_for(&home)).unwrap();
        assert_eq!(report.pruned.len(), 3);
        let files = super::generated_skill_files(&home.path().join(".agents/skills")).unwrap();
        for source in names {
            let content = files
                .iter()
                .map(|path| fs::read_to_string(path).unwrap())
                .find(|content| content.contains(&format!("loopflow-skill: {source}\n")))
                .unwrap();
            let metadata: serde_yaml_ng::Value =
                serde_yaml_ng::from_str(split_frontmatter(&content).unwrap().0).unwrap();
            let name = metadata["name"].as_str().unwrap();
            assert!(name.len() <= 64 && !name.is_empty());
            assert!(name
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'));
            assert!(!name.starts_with('-') && !name.ends_with('-') && !name.contains("--"));
            if source.starts_with("synced/") {
                assert!(name.starts_with("pdf-") && name.len() < 20);
            }
            assert!(content.contains("allowed-tools: Read"));
            assert!(content.contains(
                &home
                    .path()
                    .join(".codex/skills")
                    .join(source)
                    .display()
                    .to_string()
            ));
            assert!(content.contains("Read rules.md."));
            let claude = fs::read_to_string(
                home.path()
                    .join(".claude/skills")
                    .join(name)
                    .join(SKILL_FILE_NAME),
            )
            .unwrap();
            assert_eq!(content, claude);
        }
        let again = sync_skills(&options_for(&home)).unwrap();
        assert!(again.written.is_empty() && again.pruned.is_empty());
    }

    #[test]
    fn sync_skills_preserves_third_party_collisions_and_original_bundles() {
        let home = TempDir::new().unwrap();
        let original = home.path().join(".claude/skills/audit/SKILL.md");
        fs::create_dir_all(original.parent().unwrap()).unwrap();
        let content = "---\nname: audit\ndescription: Audit code\nallowed-tools: Read\n---\nRead [rules](rules.md).\n";
        fs::write(&original, content).unwrap();
        fs::write(original.parent().unwrap().join("rules.md"), "rules").unwrap();
        let collision = home.path().join(".agents/skills/implement/SKILL.md");
        fs::create_dir_all(collision.parent().unwrap()).unwrap();
        fs::write(&collision, "Third-party implement").unwrap();
        let example = collision.parent().unwrap().join("examples/old/SKILL.md");
        fs::create_dir_all(example.parent().unwrap()).unwrap();
        fs::write(&example, "---\nloopflow: true\n---\nBundled example").unwrap();
        let report = sync_skills(&options_for(&home)).unwrap();
        assert!(
            example.exists(),
            "pruning must not enter third-party bundles"
        );
        assert_eq!(fs::read_to_string(&original).unwrap(), content);
        assert_eq!(
            fs::read_to_string(&collision).unwrap(),
            "Third-party implement"
        );
        assert!(!report.written.contains(&original));
        assert!(!report.written.contains(&collision));
        let export = fs::read_to_string(home.path().join(".agents/skills/audit/SKILL.md")).unwrap();
        assert!(export.contains(&original.parent().unwrap().display().to_string()));
        assert!(export.contains("allowed-tools: Read"));
        assert!(!home
            .path()
            .join(".agents/skills/audit/rules/SKILL.md")
            .exists());
        assert!(sync_skills(&options_for(&home)).unwrap().written.is_empty());
    }

    #[test]
    fn sync_skills_writes_home_targets_with_marker() {
        let home = TempDir::new().unwrap();
        let skills = home.path().join(".lf/skills");
        fs::create_dir_all(&skills).unwrap();
        fs::write(
            skills.join("local.md"),
            "---\nagent: codex:o3\n---\nLocal skill summary.\n\nDo it.\n",
        )
        .unwrap();

        let report = sync_skills(&options_for(&home)).unwrap();
        assert!(report
            .written
            .iter()
            .any(|path| path.ends_with(".claude/skills/local/SKILL.md")));
        assert!(report
            .written
            .iter()
            .any(|path| path.ends_with(".agents/skills/local/SKILL.md")));

        let claude = fs::read_to_string(home.path().join(".claude/skills/local/SKILL.md")).unwrap();
        assert!(claude.contains("description: Local skill summary."));
        assert!(claude.contains("loopflow: true"));
        assert!(claude.contains("loopflow-skill: local"));
        assert!(!claude.contains("disable-model-invocation"));
        assert!(!claude.contains("agent: codex:o3"));
        assert!(claude.contains("Do it."));

        let codex = fs::read_to_string(home.path().join(".agents/skills/local/SKILL.md")).unwrap();
        assert!(!codex.contains("disable-model-invocation"));
    }

    #[test]
    fn sync_skills_compiles_builtin_skills() {
        let home = TempDir::new().unwrap();

        let report = sync_skills(&options_for(&home)).unwrap();

        // Builtin skills are always compiled, even with an empty home.
        assert!(!report.written.is_empty());
        assert!(report
            .written
            .iter()
            .all(|path| path.starts_with(home.path())));
        assert!(home
            .path()
            .join(".agents/skills/wave-operate/SKILL.md")
            .exists());
        assert!(!home
            .path()
            .join(".agents/skills/wave_operate/SKILL.md")
            .exists());

        // Standalone vendor skills receive their procedures without an ambient prompt.
        for vendor in [".agents", ".claude"] {
            let root = home.path().join(vendor).join("skills");
            for name in builtins::builtin_skill_names() {
                let exported = fs::read_to_string(root.join(name).join("SKILL.md")).unwrap();
                let source = builtins::get_builtin_skill(name).unwrap();
                let body = split_frontmatter(source).map_or(source, |(_, body)| body);
                assert!(
                    exported.contains(body.trim()),
                    "{vendor} omitted {name}'s method"
                );
            }
            assert!(!root.join("review-slice/SKILL.md").exists());
            assert!(!root.join("refresh-plan/SKILL.md").exists());
            assert!(!root.join("update-wave/SKILL.md").exists());
            assert!(!root.join("record-learnings/SKILL.md").exists());
            assert!(!root.join("loopflow/SKILL.md").exists());
            assert!(!root.join("review-open-work/SKILL.md").exists());
            let control = fs::read_to_string(root.join("repo-operate/SKILL.md")).unwrap();
            assert!(control.contains("--repo <repo> set machine"));
            let implement = fs::read_to_string(root.join("implement/SKILL.md")).unwrap();
            assert!(!implement.contains("--repo <repo> set machine"));
        }
    }

    #[test]
    fn operator_skills_stay_on_existing_work() {
        for name in ["repo-operate", "wave-operate"] {
            let skill = builtins::get_builtin_skill(name).unwrap();
            for planning in ["lf task create", "update-project", "lf task claim", "KR"] {
                assert!(!skill.contains(planning), "{name} names {planning:?}");
            }
            assert!(skill.contains("lf config user"), "{name} omits ownership");
        }
        for name in ["s1", "s2", "s3", "s4", "s5"] {
            let skill = builtins::get_builtin_skill(name).unwrap();
            for planning in ["lf task create", "update-project"] {
                assert!(!skill.contains(planning), "{name} names {planning:?}");
            }
        }
    }

    #[test]
    fn sync_skills_never_writes_under_a_repo() {
        let home = TempDir::new().unwrap();
        let skills = home.path().join(".lf/skills");
        fs::create_dir_all(&skills).unwrap();
        fs::write(skills.join("global-only.md"), "Global only.\n").unwrap();

        let report = sync_skills(&options_for(&home)).unwrap();

        // Everything lands under home; nothing outside it.
        assert!(report
            .written
            .iter()
            .all(|path| path.starts_with(home.path())));
        assert!(home
            .path()
            .join(".agents/skills/global-only/SKILL.md")
            .exists());
    }

    #[test]
    fn sync_skills_prunes_only_generated_skills() {
        let home = TempDir::new().unwrap();
        let user_dir = home.path().join(".agents/skills/user");
        fs::create_dir_all(&user_dir).unwrap();
        fs::write(
            user_dir.join(SKILL_FILE_NAME),
            "---\nname: user\n---\nkeep\n",
        )
        .unwrap();

        // Catalog tests own which names retire; pruning handles flat and nested paths.
        let retired = ["restore", "task/clarify", "vsm/operate"];
        for name in retired {
            let dir = home.path().join(".agents/skills").join(name);
            fs::create_dir_all(&dir).unwrap();
            fs::write(
                dir.join(SKILL_FILE_NAME),
                format!("---\nname: {name}\nloopflow: true\n---\nold\n"),
            )
            .unwrap();
        }
        let report = sync_skills(&options_for(&home)).unwrap();
        for name in retired {
            let dir = home.path().join(".agents/skills").join(name);
            assert!(!dir.exists(), "{name}");
            assert!(report.pruned.contains(&dir.join(SKILL_FILE_NAME)), "{name}");
        }
        assert_eq!(
            fs::read_to_string(user_dir.join(SKILL_FILE_NAME)).unwrap(),
            "---\nname: user\n---\nkeep\n"
        );
    }
}

pub mod listing;
