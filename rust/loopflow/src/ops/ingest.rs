use std::path::{Path, PathBuf};

use crate::engine::worktrees::main_repo_root;
use crate::lfd::pm::PriorityBucket;
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

/// Fast-path ingest: pick the highest-priority roadmap item and move it to scratch/.
///
/// Priority files (`1-*` through `4-*`) take precedence over legacy numbered files.
/// Within the same bucket or stage, the fast path uses filename order.
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

    let items = list_wave_items(&wave_dir)?;

    if items.is_empty() {
        return Err(OpsError::Message(format!(
            "no roadmap items in wave/{wave}/"
        )));
    }

    let item = items.first().expect("items is non-empty");
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WaveItemOrder {
    Bucket(PriorityBucket),
    LegacyStage(u32),
}

#[derive(Debug, Clone)]
pub(crate) struct WaveItem {
    pub filename: String,
    pub order: WaveItemOrder,
    pub slug: String,
}

impl WaveItem {
    pub fn priority_bucket(&self) -> Option<PriorityBucket> {
        match self.order {
            WaveItemOrder::Bucket(bucket) => Some(bucket),
            WaveItemOrder::LegacyStage(_) => None,
        }
    }
}

/// List roadmap item files in a wave directory, skipping README.md.
pub(crate) fn list_wave_items(dir: &Path) -> OpsResult<Vec<WaveItem>> {
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

        if let Some(item) = parse_wave_item_filename(&name) {
            items.push(item);
        }
    }

    items.sort_by(|left, right| {
        order_rank(left.order)
            .cmp(&order_rank(right.order))
            .then_with(|| left.filename.cmp(&right.filename))
    });
    Ok(items)
}

pub(crate) fn parse_wave_item_filename(filename: &str) -> Option<WaveItem> {
    let stem = filename.strip_suffix(".md")?;
    let dash_pos = stem.find('-')?;
    let prefix = &stem[..dash_pos];
    let slug = &stem[dash_pos + 1..];

    if slug.is_empty() {
        return None;
    }

    let order = if let Some(bucket) = PriorityBucket::from_filename_prefix(prefix) {
        WaveItemOrder::Bucket(bucket)
    } else {
        WaveItemOrder::LegacyStage(prefix.parse().ok()?)
    };

    Some(WaveItem {
        filename: filename.to_string(),
        order,
        slug: slug.to_string(),
    })
}

fn order_rank(order: WaveItemOrder) -> (u8, u32) {
    match order {
        WaveItemOrder::Bucket(bucket) => (0, bucket.order().into()),
        WaveItemOrder::LegacyStage(stage) => (1, stage),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::NullProgress;
    use tempfile::TempDir;

    #[test]
    fn parse_wave_item_filename_parses_bucketed_files() {
        let item = parse_wave_item_filename("2-setup.md").expect("bucketed item");
        assert_eq!(item.priority_bucket(), Some(PriorityBucket::High));
        assert_eq!(item.slug, "setup");
    }

    #[test]
    fn parse_wave_item_filename_parses_legacy_numbered_files() {
        let item = parse_wave_item_filename("02-mac-mini-dogfood.md").expect("legacy item");
        assert_eq!(item.order, WaveItemOrder::LegacyStage(2));
        assert_eq!(item.slug, "mac-mini-dogfood");
    }

    #[test]
    fn parse_wave_item_filename_rejects_readme() {
        assert!(parse_wave_item_filename("README.md").is_none());
    }

    #[test]
    fn parse_wave_item_filename_rejects_unknown_prefix() {
        assert!(parse_wave_item_filename("backlog-setup.md").is_none());
    }

    #[test]
    fn parse_wave_item_filename_rejects_no_slug() {
        assert!(parse_wave_item_filename("2-.md").is_none());
    }

    #[test]
    fn list_wave_items_filters_readme_and_sorts_bucketed_before_legacy() {
        let dir = TempDir::new().expect("temp dir");
        std::fs::write(dir.path().join("README.md"), "# Wave").expect("write readme");
        std::fs::write(dir.path().join("03-third.md"), "# Third").expect("write third");
        std::fs::write(dir.path().join("3-later.md"), "# Later").expect("write later");
        std::fs::write(dir.path().join("1-broken.md"), "# Broken").expect("write broken");
        std::fs::write(dir.path().join("notes.txt"), "ignored").expect("write notes");

        let items = list_wave_items(dir.path()).expect("list wave items");
        assert_eq!(
            items
                .iter()
                .map(|item| item.filename.as_str())
                .collect::<Vec<_>>(),
            vec!["1-broken.md", "3-later.md", "03-third.md"]
        );
    }

    #[test]
    fn ingest_prefers_bucketed_items() {
        let dir = TempDir::new().expect("temp dir");
        let repo = dir.path();

        std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo)
            .output()
            .expect("git init");

        let wave_dir = repo.join("wave").join("test-wave");
        std::fs::create_dir_all(&wave_dir).expect("create wave dir");
        std::fs::write(wave_dir.join("README.md"), "# Test").expect("write readme");
        std::fs::write(wave_dir.join("01-legacy.md"), "# Legacy").expect("write legacy");
        std::fs::write(wave_dir.join("2-next.md"), "# Next").expect("write high");
        std::fs::write(wave_dir.join("1-broken.md"), "# Broken").expect("write urgent");

        let result = ingest(
            repo,
            &IngestOptions {
                wave: Some("test-wave".to_string()),
            },
            &NullProgress,
        )
        .expect("ingest succeeds");

        assert_eq!(result.wave, "test-wave");
        assert_eq!(result.slug, "broken");
        assert!(result.dest.ends_with("scratch/test-wave-broken.md"));
        assert!(result.dest.exists());
        assert!(!wave_dir.join("1-broken.md").exists());
        assert!(wave_dir.join("2-next.md").exists());
        assert!(wave_dir.join("01-legacy.md").exists());
    }

    #[test]
    fn ingest_uses_filename_order_within_the_same_bucket() {
        let dir = TempDir::new().expect("temp dir");
        let repo = dir.path();

        std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo)
            .output()
            .expect("git init");

        let wave_dir = repo.join("wave").join("test-wave");
        std::fs::create_dir_all(&wave_dir).expect("create wave dir");
        std::fs::write(wave_dir.join("2-alpha.md"), "# Alpha").expect("write alpha");
        std::fs::write(wave_dir.join("2-beta.md"), "# Beta").expect("write beta");

        let result = ingest(
            repo,
            &IngestOptions {
                wave: Some("test-wave".to_string()),
            },
            &NullProgress,
        )
        .expect("ingest succeeds");

        assert_eq!(result.slug, "alpha");
        assert!(!wave_dir.join("2-alpha.md").exists());
        assert!(wave_dir.join("2-beta.md").exists());
    }

    #[test]
    fn ingest_empty_wave_errors() {
        let dir = TempDir::new().expect("temp dir");
        let repo = dir.path();

        std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo)
            .output()
            .expect("git init");

        let wave_dir = repo.join("wave").join("test-wave");
        std::fs::create_dir_all(&wave_dir).expect("create wave dir");

        let result = ingest(
            repo,
            &IngestOptions {
                wave: Some("test-wave".to_string()),
            },
            &NullProgress,
        );

        assert!(result.is_err());
        assert_eq!(
            result.expect_err("empty wave should error").to_string(),
            "no roadmap items in wave/test-wave/"
        );
    }
}
