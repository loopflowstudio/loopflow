use crate::ops::{OpsError, OpsResult, Progress};
use std::path::{Path, PathBuf};
use std::process::Command;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn sync(repo_root: &Path, progress: &dyn Progress) -> OpsResult<SyncResult> {
    let cache_dir = cache_dir()?;
    let manifest_path = repo_root.join(".lf/steps/gstack/workstyle.yaml");
    let output_dir = repo_root.join(".lf/steps/gstack");
    let direction_output = repo_root.join("rust/loopflow/src/engine/builtins/directions/gstack.md");

    // Snapshot current steps for diffing
    let before = snapshot_steps(&output_dir);

    // Clone or fetch
    if cache_dir.join(".git").exists() {
        progress.status("Fetching garrytan/gstack...");
        git_fetch(&cache_dir)?;
    } else {
        progress.status("Cloning garrytan/gstack...");
        git_clone(&cache_dir)?;
    }

    let upstream_head = git_head(&cache_dir)?;
    let last_commit = read_last_commit(&manifest_path);

    if last_commit.as_deref() == Some(upstream_head.as_str()) {
        progress.status("Already up to date.");
        return Ok(SyncResult {
            commit: upstream_head,
            added: vec![],
            changed: vec![],
            removed: vec![],
        });
    }

    // Run the Python converter
    progress.status(&format!(
        "Converting skills ({}..{})...",
        short_sha(&last_commit.unwrap_or_default()),
        short_sha(&upstream_head),
    ));
    run_converter(&cache_dir, &output_dir, &direction_output)?;
    flatten_converter_output(&output_dir)?;

    // Diff
    let after = snapshot_steps(&output_dir);
    let (added, changed, removed) = diff_snapshots(&before, &after);

    let total_changed = added.len() + changed.len() + removed.len();
    let total_steps = after.len();
    progress.status(&format!(
        "Synced to {} — {total_steps} steps, {total_changed} changed.",
        short_sha(&upstream_head),
    ));

    for name in &added {
        progress.status(&format!("  + {name}"));
    }
    for name in &changed {
        progress.status(&format!("  ~ {name}"));
    }
    for name in &removed {
        progress.status(&format!("  - {name}"));
    }

    Ok(SyncResult {
        commit: upstream_head,
        added,
        changed,
        removed,
    })
}

pub fn diff(repo_root: &Path, progress: &dyn Progress) -> OpsResult<DiffResult> {
    let cache_dir = cache_dir()?;
    let manifest_path = repo_root.join(".lf/steps/gstack/workstyle.yaml");

    if !cache_dir.join(".git").exists() {
        progress.status("Cloning garrytan/gstack...");
        git_clone(&cache_dir)?;
    } else {
        progress.status("Fetching garrytan/gstack...");
        git_fetch(&cache_dir)?;
    }

    let upstream_head = git_head(&cache_dir)?;
    let last_commit = read_last_commit(&manifest_path);

    if last_commit.as_deref() == Some(upstream_head.as_str()) {
        progress.status("Already up to date.");
        return Ok(DiffResult {
            current_commit: upstream_head.clone(),
            upstream_commit: upstream_head,
            commits_behind: 0,
            changed_skills: vec![],
        });
    }

    let commits_behind = if let Some(ref base) = last_commit {
        count_commits(&cache_dir, base, &upstream_head).unwrap_or(0)
    } else {
        0
    };

    let changed_skills = if let Some(ref base) = last_commit {
        changed_skill_names(&cache_dir, base, &upstream_head)?
    } else {
        vec!["(full initial sync needed)".to_string()]
    };

    progress.status(&format!(
        "{} commits behind ({}..{})",
        commits_behind,
        short_sha(&last_commit.clone().unwrap_or_default()),
        short_sha(&upstream_head),
    ));
    for name in &changed_skills {
        progress.status(&format!("  {name}"));
    }

    Ok(DiffResult {
        current_commit: last_commit.unwrap_or_default(),
        upstream_commit: upstream_head,
        commits_behind,
        changed_skills,
    })
}

pub fn list(repo_root: &Path, progress: &dyn Progress) -> OpsResult<ListResult> {
    let output_dir = repo_root.join(".lf/steps/gstack");
    let manifest_path = output_dir.join("workstyle.yaml");

    if !manifest_path.exists() {
        return Err(OpsError::Message(
            "No gstack steps found. Run `lf op gstack sync` first.".into(),
        ));
    }

    let last_commit = read_last_commit(&manifest_path).unwrap_or_default();
    let last_sync = read_last_sync(&manifest_path).unwrap_or_default();

    let mut steps: Vec<String> = std::fs::read_dir(&output_dir)
        .map_err(|e| OpsError::Message(format!("Failed to read {}: {e}", output_dir.display())))?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".md") {
                Some(name.trim_end_matches(".md").to_string())
            } else {
                None
            }
        })
        .collect();
    steps.sort();

    progress.status(&format!(
        "gstack — garrytan/gstack@main — synced {} ({})",
        &last_sync,
        short_sha(&last_commit),
    ));
    progress.status(&format!("{} steps installed:", steps.len()));
    for name in &steps {
        progress.status(&format!("  {name}"));
    }

    // List flows
    let flows_dir = repo_root.join(".lf/flows/gstack");
    let mut flows: Vec<String> = Vec::new();
    if flows_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&flows_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".yaml") {
                    flows.push(name.trim_end_matches(".yaml").to_string());
                }
            }
        }
        flows.sort();
    }
    if !flows.is_empty() {
        progress.status(&format!("{} flows:", flows.len()));
        for name in &flows {
            progress.status(&format!("  {name}"));
        }
    }

    Ok(ListResult {
        last_commit,
        last_sync,
        steps,
        flows,
    })
}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct SyncResult {
    pub commit: String,
    pub added: Vec<String>,
    pub changed: Vec<String>,
    pub removed: Vec<String>,
}

#[derive(Debug)]
pub struct DiffResult {
    pub current_commit: String,
    pub upstream_commit: String,
    pub commits_behind: usize,
    pub changed_skills: Vec<String>,
}

#[derive(Debug)]
pub struct ListResult {
    pub last_commit: String,
    pub last_sync: String,
    pub steps: Vec<String>,
    pub flows: Vec<String>,
}

// ---------------------------------------------------------------------------
// Git helpers
// ---------------------------------------------------------------------------

fn cache_dir() -> OpsResult<PathBuf> {
    let home = std::env::var("HOME").map_err(|_| OpsError::Message("HOME not set".into()))?;
    let dir = PathBuf::from(home).join(".lf/cache/gstack");
    Ok(dir)
}

fn git_clone(target: &Path) -> OpsResult<()> {
    std::fs::create_dir_all(target)
        .map_err(|e| OpsError::Message(format!("Failed to create cache dir: {e}")))?;

    let status = Command::new("git")
        .args(["clone", "https://github.com/garrytan/gstack.git"])
        .arg(target)
        .status()
        .map_err(|e| OpsError::Message(format!("git clone failed: {e}")))?;

    if !status.success() {
        return Err(OpsError::Message("git clone failed".into()));
    }
    Ok(())
}

fn git_fetch(repo: &Path) -> OpsResult<()> {
    let status = Command::new("git")
        .args(["-C"])
        .arg(repo)
        .args(["fetch", "origin", "main"])
        .status()
        .map_err(|e| OpsError::Message(format!("git fetch failed: {e}")))?;

    if !status.success() {
        return Err(OpsError::Message("git fetch failed".into()));
    }

    // Reset to origin/main
    let status = Command::new("git")
        .args(["-C"])
        .arg(repo)
        .args(["reset", "--hard", "origin/main"])
        .status()
        .map_err(|e| OpsError::Message(format!("git reset failed: {e}")))?;

    if !status.success() {
        return Err(OpsError::Message("git reset to origin/main failed".into()));
    }
    Ok(())
}

fn git_head(repo: &Path) -> OpsResult<String> {
    let output = Command::new("git")
        .args(["-C"])
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| OpsError::Message(format!("git rev-parse failed: {e}")))?;

    if !output.status.success() {
        return Err(OpsError::Message("git rev-parse HEAD failed".into()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn count_commits(repo: &Path, base: &str, head: &str) -> OpsResult<usize> {
    let output = Command::new("git")
        .args(["-C"])
        .arg(repo)
        .args(["rev-list", "--count", &format!("{base}..{head}")])
        .output()
        .map_err(|e| OpsError::Message(format!("git rev-list failed: {e}")))?;

    if !output.status.success() {
        return Err(OpsError::Message("git rev-list --count failed".into()));
    }
    let count_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    count_str
        .parse()
        .map_err(|e| OpsError::Message(format!("Failed to parse commit count: {e}")))
}

fn changed_skill_names(repo: &Path, base: &str, head: &str) -> OpsResult<Vec<String>> {
    let output = Command::new("git")
        .args(["-C"])
        .arg(repo)
        .args([
            "diff",
            "--name-only",
            &format!("{base}..{head}"),
            "--",
            "*/SKILL.md",
            "SKILL.md",
            "GSTACK.md",
        ])
        .output()
        .map_err(|e| OpsError::Message(format!("git diff failed: {e}")))?;

    if !output.status.success() {
        // Shallow clone may not have the base commit — fall back
        return Ok(vec![
            "(cannot diff — base commit not in shallow history)".to_string()
        ]);
    }

    let names: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| {
            // "office-hours/SKILL.md" -> "office-hours"
            // "SKILL.md" -> "gstack"
            if line == "SKILL.md" || line == "GSTACK.md" {
                line.to_string()
            } else {
                line.split('/').next().unwrap_or(line).to_string()
            }
        })
        .collect();
    Ok(names)
}

// ---------------------------------------------------------------------------
// Converter
// ---------------------------------------------------------------------------

fn run_converter(source: &Path, output_dir: &Path, direction_output: &Path) -> OpsResult<()> {
    let output = Command::new("uv")
        .args(["run", "python", "-m", "loopflow.workstyle.convert"])
        .arg(source)
        .arg(output_dir)
        .args(["--direction-output"])
        .arg(direction_output)
        .output()
        .map_err(|e| OpsError::Message(format!("Failed to run converter: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(OpsError::Message(format!("Converter failed:\n{stderr}")));
    }
    Ok(())
}

/// The converter writes steps to `output_dir/steps/*.md`. Move them up to
/// `output_dir/*.md` so the flat layout matches what loopflow expects, then
/// remove the now-empty `steps/` subdirectory.
fn flatten_converter_output(output_dir: &Path) -> OpsResult<()> {
    let steps_subdir = output_dir.join("steps");
    if !steps_subdir.exists() {
        return Ok(());
    }

    // Remove old .md files in the output dir (will be replaced by converter output)
    for entry in std::fs::read_dir(output_dir)
        .map_err(|e| OpsError::Message(format!("Failed to read output dir: {e}")))?
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".md") {
            std::fs::remove_file(entry.path()).ok();
        }
    }

    // Move steps/*.md up
    for entry in std::fs::read_dir(&steps_subdir)
        .map_err(|e| OpsError::Message(format!("Failed to read steps subdir: {e}")))?
        .flatten()
    {
        let dest = output_dir.join(entry.file_name());
        std::fs::rename(entry.path(), dest)
            .map_err(|e| OpsError::Message(format!("Failed to move step file: {e}")))?;
    }

    std::fs::remove_dir(&steps_subdir).ok();
    Ok(())
}

// ---------------------------------------------------------------------------
// Step snapshots for diffing
// ---------------------------------------------------------------------------

/// Read all .md files in a directory and return (name, content) pairs.
fn snapshot_steps(dir: &Path) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return result,
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".md") {
            let step_name = name.trim_end_matches(".md").to_string();
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                result.push((step_name, content));
            }
        }
    }
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

fn diff_snapshots(
    before: &[(String, String)],
    after: &[(String, String)],
) -> (Vec<String>, Vec<String>, Vec<String>) {
    use std::collections::HashMap;
    let before_map: HashMap<&str, &str> = before
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let after_map: HashMap<&str, &str> = after
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    let mut added = Vec::new();
    let mut changed = Vec::new();
    let mut removed = Vec::new();

    for (name, content) in after {
        match before_map.get(name.as_str()) {
            None => added.push(name.clone()),
            Some(old) if *old != content.as_str() => changed.push(name.clone()),
            _ => {}
        }
    }
    for (name, _) in before {
        if !after_map.contains_key(name.as_str()) {
            removed.push(name.clone());
        }
    }

    added.sort();
    changed.sort();
    removed.sort();
    (added, changed, removed)
}

// ---------------------------------------------------------------------------
// Manifest helpers
// ---------------------------------------------------------------------------

fn read_last_commit(manifest_path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(manifest_path).ok()?;
    let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content).ok()?;
    doc.get("source")?
        .get("last_commit")?
        .as_str()
        .map(|s| s.to_string())
}

fn read_last_sync(manifest_path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(manifest_path).ok()?;
    let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content).ok()?;
    doc.get("source")?
        .get("last_sync")?
        .as_str()
        .map(|s| s.to_string())
}

fn short_sha(sha: &str) -> &str {
    if sha.len() >= 7 {
        &sha[..7]
    } else {
        sha
    }
}
