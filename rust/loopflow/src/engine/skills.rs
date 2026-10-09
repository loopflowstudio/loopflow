use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::engine::definition_name::portable_name;
use crate::engine::flow::{compile_flow_with_catalog, split_frontmatter, DefinitionLoader};
use crate::engine::flow_instructions::render_flow_instructions;
use crate::engine::skill_catalog::{is_generated, SkillCatalog, SkillDialect, SkillSource};
use crate::engine::LoadError;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Vendor {
    Claude,
    Codex,
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
    let flows = match &options.repo {
        Some(repo) => crate::engine::flow::repo_flow_names(repo)?,
        None => crate::engine::builtins::builtin_flow_names()
            .into_iter()
            .map(str::to_string)
            .collect(),
    };
    let repo = options.repo.as_ref().map(std::path::absolute).transpose()?;
    let mut all_flows = crate::engine::builtins::builtin_flow_names()
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    all_flows.extend(flows.iter().cloned());
    let mut recipes = BTreeMap::new();
    let mut targets = Vec::new();
    let mut report = SkillSyncReport::default();
    // Resolve and validate the entire export set before touching either provider.
    for (folder, vendor) in [
        (".claude/skills", Vendor::Claude),
        (".agents/skills", Vendor::Codex),
    ] {
        let root = base.join(folder);
        let mut exports = BTreeMap::new();
        let skills = project_names(
            catalog.entries().map(|skill| skill.name.as_str()),
            vendor,
            &root,
            &mut report,
        )?;
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
            exports.insert(name.clone(), render_skill(skill, name, vendor)?);
        }
        for (name, flow) in
            project_names(flows.iter().map(String::as_str), vendor, &root, &mut report)?
        {
            if skills.contains_key(&name) {
                let path = root.join(&name).join(SKILL_FILE_NAME);
                eprintln!(
                    "warning: skipping Flow {flow:?}: skill owns the native name {name:?} at {}",
                    path.display()
                );
                report.skipped.push(path);
                continue;
            }
            let recipe = match recipes.entry(flow.clone()) {
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::btree_map::Entry::Vacant(entry) => {
                    let definition = DefinitionLoader::new(options.repo.as_deref(), &catalog)
                        .load_flow(&flow)?;
                    let steps = compile_flow_with_catalog(&definition, &catalog)?;
                    entry.insert(render_flow_instructions(&flow, &steps, &all_flows)?)
                }
            };
            exports.insert(
                name.clone(),
                render_flow_skill(&flow, &name, vendor, recipe),
            );
        }
        let blocked = exports
            .keys()
            .filter(|name| {
                !writable_target(&root, name)
                    || exports
                        .keys()
                        .any(|other| other.starts_with(&format!("{name}/")))
            })
            .cloned()
            .collect::<BTreeSet<_>>();
        targets.push((root, exports, blocked));
    }
    for (root, exports, blocked) in &targets {
        write_targets(exports, blocked, root, &mut report)?;
    }
    // A failed replacement in either provider must not erase working old exports.
    if options.prune {
        for (root, exports, blocked) in &targets {
            prune_targets(exports, blocked, root, &mut report)?;
        }
    }
    report.written.sort();
    report.pruned.sort();
    report.skipped.sort();
    Ok(report)
}

/// Provider projection is independent of discovery order: literal targets win.
fn project_names<'a>(
    names: impl Iterator<Item = &'a str>,
    vendor: Vendor,
    root: &Path,
    report: &mut SkillSyncReport,
) -> Result<BTreeMap<String, String>, LoadError> {
    let mut candidates = BTreeMap::<String, BTreeSet<String>>::new();
    for name in names {
        let portable = portable_name(name);
        candidates
            .entry(portable)
            .or_default()
            .insert(name.to_string());
        if vendor == Vendor::Codex {
            candidates
                .entry(name.to_string())
                .or_default()
                .insert(name.to_string());
        }
    }
    let mut selected = BTreeMap::new();
    for (target, sources) in candidates {
        for component in target.split('/') {
            validate_export_name(component)?;
        }
        let winner = if sources.contains(&target) {
            Some(target.clone())
        } else if sources.len() == 1 {
            sources.first().cloned()
        } else {
            None
        };
        if winner.is_none() || (vendor == Vendor::Claude && sources.len() > 1) {
            eprintln!(
                "warning: skipping unrepresentable exports at {target:?}: {}",
                sources
                    .iter()
                    .filter(|source| Some(*source) != winner.as_ref())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            report
                .skipped
                .push(root.join(&target).join(SKILL_FILE_NAME));
        }
        if let Some(source) = winner {
            selected.insert(target, source);
        }
    }
    Ok(selected)
}

fn validate_export_name(name: &str) -> Result<(), LoadError> {
    if name.is_empty()
        || name.len() > 64
        || name.starts_with('-')
        || name.ends_with('-')
        || name.contains("--")
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(LoadError::InvalidSkill(format!("invalid portable skill name {name:?}; use 1–64 lowercase letters, digits and single separating hyphens")));
    }
    Ok(())
}

fn writable_target(root: &Path, name: &str) -> bool {
    let directory = root.join(name);
    let path = directory.join(SKILL_FILE_NAME);
    // Never write through third-party symlinks or bundle assets.
    if directory
        .ancestors()
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
    if path.exists() {
        return path.is_file() && is_generated(&path);
    }
    // A new bundle must not hide an existing namespace or claim its assets.
    !directory.is_dir()
        || fs::read_dir(&directory).is_ok_and(|mut entries| entries.next().is_none())
}

fn render_flow_skill(flow: &str, name: &str, vendor: Vendor, recipe: &str) -> String {
    let user_only = if vendor == Vendor::Claude {
        "disable-model-invocation: true\n"
    } else {
        ""
    };
    format!("---\nname: {name}\ndescription: Follow the {name} Flow in this conversation.\nloopflow: true\nloopflow-kind: flow\nloopflow-flow: {}\n{user_only}---\n{recipe}", yaml_string(flow))
}

fn write_targets(
    exports: &BTreeMap<String, String>,
    blocked: &BTreeSet<String>,
    target_root: &Path,
    report: &mut SkillSyncReport,
) -> Result<(), LoadError> {
    for (name, content) in exports {
        let path = target_root.join(name).join(SKILL_FILE_NAME);
        if blocked.contains(name) {
            eprintln!(
                "warning: skipping export at {}: destination is occupied or needed as a namespace",
                path.display()
            );
            report.skipped.push(path);
            continue;
        }
        if fs::read_to_string(&path).ok().as_deref() == Some(content.as_str()) {
            continue;
        }
        fs::create_dir_all(path.parent().expect("skill destination has a parent"))?;
        fs::write(&path, content)?;
        report.written.push(path);
    }
    Ok(())
}

fn prune_targets(
    exports: &BTreeMap<String, String>,
    blocked: &BTreeSet<String>,
    target_root: &Path,
    report: &mut SkillSyncReport,
) -> Result<(), LoadError> {
    if !target_root.ancestors().any(Path::is_symlink) {
        for path in generated_skill_files(target_root)? {
            let Some(name) = synced_skill_name_from_path(target_root, &path) else {
                continue;
            };
            if exports.contains_key(&name)
                || blocked
                    .iter()
                    .any(|blocked| portable_name(blocked) == portable_name(&name))
            {
                continue;
            }
            fs::remove_file(&path)?;
            report.pruned.push(path.clone());
            prune_empty_skill_dir(&path, target_root)?;
        }
    }
    Ok(())
}

fn render_skill(skill: &SkillSource, name: &str, vendor: Vendor) -> Result<String, LoadError> {
    if matches!(
        (skill.dialect, vendor),
        (SkillDialect::Claude, Vendor::Codex) | (SkillDialect::Codex, Vendor::Claude)
    ) {
        eprintln!("warning: exporting {} across harnesses; native argument and control declarations are retained, not translated", skill.name);
    }
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

pub(crate) fn skill_description(content: &str) -> Option<String> {
    let Some((frontmatter, body)) = split_frontmatter(content) else {
        return first_prose_line(content);
    };
    serde_yaml_ng::from_str::<serde_yaml_ng::Value>(frontmatter)
        .ok()
        .and_then(|value| {
            value
                .get("description")?
                .as_str()?
                .lines()
                .next()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
        })
        .or_else(|| first_prose_line(body))
}

pub(crate) fn first_prose_line(body: &str) -> Option<String> {
    for raw in body.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("```") {
            continue;
        }
        return Some(line.to_string());
    }
    None
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
    use super::{sync_skills, SkillSyncOptions, SKILL_FILE_NAME};
    use crate::engine::{builtins, flow::split_frontmatter};
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
    fn portable_flow_export_yields_to_third_party_skills_in_any_scope() {
        let home = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        let native = home.path().join(".claude/skills/team/ship/SKILL.md");
        fs::create_dir_all(native.parent().unwrap()).unwrap();
        fs::write(&native, "Third-party owns team-ship").unwrap();
        fs::create_dir_all(repo.path().join(".lf/flows")).unwrap();
        fs::write(
            repo.path().join(".lf/flows/team-ship.yaml"),
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
    fn portable_flow_exports_are_scoped_and_idempotent() {
        let home = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        fs::create_dir_all(repo.path().join(".lf/flows/team")).unwrap();
        fs::write(
            repo.path().join(".lf/flows/team/ship.yaml"),
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
            assert!(wrapper.contains("# Flow: team/ship"));
            assert!(wrapper.contains("1. Run `lf -b implement`."));
            assert!(wrapper.contains("loopflow-kind: flow"));
            assert!(!wrapper.contains("--instructions"));
            assert!(root.join("team-check/SKILL.md").exists());
            assert!(!root.join("pursue/SKILL.md").exists());
        }
        assert!(sync_skills(&options).unwrap().written.is_empty());
        fs::remove_file(repo.path().join(".lf/flows/team/ship.yaml")).unwrap();
        assert_eq!(sync_skills(&options).unwrap().pruned.len(), 3);
    }

    #[test]
    fn chat_recipes_refresh_repository_overrides_and_keep_global_scope() {
        let home = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        fs::create_dir_all(home.path().join(".lf/skills")).unwrap();
        fs::write(
            home.path().join(".lf/skills/loop-or-next.md"),
            "Personal decision criteria",
        )
        .unwrap();
        sync_skills(&options_for(&home)).unwrap();
        let global = home.path().join(".claude/skills/pursue/SKILL.md");
        let original = fs::read_to_string(&global).unwrap();
        assert!(original.contains("Execute the skill `loop-or-next` in this conversation."));
        assert!(original.contains(
            &home
                .path()
                .join(".lf/skills/loop-or-next.md")
                .display()
                .to_string()
        ));
        assert!(!original.contains("Personal decision criteria"));

        fs::create_dir_all(repo.path().join(".lf/flows")).unwrap();
        let flow = repo.path().join(".lf/flows/pursue.yaml");
        fs::write(&flow, "- step: {name: demo, id: review, human: true}\n").unwrap();
        let options = SkillSyncOptions {
            repo: Some(repo.path().to_path_buf()),
            ..options_for(&home)
        };
        sync_skills(&options).unwrap();
        let local = repo.path().join(".claude/skills/pursue/SKILL.md");
        assert!(fs::read_to_string(&local)
            .unwrap()
            .contains("1. Execute the skill `demo` in this conversation."));
        fs::write(&flow, "- compress\n").unwrap();
        sync_skills(&options).unwrap();
        let updated = fs::read_to_string(local).unwrap();
        assert!(updated.contains("1. Run `lf -b compress`."));
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
    fn portable_codex_preserves_literal_pairs_and_claude_prefers_dashed() {
        let home = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        for (kind, extension) in [("skills", "md"), ("flows", "yaml")] {
            for (name, body) in [("team/check", "Slash"), ("team-check", "Dash")] {
                let name = if kind == "flows" {
                    name.replace("check", "ship")
                } else {
                    name.to_string()
                };
                let path = repo.path().join(format!(".lf/{kind}/{name}.{extension}"));
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                let content = if kind == "flows" {
                    format!(
                        "- step: {}\n",
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
        assert!(report
            .skipped
            .contains(&repo.path().join(".claude/skills/team-check/SKILL.md")));
        for name in ["team/check", "team-check", "team/ship", "team-ship"] {
            let path = repo.path().join(format!(".agents/skills/{name}/SKILL.md"));
            let body = fs::read_to_string(path).unwrap();
            assert!(body.contains(&format!("name: {name}\n")));
            if name.contains("ship") {
                assert!(body.contains(&format!("# Flow: {name}\n")));
                assert!(body.contains(&format!(
                    "lf -b {}",
                    if name.contains('/') {
                        "team/check"
                    } else {
                        "team-check"
                    }
                )));
            } else {
                assert!(body.contains(if name.contains('/') { "Slash" } else { "Dash" }));
            }
        }
        let root = repo.path().join(".claude/skills");
        assert!(!root.join("team/check/SKILL.md").exists());
        assert!(fs::read_to_string(root.join("team-check/SKILL.md"))
            .unwrap()
            .contains("Dash"));
        assert!(fs::read_to_string(root.join("team-ship/SKILL.md"))
            .unwrap()
            .contains("# Flow: team-ship"));
        let again = sync_skills(&options).unwrap();
        assert!(again.written.is_empty() && again.pruned.is_empty());
    }

    #[test]
    fn portable_flow_collisions_yield_to_skills_without_prefixes() {
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
            assert!(!body.contains("loopflow-kind: flow"));
            assert!(!root.join("flow-pursue").exists());
        }
    }

    #[test]
    fn portable_invalid_names_fail_before_writes() {
        let home = TempDir::new().unwrap();
        fs::create_dir_all(home.path().join(".lf/skills")).unwrap();
        fs::write(
            home.path().join(".lf/skills/UPPER.md"),
            "Invalid portable name",
        )
        .unwrap();
        assert!(sync_skills(&options_for(&home))
            .unwrap_err()
            .to_string()
            .contains("invalid portable"));
        assert!(!home.path().join(".claude").exists());
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
            assert!(control.contains("lf wave place"));
            let implement = fs::read_to_string(root.join("implement/SKILL.md")).unwrap();
            assert!(!implement.contains("lf wave place"));
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
