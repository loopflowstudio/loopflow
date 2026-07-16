//! Local ops telemetry — the single owned source of truth for the path and
//! writer that records operational metrics (`lf rebase`, `lf wt create`, …).
//!
//! Telemetry lives under the git-ignored `.lf/tmp/metrics/ops.jsonl` tree so
//! read-only operations never dirty a tracked worktree. The previous design
//! appended to a *tracked* `.lf/metrics/ops.jsonl`; that made every read-only
//! op dirty the checkout, which then made dispatch refuse the "previously
//! clean" checkout and broke `lf task run` on the dogfood loop. The path
//! contract here — ignored tree, never tracked — is guarded by tests below so
//! a regression back to a tracked path fails the build.

use std::io::Write;
use std::path::{Path, PathBuf};

/// Where ops telemetry is recorded: `.lf/tmp/metrics/ops.jsonl` under the
/// worktree. `.lf/tmp/` is git-ignored, so recording a metric never touches a
/// tracked file. One source of truth for the path.
pub(crate) fn ops_metrics_path(repo: &Path) -> PathBuf {
    repo.join(".lf")
        .join("tmp")
        .join("metrics")
        .join("ops.jsonl")
}

/// Best-effort append of one ops-metric event. Never fails the caller: a
/// telemetry write that errors is dropped, not propagated, so an op never
/// breaks because its metric couldn't land. A `ts` is stamped on every event.
pub(crate) fn record_ops_metric(repo: &Path, mut event: serde_json::Value) {
    let Some(object) = event.as_object_mut() else {
        return;
    };
    object.insert(
        "ts".to_string(),
        serde_json::Value::String(chrono::Utc::now().to_rfc3339()),
    );
    let path = ops_metrics_path(repo);
    let Some(parent) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    else {
        return;
    };
    if serde_json::to_writer(&mut file, &event).is_ok() {
        let _ = writeln!(file);
    }
}

#[cfg(test)]
mod tests {
    use super::{ops_metrics_path, record_ops_metric};
    use std::path::Path;
    use std::process::Command;

    fn git(repo: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .args(args)
            .current_dir(repo)
            .status()
            .expect("git runs")
            .success();
        assert!(ok, "git {args:?} failed");
    }

    fn porcelain(repo: &Path) -> String {
        let out = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(repo)
            .output()
            .expect("git status runs");
        String::from_utf8(out.stdout).expect("utf8")
    }

    fn ignored(repo: &Path, path: &Path) -> bool {
        Command::new("git")
            .args(["check-ignore", "--quiet", "--"])
            .arg(path)
            .current_dir(repo)
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    fn clean_fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = dir.path();
        git(repo, &["init", "-q"]);
        git(repo, &["config", "user.email", "test@example.com"]);
        git(repo, &["config", "user.name", "Test"]);
        // The ignored-path contract: `.lf/tmp/` is never committed.
        std::fs::write(repo.join(".gitignore"), ".lf/tmp/\n").expect("write .gitignore");
        git(repo, &["add", "."]);
        git(repo, &["commit", "-qm", "init"]);
        assert_eq!(porcelain(repo), "", "fixture should start clean");
        dir
    }

    /// The path contract: telemetry targets the git-ignored `.lf/tmp/` tree and
    /// never the legacy tracked `.lf/metrics/` path that used to dirty checkouts.
    /// A change that moves the path back under a tracked directory fails here
    /// before it can reach production.
    #[test]
    fn ops_metrics_path_targets_ignored_tree_not_legacy_tracked_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = dir.path();
        let path = ops_metrics_path(repo);

        assert!(
            path.starts_with(repo.join(".lf").join("tmp")),
            "telemetry must live under the ignored .lf/tmp/ tree, got {}",
            path.display()
        );
        assert!(
            !path.starts_with(repo.join(".lf").join("metrics")),
            "telemetry must never target the legacy tracked .lf/metrics/ path, got {}",
            path.display()
        );
        assert_eq!(
            path.file_name(),
            Some(std::ffi::OsStr::new("ops.jsonl")),
            "telemetry file name is stable"
        );
    }

    /// The dogfood regression: recording ops telemetry into a clean checkout
    /// must leave it clean. The tracked-path bug (`.lf/metrics/ops.jsonl`) made
    /// every read-only op dirty the worktree and blocked `lf task run`; the fix
    /// routes telemetry under the git-ignored `.lf/tmp/` tree.
    #[test]
    fn recording_telemetry_leaves_a_clean_checkout() {
        let dir = clean_fixture();
        let repo = dir.path();

        record_ops_metric(repo, serde_json::json!({ "op": "rebase", "class": "noop" }));

        // Telemetry is not disabled: the record lands on disk...
        let path = ops_metrics_path(repo);
        assert!(path.exists(), "telemetry file must be written");
        // ...and git reports the path as ignored, so it cannot dirty the tree...
        assert!(
            ignored(repo, &path),
            "telemetry path {} must be git-ignored",
            path.display()
        );
        // ...so the checkout stays clean and dispatch/rebase/status never refuse.
        assert_eq!(
            porcelain(repo),
            "",
            "recording telemetry must not dirty a clean worktree"
        );
    }

    /// A malformed (non-object) event is dropped silently rather than panicking
    /// or writing a torn line — telemetry never breaks the op that emits it.
    #[test]
    fn non_object_event_is_dropped_not_written() {
        let dir = clean_fixture();
        let repo = dir.path();

        record_ops_metric(repo, serde_json::json!(["not", "an", "object"]));

        let path = ops_metrics_path(repo);
        assert!(
            !path.exists(),
            "a non-object event must not create the file"
        );
        assert_eq!(porcelain(repo), "", "dropped event must not dirty the tree");
    }
}
