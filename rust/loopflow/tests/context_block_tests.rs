//! The real hook entry reads current owned sources without launching a provider.
mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use loopflow::store::sqlite::SqliteStore;
use loopflow_test_support::TestRepo;
use serde_json::Value;

fn hook(repo: &Path, home: &Path, moment: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", home)
        .env("LF_HOME", home.join("machine"))
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .current_dir(repo)
        .args(["--wave", "parent/child", "__context-block", "--repo"])
        .arg(repo)
        .args(["--moment", moment, "--skill-file", "active.md"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        output["hookSpecificOutput"]["hookEventName"],
        "SessionStart"
    );
    let text = output["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(text.len() <= 10_000);
    text.to_owned()
}

#[test]
fn hook_refreshes_sqlite_ancestors_and_scratch_without_replaying_the_request() {
    let _env = support::EnvGuard::new(&[]);
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    fs::create_dir_all(home.path().join("machine")).unwrap();
    let store = SqliteStore::new(&home.path().join("machine/loopflow.db")).unwrap();
    let repo_path = repo.path().canonicalize().unwrap();
    let parent = store
        .ensure_wave(repo_path.to_str().unwrap(), "parent")
        .unwrap();
    let child = store
        .ensure_wave(repo_path.to_str().unwrap(), "parent/child")
        .unwrap();
    store
        .update_wave_document(&parent, "MEMORY.md", "Inherited saved direction")
        .unwrap();
    store
        .update_wave_document(&child, "MEMORY.md", "Current saved memory")
        .unwrap();
    fs::create_dir_all(repo.path().join("wave/parent/child")).unwrap();
    fs::write(
        repo.path().join("wave/parent/child/MEMORY.md"),
        "STALE CHECKOUT",
    )
    .unwrap();
    fs::create_dir(repo.path().join("scratch")).unwrap();
    fs::write(repo.path().join("scratch/plan.md"), "CURRENT_SCRATCH").unwrap();
    fs::write(repo.path().join("active.md"), "Saved active skill\n").unwrap();
    let start = hook(repo.path(), home.path(), "start");
    assert!(start.contains("Inherited saved direction"));
    assert!(start.contains("Current saved memory"));
    assert!(start.contains("CURRENT_SCRATCH"));
    assert!(!start.contains("STALE CHECKOUT"));
    assert!(!start.contains("Saved active skill\n"));

    store
        .update_wave_document(&parent, "MEMORY.md", "Fresh inherited direction")
        .unwrap();
    store
        .update_wave_document(&child, "MEMORY.md", "Fresh child memory")
        .unwrap();
    fs::write(repo.path().join("scratch/plan.md"), "FRESH_SCRATCH").unwrap();
    let compact = hook(repo.path(), home.path(), "compact");
    for fresh in [
        "Fresh inherited direction",
        "Fresh child memory",
        "FRESH_SCRATCH",
        "Saved active skill\n",
    ] {
        assert!(compact.contains(fresh), "missing {fresh}: {compact}");
    }
    for stale in [
        "Inherited saved direction",
        "Current saved memory",
        "CURRENT_SCRATCH",
        "STALE CHECKOUT",
    ] {
        assert!(!compact.contains(stale));
    }
    // Logical Wave paths have complete, private snapshots with the saved bytes.
    let marker = "Read the complete context listing and any files not preloaded below: ";
    let path = compact
        .split_once(marker)
        .unwrap()
        .1
        .lines()
        .next()
        .unwrap()
        .trim_end_matches('.');
    let path: String = serde_json::from_str(path).unwrap();
    let manifest: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    let snapshots: Vec<_> = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|file| file["source"].as_str().unwrap().starts_with("wave/"))
        .map(|file| fs::read_to_string(file["path"].as_str().unwrap()).unwrap())
        .collect();
    assert!(snapshots
        .iter()
        .any(|text| text == "Fresh inherited direction"));
    assert!(snapshots.iter().any(|text| text == "Fresh child memory"));
}
