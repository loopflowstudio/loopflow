//! The real hook entry reads current owned sources without launching a provider.
use crate::support;

use std::fs;
use std::path::Path;
use std::process::Command;

use loopflow::context_block::ContextDelivery;
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
        .args(["__context-block", "--delivery"])
        .arg(repo.join("delivery.json"))
        .args(["--moment", moment])
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
fn hook_refreshes_checkout_ancestors_and_scratch_without_replaying_the_request() {
    let _env = support::EnvGuard::new(&[]);
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    fs::create_dir_all(home.path().join("machine")).unwrap();
    let repo_path = repo.path().canonicalize().unwrap();
    fs::create_dir_all(repo.path().join(".lf/wave/parent/child")).unwrap();
    fs::write(
        repo.path().join(".lf/wave/parent/child/MEMORY.md"),
        "Current checkout memory",
    )
    .unwrap();
    fs::write(
        repo.path().join(".lf/wave/parent/MEMORY.md"),
        "Inherited checkout direction",
    )
    .unwrap();
    fs::create_dir(repo.path().join("scratch")).unwrap();
    fs::write(repo.path().join("scratch/plan.md"), "CURRENT_SCRATCH").unwrap();
    fs::write(repo.path().join("active.md"), "Saved active skill\n").unwrap();
    let delivery = ContextDelivery {
        repository: None,
        repo: repo_path,
        wave: Some("parent/child".into()),
        skill_file: Some(repo.path().join("active.md")),
        submitted_docs: None,
        references: Vec::new(),
        home: home.path().join("machine"),
    };
    fs::write(
        repo.path().join("delivery.json"),
        serde_json::to_vec(&delivery).unwrap(),
    )
    .unwrap();
    let start = hook(repo.path(), home.path(), "start");
    assert!(start.contains("Inherited checkout direction"));
    assert!(start.contains("Current checkout memory"));
    assert!(start.contains("CURRENT_SCRATCH"));
    assert!(!start.contains("Saved active skill\n"));

    fs::write(
        repo.path().join(".lf/wave/parent/MEMORY.md"),
        "Fresh inherited direction",
    )
    .unwrap();
    fs::write(
        repo.path().join(".lf/wave/parent/child/MEMORY.md"),
        "Fresh child memory",
    )
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
        "Inherited checkout direction",
        "CURRENT_SCRATCH",
        "Current checkout memory",
    ] {
        assert!(!compact.contains(stale));
    }
    // Wave listings point to the real checkout files, not generated snapshots.
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
    let manifest: Value =
        serde_json::from_slice(&fs::read(repo.path().join(path)).unwrap()).unwrap();
    let wave_files: Vec<_> = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|file| file["source"].as_str().unwrap().starts_with(".lf/wave/"))
        .collect();
    assert_eq!(wave_files.len(), 2);
    for file in wave_files {
        let path = Path::new(file["path"].as_str().unwrap());
        assert_eq!(
            path,
            repo.path()
                .canonicalize()
                .unwrap()
                .join(file["source"].as_str().unwrap())
        );
        assert_eq!(
            fs::read(path).unwrap().len() as u64,
            file["bytes"].as_u64().unwrap()
        );
    }
}

#[test]
fn hook_after_repository_move_reads_retained_skill_and_current_scratch() {
    let _env = support::EnvGuard::new(&[]);
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let old = repo.path().canonicalize().unwrap();
    let store = loopflow::store::sqlite::SqliteStore::new(&home.path().join("machine/loopflow.db"))
        .unwrap();
    let id = store.ensure_repository(old.to_str().unwrap()).unwrap();
    fs::create_dir(old.join("scratch")).unwrap();
    fs::write(old.join("scratch/plan.md"), "RETAINED_SCRATCH").unwrap();
    fs::write(old.join("active.md"), "RETAINED_SKILL").unwrap();
    let delivery = ContextDelivery {
        repository: Some((id.clone(), old.clone())),
        repo: old.clone(),
        wave: None,
        skill_file: Some(old.join("active.md")),
        submitted_docs: None,
        references: Vec::new(),
        home: home.path().join("machine"),
    };
    fs::write(
        old.join("delivery.json"),
        serde_json::to_vec(&delivery).unwrap(),
    )
    .unwrap();
    let root = tempfile::tempdir().unwrap();
    let moved = root.path().join("moved");
    fs::rename(&old, &moved).unwrap();
    let compact = hook(&moved, home.path(), "compact");
    assert!(compact.contains("RETAINED_SCRATCH"));
    assert!(compact.contains("RETAINED_SKILL"));
    assert_eq!(
        store.repository_path(&id).unwrap().unwrap(),
        moved.canonicalize().unwrap().to_str().unwrap()
    );
}
