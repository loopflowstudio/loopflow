use std::path::{Path, PathBuf};

use crate::engine::git::current_branch;
use crate::engine::worktrees::main_repo_root;
use crate::lfd::pm::{PriorityBucket, RoadmapItemDocument};
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::pm::{pm_pull, pm_try_claim, wave_pm_is_enabled, PmPullOptions};
use crate::ops::progress::Progress;
use crate::ops::util::resolve_wave_name;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

const LF_RUN_ID_ENV: &str = "LF_RUN_ID";

#[derive(Debug, Clone, Default)]
pub struct IngestOptions {
    pub wave: Option<String>,
    pub item: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IngestResult {
    pub wave: String,
    pub slug: String,
    pub dest: PathBuf,
}

/// Pick the highest-priority roadmap item and move it to scratch/.
///
/// When PM is enabled for the wave, refresh the local wave mirror from the
/// provider before selecting an item. Without PM, falls back to local file
/// ordering.
pub fn ingest(
    repo: &Path,
    options: &IngestOptions,
    progress: &impl Progress,
) -> OpsResult<IngestResult> {
    let wave = resolve_wave_name(repo, options.wave.as_deref())
        .ok_or_else(|| OpsError::Message("cannot determine wave name".to_string()))?;

    let main_repo = main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());

    let pm_enabled = wave_pm_is_enabled(&main_repo, &wave);
    let claimed_filename = if pm_enabled {
        try_claim_pm_item(&main_repo, &wave, progress)
    } else {
        None
    };

    if pm_enabled {
        if let Err(err) = refresh_wave_from_pm(&main_repo, &wave, progress) {
            progress.warning(&format!(
                "warning: failed to pull PM items for wave/{wave}: {err}"
            ));
        }
    }

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

    let item = select_ingest_item(
        &items,
        options.item.as_deref(),
        claimed_filename.as_deref(),
        progress,
    )?;
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
    write_claim_status(&dest, repo)?;

    Ok(IngestResult {
        wave,
        slug: item.slug.clone(),
        dest,
    })
}

fn refresh_wave_from_pm(repo: &Path, wave: &str, progress: &impl Progress) -> OpsResult<()> {
    #[cfg(test)]
    if let Some(hook) = test_pm_hooks().lock().expect("pm hooks lock").pull {
        return hook(repo, wave, progress);
    }

    pm_pull(
        repo,
        &PmPullOptions {
            wave: wave.to_string(),
        },
        progress,
    )
    .map(|_| ())
}

#[cfg(test)]
type TestPmPullHook = fn(&Path, &str, &dyn Progress) -> OpsResult<()>;

#[cfg(test)]
type TestPmClaimHook = fn(&Path, &str, &dyn Progress) -> Option<String>;

#[cfg(test)]
#[derive(Default)]
struct TestPmHooks {
    pull: Option<TestPmPullHook>,
    claim: Option<TestPmClaimHook>,
}

#[cfg(test)]
fn test_pm_hooks() -> &'static std::sync::Mutex<TestPmHooks> {
    static TEST_PM_HOOKS: std::sync::OnceLock<std::sync::Mutex<TestPmHooks>> =
        std::sync::OnceLock::new();
    TEST_PM_HOOKS.get_or_init(|| std::sync::Mutex::new(TestPmHooks::default()))
}

fn try_claim_pm_item(repo: &Path, wave: &str, progress: &impl Progress) -> Option<String> {
    #[cfg(test)]
    if let Some(hook) = test_pm_hooks().lock().expect("pm hooks lock").claim {
        return hook(repo, wave, progress);
    }

    pm_try_claim(repo, wave, progress)
}

fn select_ingest_item<'a>(
    items: &'a [WaveItem],
    requested: Option<&str>,
    claimed: Option<&str>,
    progress: &impl Progress,
) -> OpsResult<&'a WaveItem> {
    if let Some(requested) = requested {
        return select_wave_item(items, Some(requested));
    }

    if let Some(claimed) = claimed {
        match select_wave_item(items, Some(claimed)) {
            Ok(item) => return Ok(item),
            Err(_) => progress.warning(&format!(
                "warning: claimed roadmap item no longer exists locally: {claimed}"
            )),
        }
    }

    select_wave_item(items, None)
}

fn write_claim_status(path: &Path, repo: &Path) -> OpsResult<()> {
    let content = std::fs::read_to_string(path)?;
    let mut document =
        RoadmapItemDocument::parse(&content).map_err(|err| OpsError::Message(err.to_string()))?;
    document
        .frontmatter
        .mark_in_progress(claimed_by(repo), claimed_at());
    let rendered = document
        .render()
        .map_err(|err| OpsError::Message(err.to_string()))?;
    std::fs::write(path, rendered)?;
    Ok(())
}

fn claimed_by(repo: &Path) -> String {
    std::env::var(LF_RUN_ID_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| current_branch(repo).ok().flatten())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "manual".to_string())
}

fn claimed_at() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("rfc3339 timestamp should format")
}

fn select_wave_item<'a>(items: &'a [WaveItem], requested: Option<&str>) -> OpsResult<&'a WaveItem> {
    let Some(requested) = requested.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(items.first().expect("items is non-empty"));
    };

    let requested_stem = requested.strip_suffix(".md").unwrap_or(requested);
    items
        .iter()
        .find(|item| item.matches_request(requested, requested_stem))
        .ok_or_else(|| OpsError::Message(format!("roadmap item not found: {requested}")))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum WaveItemOrder {
    Frontmatter {
        priority: PriorityBucket,
        rank: Option<f64>,
    },
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
    pub(crate) fn rank(&self) -> u32 {
        match self.order {
            WaveItemOrder::Frontmatter { priority, .. } => priority.rank(),
            WaveItemOrder::Bucket(bucket) => bucket.rank(),
            WaveItemOrder::LegacyStage(stage) => stage.saturating_sub(1),
        }
    }

    fn filename_stem(&self) -> &str {
        self.filename.strip_suffix(".md").unwrap_or(&self.filename)
    }

    fn matches_request(&self, requested: &str, requested_stem: &str) -> bool {
        self.filename == requested
            || self.slug == requested
            || self.filename_stem() == requested_stem
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

        let path = entry.path();
        let content = std::fs::read_to_string(&path)?;
        if let Some(item) = parse_wave_item(&name, &content) {
            items.push(item);
        }
    }

    items.sort_by(|left, right| {
        compare_wave_item_order(left.order, right.order)
            .then_with(|| left.filename.cmp(&right.filename))
    });
    Ok(items)
}

#[cfg(test)]
pub(crate) fn parse_wave_item_filename(filename: &str) -> Option<WaveItem> {
    parse_wave_item(filename, "")
}

fn parse_wave_item(filename: &str, content: &str) -> Option<WaveItem> {
    let (slug, legacy_order) = parse_filename_parts(filename)?;
    let frontmatter = RoadmapItemDocument::parse(content).ok()?.frontmatter;
    let order = if let Some(priority) = frontmatter.priority {
        WaveItemOrder::Frontmatter {
            priority,
            rank: frontmatter.rank.filter(|value| value.is_finite()),
        }
    } else {
        legacy_order?
    };

    Some(WaveItem {
        filename: filename.to_string(),
        order,
        slug,
    })
}

fn parse_filename_parts(filename: &str) -> Option<(String, Option<WaveItemOrder>)> {
    let stem = filename.strip_suffix(".md")?;
    if let Some((prefix, slug)) = stem.split_once('-') {
        if slug.is_empty() {
            return None;
        }

        let legacy_order = if let Some(bucket) = PriorityBucket::from_filename_prefix(prefix) {
            Some(WaveItemOrder::Bucket(bucket))
        } else {
            prefix.parse().ok().map(WaveItemOrder::LegacyStage)
        };

        if legacy_order.is_some() {
            return Some((slug.to_string(), legacy_order));
        }
    }

    Some((stem.to_string(), None))
}

fn compare_wave_item_order(left: WaveItemOrder, right: WaveItemOrder) -> std::cmp::Ordering {
    order_group(left)
        .cmp(&order_group(right))
        .then_with(|| match (left, right) {
            (
                WaveItemOrder::Frontmatter {
                    priority: left_priority,
                    rank: left_rank,
                },
                WaveItemOrder::Frontmatter {
                    priority: right_priority,
                    rank: right_rank,
                },
            ) => compare_bucket_rank(left_priority, left_rank, right_priority, right_rank),
            (
                WaveItemOrder::Frontmatter {
                    priority: left_priority,
                    rank: left_rank,
                },
                WaveItemOrder::Bucket(right_priority),
            ) => compare_bucket_rank(left_priority, left_rank, right_priority, None),
            (
                WaveItemOrder::Bucket(left_priority),
                WaveItemOrder::Frontmatter {
                    priority: right_priority,
                    rank: right_rank,
                },
            ) => compare_bucket_rank(left_priority, None, right_priority, right_rank),
            (WaveItemOrder::Bucket(left_bucket), WaveItemOrder::Bucket(right_bucket)) => {
                left_bucket.order().cmp(&right_bucket.order())
            }
            (WaveItemOrder::LegacyStage(left_stage), WaveItemOrder::LegacyStage(right_stage)) => {
                left_stage.cmp(&right_stage)
            }
            _ => std::cmp::Ordering::Equal,
        })
}

fn order_group(order: WaveItemOrder) -> u8 {
    match order {
        WaveItemOrder::Frontmatter { .. } | WaveItemOrder::Bucket(_) => 0,
        WaveItemOrder::LegacyStage(_) => 1,
    }
}

fn compare_bucket_rank(
    left_priority: PriorityBucket,
    left_rank: Option<f64>,
    right_priority: PriorityBucket,
    right_rank: Option<f64>,
) -> std::cmp::Ordering {
    left_priority
        .order()
        .cmp(&right_priority.order())
        .then_with(|| match (left_rank, right_rank) {
            (Some(left), Some(right)) => left
                .partial_cmp(&right)
                .unwrap_or(std::cmp::Ordering::Equal),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::pm::RoadmapItemDocument;
    use crate::ops::NullProgress;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Mutex, OnceLock};
    use tempfile::TempDir;

    fn set_test_pm_hooks(pull: Option<TestPmPullHook>, claim: Option<TestPmClaimHook>) {
        let mut hooks = test_pm_hooks().lock().expect("pm hooks lock");
        hooks.pull = pull;
        hooks.claim = claim;
    }

    fn pm_test_lock() -> &'static Mutex<()> {
        static PM_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        PM_TEST_LOCK.get_or_init(|| Mutex::new(()))
    }

    struct TestPmPullGuard {
        _guard: std::sync::MutexGuard<'static, ()>,
    }

    impl TestPmPullGuard {
        fn install(hook: TestPmPullHook) -> Self {
            let guard = pm_test_lock().lock().expect("pm test lock");
            set_test_pm_hooks(Some(hook), None);
            Self { _guard: guard }
        }

        fn install_with_claim(hook: TestPmPullHook, claim_hook: TestPmClaimHook) -> Self {
            let guard = pm_test_lock().lock().expect("pm test lock");
            set_test_pm_hooks(Some(hook), Some(claim_hook));
            Self { _guard: guard }
        }
    }

    impl Drop for TestPmPullGuard {
        fn drop(&mut self) {
            set_test_pm_hooks(None, None);
        }
    }

    fn write_pm_config(repo: &Path) {
        std::fs::create_dir_all(repo.join(".lf")).expect("create lf dir");

        let wave_dir = repo.join("wave").join("test-wave");
        std::fs::write(
            wave_dir.join("test-wave.yaml"),
            "flow: build\npm:\n  asana_project: \"asa-1\"\n",
        )
        .expect("write wave pm config");
    }

    fn refresh_from_pm_fixture(repo: &Path, wave: &str, _progress: &dyn Progress) -> OpsResult<()> {
        PM_PULL_CALLS.fetch_add(1, Ordering::SeqCst);
        let wave_dir = repo.join("wave").join(wave);
        for entry in std::fs::read_dir(&wave_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "md")
                && path.file_name().and_then(|name| name.to_str()) != Some("README.md")
            {
                std::fs::remove_file(path)?;
            }
        }
        write_wave_file(&wave_dir, "1-fresh.md", "# Fresh from PM");
        write_wave_file(&wave_dir, "2-later.md", "# Later");
        Ok(())
    }

    fn fail_pm_pull(_repo: &Path, _wave: &str, _progress: &dyn Progress) -> OpsResult<()> {
        Err(OpsError::Message("pm unavailable".to_string()))
    }

    fn claim_second_item(_repo: &Path, _wave: &str, _progress: &dyn Progress) -> Option<String> {
        Some("2-claimed.md".to_string())
    }

    static PM_PULL_CALLS: AtomicUsize = AtomicUsize::new(0);

    #[derive(Debug, Default)]
    struct RecordingProgress {
        warnings: Mutex<Vec<String>>,
    }

    impl Progress for RecordingProgress {
        fn status(&self, _msg: &str) {}

        fn error(&self, _msg: &str) {}

        fn warning(&self, msg: &str) {
            self.warnings
                .lock()
                .expect("warnings lock")
                .push(msg.to_string());
        }

        fn confirm(&self, _msg: &str) -> bool {
            true
        }
    }

    fn init_test_wave() -> (TempDir, PathBuf) {
        let dir = TempDir::new().expect("temp dir");
        std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(dir.path())
            .output()
            .expect("git init");

        let wave_dir = dir.path().join("wave").join("test-wave");
        std::fs::create_dir_all(&wave_dir).expect("create wave dir");
        (dir, wave_dir)
    }

    fn write_wave_file(wave_dir: &Path, name: &str, contents: &str) {
        std::fs::write(wave_dir.join(name), contents).expect("write wave file");
    }

    #[test]
    fn parse_wave_item_filename_parses_bucketed_files() {
        let item = parse_wave_item_filename("2-setup.md").expect("bucketed item");
        assert_eq!(item.order, WaveItemOrder::Bucket(PriorityBucket::High));
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
    fn list_wave_items_uses_frontmatter_priority_and_rank() {
        let dir = TempDir::new().expect("temp dir");
        std::fs::write(
            dir.path().join("alpha.md"),
            "---\npriority: high\nrank: 0.8\n---\n# Alpha\n",
        )
        .expect("write alpha");
        std::fs::write(
            dir.path().join("beta.md"),
            "---\npriority: high\nrank: 0.2\n---\n# Beta\n",
        )
        .expect("write beta");
        std::fs::write(
            dir.path().join("gamma.md"),
            "---\npriority: low\n---\n# Gamma\n",
        )
        .expect("write gamma");

        let items = list_wave_items(dir.path()).expect("list wave items");
        assert_eq!(
            items
                .iter()
                .map(|item| item.filename.as_str())
                .collect::<Vec<_>>(),
            vec!["beta.md", "alpha.md", "gamma.md"]
        );
    }

    #[test]
    fn ingest_prefers_bucketed_items() {
        let (dir, wave_dir) = init_test_wave();
        let repo = dir.path();

        write_wave_file(&wave_dir, "README.md", "# Test");
        write_wave_file(&wave_dir, "01-legacy.md", "# Legacy");
        write_wave_file(&wave_dir, "2-next.md", "# Next");
        write_wave_file(&wave_dir, "1-broken.md", "# Broken");

        let result = ingest(
            repo,
            &IngestOptions {
                wave: Some("test-wave".to_string()),
                item: None,
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
        let document = RoadmapItemDocument::parse(
            &std::fs::read_to_string(&result.dest).expect("read scratch item"),
        )
        .expect("parse scratch item");
        assert_eq!(document.frontmatter.status.as_deref(), Some("in-progress"));
        assert_eq!(document.frontmatter.claimed_by.as_deref(), Some("main"));
        assert!(document.frontmatter.claimed_at.is_some());
    }

    #[test]
    fn ingest_uses_filename_order_within_the_same_bucket() {
        let (dir, wave_dir) = init_test_wave();
        let repo = dir.path();

        write_wave_file(&wave_dir, "2-alpha.md", "# Alpha");
        write_wave_file(&wave_dir, "2-beta.md", "# Beta");

        let result = ingest(
            repo,
            &IngestOptions {
                wave: Some("test-wave".to_string()),
                item: None,
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
        let (dir, _) = init_test_wave();
        let repo = dir.path();

        let result = ingest(
            repo,
            &IngestOptions {
                wave: Some("test-wave".to_string()),
                item: None,
            },
            &NullProgress,
        );

        assert!(result.is_err());
        assert_eq!(
            result.expect_err("empty wave should error").to_string(),
            "no roadmap items in wave/test-wave/"
        );
    }

    #[test]
    fn ingest_accepts_targeted_filename() {
        let (dir, wave_dir) = init_test_wave();
        let repo = dir.path();

        write_wave_file(&wave_dir, "1-alpha.md", "# Alpha");
        write_wave_file(&wave_dir, "4-beta.md", "# Beta");

        let result = ingest(
            repo,
            &IngestOptions {
                wave: Some("test-wave".to_string()),
                item: Some("4-beta.md".to_string()),
            },
            &NullProgress,
        )
        .expect("targeted ingest succeeds");

        assert_eq!(result.slug, "beta");
        assert!(!wave_dir.join("4-beta.md").exists());
        assert!(wave_dir.join("1-alpha.md").exists());
    }

    #[test]
    fn ingest_accepts_targeted_slug() {
        let (dir, wave_dir) = init_test_wave();
        let repo = dir.path();

        write_wave_file(&wave_dir, "1-alpha.md", "# Alpha");
        write_wave_file(&wave_dir, "4-beta.md", "# Beta");

        let result = ingest(
            repo,
            &IngestOptions {
                wave: Some("test-wave".to_string()),
                item: Some("beta".to_string()),
            },
            &NullProgress,
        )
        .expect("targeted ingest succeeds");

        assert_eq!(result.slug, "beta");
        assert!(!wave_dir.join("4-beta.md").exists());
        assert!(wave_dir.join("1-alpha.md").exists());
    }

    #[test]
    fn ingest_errors_when_targeted_item_is_missing() {
        let (dir, wave_dir) = init_test_wave();
        let repo = dir.path();

        write_wave_file(&wave_dir, "1-alpha.md", "# Alpha");

        let result = ingest(
            repo,
            &IngestOptions {
                wave: Some("test-wave".to_string()),
                item: Some("beta".to_string()),
            },
            &NullProgress,
        );

        assert_eq!(
            result
                .expect_err("missing roadmap item should error")
                .to_string(),
            "roadmap item not found: beta"
        );
    }

    #[test]
    fn ingest_refreshes_pm_backed_waves_before_picking_an_item() {
        let _guard = TestPmPullGuard::install(refresh_from_pm_fixture);
        let (dir, wave_dir) = init_test_wave();
        let repo = dir.path();
        write_pm_config(repo);
        write_wave_file(&wave_dir, "2-stale.md", "# Stale local item");

        PM_PULL_CALLS.store(0, Ordering::SeqCst);

        let result = ingest(
            repo,
            &IngestOptions {
                wave: Some("test-wave".to_string()),
                item: None,
            },
            &NullProgress,
        )
        .expect("ingest succeeds");

        assert_eq!(PM_PULL_CALLS.load(Ordering::SeqCst), 1);
        assert_eq!(result.slug, "fresh");
        assert!(result.dest.ends_with("scratch/test-wave-fresh.md"));
        assert!(!wave_dir.join("1-fresh.md").exists());
        assert!(wave_dir.join("2-later.md").exists());
        assert!(!wave_dir.join("2-stale.md").exists());
    }

    #[test]
    fn ingest_prefers_pm_claimed_item_over_local_priority() {
        let _guard = TestPmPullGuard::install_with_claim(fail_pm_pull, claim_second_item);
        let (dir, wave_dir) = init_test_wave();
        let repo = dir.path();
        write_pm_config(repo);
        write_wave_file(&wave_dir, "1-first.md", "# First");
        write_wave_file(&wave_dir, "2-claimed.md", "# Claimed");

        let result = ingest(
            repo,
            &IngestOptions {
                wave: Some("test-wave".to_string()),
                item: None,
            },
            &NullProgress,
        )
        .expect("ingest succeeds");

        assert_eq!(result.slug, "claimed");
        assert!(!wave_dir.join("2-claimed.md").exists());
        assert!(wave_dir.join("1-first.md").exists());
    }

    #[test]
    fn ingest_warns_when_pm_pull_fails_and_continues_with_local_items() {
        let _guard = TestPmPullGuard::install(fail_pm_pull);
        let (dir, wave_dir) = init_test_wave();
        let repo = dir.path();
        write_pm_config(repo);
        write_wave_file(&wave_dir, "2-stale.md", "# Stale local item");
        let progress = RecordingProgress::default();

        let result = ingest(
            repo,
            &IngestOptions {
                wave: Some("test-wave".to_string()),
                item: None,
            },
            &progress,
        )
        .expect("ingest succeeds");

        assert_eq!(result.slug, "stale");
        assert_eq!(
            progress.warnings.lock().expect("warnings lock").as_slice(),
            ["warning: failed to pull PM items for wave/test-wave: pm unavailable"]
        );
    }
}
