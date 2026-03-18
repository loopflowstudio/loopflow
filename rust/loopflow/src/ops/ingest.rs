use std::path::{Path, PathBuf};

use crate::engine::worktrees::main_repo_root;
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;
use crate::ops::util::resolve_wave_name;

#[derive(Debug, Clone, Default)]
pub struct IngestOptions {
    pub wave: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IngestResult {
    pub wave: String,
    pub slug: String,
    pub dest: PathBuf,
}

/// Fast-path ingest: pick the lowest-numbered wave item and move it to scratch/.
///
/// Returns Ok if exactly one file has the lowest numeric prefix.
/// Returns Err if: no wave found, no items, or multiple items share the lowest prefix.
pub fn ingest(
    repo: &Path,
    options: &IngestOptions,
    progress: &impl Progress,
) -> OpsResult<IngestResult> {
    let wave = resolve_wave_name(repo, options.wave.as_deref())
        .ok_or_else(|| OpsError::Message("cannot determine wave name".to_string()))?;

    let main_repo = main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
    let wave_dir = main_repo.join("wave").join(&wave);

    if !wave_dir.is_dir() {
        return Err(OpsError::Message(format!(
            "wave directory not found: {}",
            wave_dir.display()
        )));
    }

    let items = list_numbered_items(&wave_dir)?;

    if items.is_empty() {
        return Err(OpsError::Message(format!(
            "no numbered items in wave/{wave}/"
        )));
    }

    let min_prefix = items
        .iter()
        .map(|i| i.prefix)
        .min()
        .expect("items is non-empty");
    let lowest: Vec<&WaveItem> = items.iter().filter(|i| i.prefix == min_prefix).collect();

    if lowest.len() > 1 {
        let names: Vec<&str> = lowest.iter().map(|i| i.filename.as_str()).collect();
        return Err(OpsError::Message(format!(
            "multiple items with prefix {:02}: {}",
            min_prefix,
            names.join(", ")
        )));
    }

    let item = lowest[0];
    let scratch_dir = repo.join("scratch");
    std::fs::create_dir_all(&scratch_dir)?;

    let dest_name = format!("{}-{}.md", wave, item.slug);
    let dest = scratch_dir.join(&dest_name);
    let source = wave_dir.join(&item.filename);

    progress.status(&format!(
        "ingest: {} → scratch/{}",
        item.filename, dest_name
    ));

    // Can't use fs::rename — source (wave/) may be in the main repo
    // while dest (scratch/) is in a worktree on a different mount point.
    std::fs::copy(&source, &dest)?;
    std::fs::remove_file(&source)?;

    Ok(IngestResult {
        wave,
        slug: item.slug.clone(),
        dest,
    })
}

#[derive(Debug, Clone)]
pub(crate) struct WaveItem {
    pub filename: String,
    pub prefix: u32,
    pub slug: String,
}

/// List numbered .md files in a wave directory, skipping README.md.
pub(crate) fn list_numbered_items(dir: &Path) -> OpsResult<Vec<WaveItem>> {
    let mut items = Vec::new();

    let entries = std::fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();

        if !name.ends_with(".md") {
            continue;
        }
        if name.eq_ignore_ascii_case("README.md") {
            continue;
        }

        if let Some(item) = parse_numbered_item(&name) {
            items.push(item);
        }
    }

    items.sort_by_key(|i| i.prefix);
    Ok(items)
}

/// Parse "02-mac-mini-dogfood.md" into prefix=2, slug="mac-mini-dogfood".
fn parse_numbered_item(filename: &str) -> Option<WaveItem> {
    let stem = filename.strip_suffix(".md")?;
    let dash_pos = stem.find('-')?;
    let prefix_str = &stem[..dash_pos];
    let prefix: u32 = prefix_str.parse().ok()?;
    let slug = &stem[dash_pos + 1..];

    if slug.is_empty() {
        return None;
    }

    Some(WaveItem {
        filename: filename.to_string(),
        prefix,
        slug: slug.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::NullProgress;
    use tempfile::TempDir;

    #[test]
    fn parse_numbered_item_basic() {
        let item = parse_numbered_item("01-setup.md").unwrap();
        assert_eq!(item.prefix, 1);
        assert_eq!(item.slug, "setup");
    }

    #[test]
    fn parse_numbered_item_multi_dash() {
        let item = parse_numbered_item("02-mac-mini-dogfood.md").unwrap();
        assert_eq!(item.prefix, 2);
        assert_eq!(item.slug, "mac-mini-dogfood");
    }

    #[test]
    fn parse_numbered_item_rejects_readme() {
        assert!(parse_numbered_item("README.md").is_none());
    }

    #[test]
    fn parse_numbered_item_rejects_non_numeric() {
        assert!(parse_numbered_item("abc-setup.md").is_none());
    }

    #[test]
    fn parse_numbered_item_rejects_no_slug() {
        assert!(parse_numbered_item("01-.md").is_none());
    }

    #[test]
    fn list_numbered_items_filters_readme() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("README.md"), "# Wave").unwrap();
        std::fs::write(dir.path().join("01-first.md"), "# First").unwrap();
        std::fs::write(dir.path().join("02-second.md"), "# Second").unwrap();
        std::fs::write(dir.path().join("notes.txt"), "ignored").unwrap();

        let items = list_numbered_items(dir.path()).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].prefix, 1);
        assert_eq!(items[1].prefix, 2);
    }

    #[test]
    fn ingest_single_lowest() {
        let dir = TempDir::new().unwrap();
        let repo = dir.path();

        // Set up git repo so resolve_wave_name works
        std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo)
            .output()
            .unwrap();

        // Create wave directory
        let wave_dir = repo.join("wave").join("test-wave");
        std::fs::create_dir_all(&wave_dir).unwrap();
        std::fs::write(wave_dir.join("README.md"), "# Test").unwrap();
        std::fs::write(wave_dir.join("01-first.md"), "# First item").unwrap();
        std::fs::write(wave_dir.join("02-second.md"), "# Second item").unwrap();

        let result = ingest(
            repo,
            &IngestOptions {
                wave: Some("test-wave".to_string()),
            },
            &NullProgress,
        )
        .unwrap();

        assert_eq!(result.wave, "test-wave");
        assert_eq!(result.slug, "first");
        assert!(result.dest.ends_with("scratch/test-wave-first.md"));
        assert!(result.dest.exists());
        assert!(!wave_dir.join("01-first.md").exists());
        assert!(wave_dir.join("02-second.md").exists());
    }

    #[test]
    fn ingest_multiple_same_prefix_errors() {
        let dir = TempDir::new().unwrap();
        let repo = dir.path();

        std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo)
            .output()
            .unwrap();

        let wave_dir = repo.join("wave").join("test-wave");
        std::fs::create_dir_all(&wave_dir).unwrap();
        std::fs::write(wave_dir.join("01-first.md"), "# A").unwrap();
        std::fs::write(wave_dir.join("01-second.md"), "# B").unwrap();

        let result = ingest(
            repo,
            &IngestOptions {
                wave: Some("test-wave".to_string()),
            },
            &NullProgress,
        );

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("multiple items with prefix 01"));
    }

    #[test]
    fn ingest_empty_wave_errors() {
        let dir = TempDir::new().unwrap();
        let repo = dir.path();

        std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo)
            .output()
            .unwrap();

        let wave_dir = repo.join("wave").join("test-wave");
        std::fs::create_dir_all(&wave_dir).unwrap();
        std::fs::write(wave_dir.join("README.md"), "# Test").unwrap();

        let result = ingest(
            repo,
            &IngestOptions {
                wave: Some("test-wave".to_string()),
            },
            &NullProgress,
        );

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("no numbered items"));
    }
}
