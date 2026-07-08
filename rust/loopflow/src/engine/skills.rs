use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::engine::builtins;
use crate::engine::flow::split_frontmatter;
use crate::engine::LoadError;

const SKILL_FILE_NAME: &str = "SKILL.md";
const LOOPFLOW_MARKER: &str = "loopflow: true";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillSyncOptions {
    pub prune: bool,
    pub global_home: Option<PathBuf>,
}

impl Default for SkillSyncOptions {
    fn default() -> Self {
        Self {
            prune: true,
            global_home: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillSyncReport {
    pub written: Vec<PathBuf>,
    pub pruned: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Vendor {
    Claude,
    Codex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SkillSource {
    name: String,
    content: String,
}

/// Compile loopflow skills (builtins + `~/.lf/`) into the user's personal home
/// agent skill directories (`~/.claude/skills`, `~/.agents/skills`). Skills
/// never land inside a working repo — the home dirs are the sole target.
pub fn sync_skills(options: &SkillSyncOptions) -> Result<SkillSyncReport, LoadError> {
    let home = options
        .global_home
        .clone()
        .or_else(dirs::home_dir)
        .ok_or_else(|| LoadError::InvalidSkill("home directory not found".to_string()))?;

    let global_skills = collect_global_skills(&home)?;
    let mut resolved = collect_builtin_skills();
    resolved.extend(global_skills);

    let mut report = SkillSyncReport::default();
    write_targets(
        &resolved,
        &home.join(".claude/skills"),
        Vendor::Claude,
        options.prune,
        &mut report,
    )?;
    write_targets(
        &resolved,
        &home.join(".agents/skills"),
        Vendor::Codex,
        options.prune,
        &mut report,
    )?;

    report.written.sort();
    report.pruned.sort();
    Ok(report)
}

fn collect_builtin_skills() -> BTreeMap<String, SkillSource> {
    builtins::builtin_skill_names()
        .into_iter()
        .filter_map(|name| {
            builtins::get_builtin_skill(name).map(|content| {
                (
                    name.to_string(),
                    SkillSource {
                        name: name.to_string(),
                        content: content.to_string(),
                    },
                )
            })
        })
        .collect()
}

fn collect_global_skills(home: &Path) -> Result<BTreeMap<String, SkillSource>, LoadError> {
    let mut skills = BTreeMap::new();
    collect_skill_dir(&home.join(".lf/skills"), &mut skills)?;
    collect_skill_dir(&home.join(".claude/commands"), &mut skills)?;
    Ok(skills)
}

fn collect_skill_dir(
    dir: &Path,
    skills: &mut BTreeMap<String, SkillSource>,
) -> Result<(), LoadError> {
    if !dir.is_dir() {
        return Ok(());
    }

    for path in markdown_files(dir)? {
        let Some(name) = skill_name_from_path(dir, &path) else {
            continue;
        };
        let content = fs::read_to_string(&path)?;
        skills.insert(name.clone(), SkillSource { name, content });
    }
    Ok(())
}

fn markdown_files(dir: &Path) -> Result<Vec<PathBuf>, LoadError> {
    let mut files = Vec::new();
    collect_markdown_files(dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_markdown_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), LoadError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_markdown_files(&path, files)?;
        } else if path.extension().is_some_and(|ext| ext == "md") {
            files.push(path);
        }
    }
    Ok(())
}

fn skill_name_from_path(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let mut without_extension = relative.to_path_buf();
    without_extension.set_extension("");
    Some(
        without_extension
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
    )
}

fn write_targets(
    skills: &BTreeMap<String, SkillSource>,
    target_root: &Path,
    vendor: Vendor,
    prune: bool,
    report: &mut SkillSyncReport,
) -> Result<(), LoadError> {
    fs::create_dir_all(target_root)?;
    let desired: BTreeSet<String> = skills.keys().cloned().collect();

    for skill in skills.values() {
        let path = skill_path(target_root, &skill.name);
        let content = render_skill(skill, vendor);
        if fs::read_to_string(&path).ok().as_deref() == Some(content.as_str()) {
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content)?;
        report.written.push(path);
    }

    if prune {
        for path in generated_skill_files(target_root)? {
            let Some(name) = synced_skill_name_from_path(target_root, &path) else {
                continue;
            };
            if desired.contains(&name) {
                continue;
            }
            fs::remove_file(&path)?;
            report.pruned.push(path.clone());
            prune_empty_skill_dir(&path, target_root)?;
        }
    }

    Ok(())
}

fn render_skill(skill: &SkillSource, vendor: Vendor) -> String {
    let (original_frontmatter, body) =
        split_frontmatter(&skill.content).unwrap_or_else(|| (String::new(), skill.content.clone()));
    let description = skill_description(&original_frontmatter, &body, &skill.name);

    let mut frontmatter = Vec::new();
    frontmatter.push(format!("name: {}", yaml_string(&skill.name)));
    frontmatter.push(format!("description: {}", yaml_string(&description)));
    frontmatter.push(LOOPFLOW_MARKER.to_string());
    frontmatter.push(format!("loopflow-skill: {}", yaml_string(&skill.name)));
    if vendor == Vendor::Claude {
        frontmatter.push("disable-model-invocation: true".to_string());
    }

    format!("---\n{}\n---\n{}", frontmatter.join("\n"), body)
}

fn skill_description(frontmatter: &str, body: &str, name: &str) -> String {
    if let Ok(value) = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(frontmatter) {
        if let Some(description) = value
            .as_mapping()
            .and_then(|map| map.get(serde_yaml_ng::Value::String("description".to_string())))
            .and_then(serde_yaml_ng::Value::as_str)
            .map(first_line)
            .filter(|line| !line.is_empty())
        {
            return description;
        }
    }

    first_prose_line(body).unwrap_or_else(|| format!("Run the loopflow {name} skill."))
}

fn first_line(value: &str) -> String {
    value.lines().next().unwrap_or("").trim().to_string()
}

fn first_prose_line(body: &str) -> Option<String> {
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

fn skill_path(root: &Path, name: &str) -> PathBuf {
    name.split('/')
        .fold(root.to_path_buf(), |path, segment| path.join(segment))
        .join(SKILL_FILE_NAME)
}

fn generated_skill_files(root: &Path) -> Result<Vec<PathBuf>, LoadError> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    collect_skill_files(root, &mut files)?;
    files.retain(|path| is_loopflow_generated(path));
    Ok(files)
}

fn collect_skill_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), LoadError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_skill_files(&path, files)?;
        } else if path.file_name().is_some_and(|name| name == SKILL_FILE_NAME) {
            files.push(path);
        }
    }
    Ok(())
}

fn is_loopflow_generated(path: &Path) -> bool {
    let Ok(content) = fs::read_to_string(path) else {
        return false;
    };
    let Some((frontmatter, _body)) = split_frontmatter(&content) else {
        return false;
    };
    frontmatter
        .lines()
        .any(|line| line.trim() == LOOPFLOW_MARKER)
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
    use super::*;
    use tempfile::TempDir;

    fn options_for(home: &TempDir) -> SkillSyncOptions {
        SkillSyncOptions {
            prune: true,
            global_home: Some(home.path().to_path_buf()),
        }
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
        assert!(claude.contains("disable-model-invocation: true"));
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
        let stale_dir = home.path().join(".agents/skills/stale");
        fs::create_dir_all(&stale_dir).unwrap();
        fs::write(
            stale_dir.join(SKILL_FILE_NAME),
            "---\nname: stale\nloopflow: true\n---\nold\n",
        )
        .unwrap();
        let user_dir = home.path().join(".agents/skills/user");
        fs::create_dir_all(&user_dir).unwrap();
        fs::write(
            user_dir.join(SKILL_FILE_NAME),
            "---\nname: user\n---\nkeep\n",
        )
        .unwrap();

        let report = sync_skills(&options_for(&home)).unwrap();
        assert!(report
            .pruned
            .iter()
            .any(|path| path.ends_with(".agents/skills/stale/SKILL.md")));
        assert!(!stale_dir.join(SKILL_FILE_NAME).exists());
        assert!(user_dir.join(SKILL_FILE_NAME).exists());
    }
}
