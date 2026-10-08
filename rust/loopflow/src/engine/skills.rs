use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::engine::flow::split_frontmatter;
use crate::engine::skill_catalog::{is_generated, SkillCatalog, SkillDialect, SkillSource};
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

/// Export the personal catalog and builtins without replacing third-party files.
pub fn sync_skills(options: &SkillSyncOptions) -> Result<SkillSyncReport, LoadError> {
    let home = options
        .global_home
        .clone()
        .or_else(dirs::home_dir)
        .ok_or_else(|| LoadError::InvalidSkill("home directory not found".to_string()))?;

    let catalog = SkillCatalog::load(None, Some(&home), options.global_home.is_none())?;
    let mut report = SkillSyncReport::default();
    write_targets(
        &catalog,
        &home.join(".claude/skills"),
        Vendor::Claude,
        options.prune,
        &mut report,
    )?;
    write_targets(
        &catalog,
        &home.join(".agents/skills"),
        Vendor::Codex,
        options.prune,
        &mut report,
    )?;

    report.written.sort();
    report.pruned.sort();
    Ok(report)
}

fn write_targets(
    catalog: &SkillCatalog,
    target_root: &Path,
    vendor: Vendor,
    prune: bool,
    report: &mut SkillSyncReport,
) -> Result<(), LoadError> {
    fs::create_dir_all(target_root)?;
    let desired: BTreeSet<_> = catalog.entries().map(|skill| skill.name.as_str()).collect();

    for skill in catalog.entries() {
        let path = target_root.join(&skill.name).join(SKILL_FILE_NAME);
        // Third-party definitions own their paths, even on a name collision.
        if path.exists() && !is_generated(&path) {
            continue;
        }
        let content = render_skill(skill, vendor)?;
        if fs::read_to_string(&path).ok().as_deref() == Some(content.as_str()) {
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content)?;
        if matches!(
            (skill.dialect, vendor),
            (SkillDialect::Claude, Vendor::Codex) | (SkillDialect::Codex, Vendor::Claude)
        ) {
            eprintln!("warning: exported {} across harnesses; native argument and control declarations are retained, not translated", skill.name);
        }
        report.written.push(path);
    }

    if prune {
        for path in generated_skill_files(target_root)? {
            let Some(name) = synced_skill_name_from_path(target_root, &path) else {
                continue;
            };
            if desired.contains(name.as_str()) {
                continue;
            }
            fs::remove_file(&path)?;
            report.pruned.push(path.clone());
            prune_empty_skill_dir(&path, target_root)?;
        }
    }

    Ok(())
}

fn render_skill(skill: &SkillSource, vendor: Vendor) -> Result<String, LoadError> {
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
    let mut frontmatter = Vec::new();
    frontmatter.push(format!("name: {}", yaml_string(&skill.name)));
    frontmatter.push(format!("description: {}", yaml_string(&description)));
    frontmatter.push(LOOPFLOW_MARKER.to_string());
    frontmatter.push(format!("loopflow-skill: {}", yaml_string(&skill.name)));
    if vendor == Vendor::Claude && skill.dialect == SkillDialect::Loopflow {
        frontmatter.push("disable-model-invocation: true".to_string());
    }

    if skill.dialect != SkillDialect::Loopflow {
        // Preserve authored declarations; the destination provider decides which it supports.
        if let Ok(serde_yaml_ng::Value::Mapping(mut metadata)) =
            serde_yaml_ng::from_str(original_frontmatter)
        {
            for field in ["name", "description", "loopflow", "loopflow-skill"] {
                metadata.remove(serde_yaml_ng::Value::String(field.into()));
            }
            if !metadata.is_empty() {
                frontmatter.push(
                    serde_yaml_ng::to_string(&metadata)
                        .map_err(|error| LoadError::InvalidSkill(error.to_string()))?,
                );
            }
        }
    }
    Ok(format!(
        "---\n{}\n---\n{source}{body}",
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
        }
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
        assert!(home
            .path()
            .join(".agents/skills/wave/operate/SKILL.md")
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
            let control = fs::read_to_string(root.join("repo/operate/SKILL.md")).unwrap();
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
