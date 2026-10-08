use std::fs;
use std::path::Path;
use std::process::Command;

use loopflow::engine::planning_git::{PlanningGit, PlanningGitError, PlanningPublication};
use loopflow_test_support::TestRepo;

fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(repo)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}

fn transport(repo: &Path) -> PlanningGit {
    PlanningGit::new(repo, "origin").unwrap()
}

fn source_state(repo: &Path) -> Vec<Vec<u8>> {
    let status = git(repo, &["status", "--porcelain=v1"]);
    let git_dir = git(repo, &["rev-parse", "--absolute-git-dir"]);
    vec![
        git(repo, &["rev-parse", "HEAD"]).into_bytes(),
        git(repo, &["symbolic-ref", "HEAD"]).into_bytes(),
        fs::read(Path::new(&git_dir).join("index")).unwrap(),
        fs::read(Path::new(&git_dir).join("FETCH_HEAD")).unwrap_or_default(),
        fs::read(repo.join("staged.txt")).unwrap(),
        fs::read(repo.join("unstaged.txt")).unwrap(),
        fs::read(repo.join("untracked.txt")).unwrap(),
        status.into_bytes(),
    ]
}

fn dirty(repo: &Path) {
    fs::write(repo.join("staged.txt"), "staged").unwrap();
    fs::write(repo.join("unstaged.txt"), "original").unwrap();
    git(repo, &["add", "staged.txt", "unstaged.txt"]);
    fs::write(repo.join("unstaged.txt"), "dirty").unwrap();
    fs::write(repo.join("untracked.txt"), "untracked").unwrap();
}

#[test]
fn concurrent_publication_preserves_both_revisions_and_all_source_bytes() {
    let first = TestRepo::new();
    first.push();
    let second = tempfile::tempdir().unwrap();
    let origin = git(first.path(), &["remote", "get-url", "origin"]);
    git(second.path(), &["clone", &origin, "."]);
    dirty(first.path());
    dirty(second.path());
    let first_before = source_state(first.path());
    let second_before = source_state(second.path());
    let laptop = transport(first.path());
    let worker = transport(second.path());
    assert_eq!(laptop.fetch().unwrap(), None);
    let initial = laptop.save(br#"{"comments":[]}"#, None, None).unwrap();
    assert_eq!(
        laptop.publish(&initial.revision).unwrap(),
        PlanningPublication::Confirmed
    );
    let received = worker.fetch().unwrap().unwrap();
    assert_eq!(initial, received);
    let base = worker
        .save(&received.bytes, None, Some(&received.revision))
        .unwrap();
    let left = laptop
        .save(br#"{"comments":["laptop"]}"#, Some(&initial.revision), None)
        .unwrap();
    let right = worker
        .save(br#"{"comments":["worker"]}"#, Some(&base.revision), None)
        .unwrap();
    assert_eq!(
        laptop.publish(&left.revision).unwrap(),
        PlanningPublication::Confirmed
    );
    let PlanningPublication::Pending {
        remote: Some(observed),
    } = worker.publish(&right.revision).unwrap()
    else {
        panic!("a stale push must preserve the remote's concurrent update");
    };
    assert_eq!(observed, left);
    assert_eq!(worker.local().unwrap(), Some(right.clone()));
    // Reconciliation is supplied by the exchange layer, never inferred by transport.
    let merged = worker
        .save(
            br#"{"comments":["laptop","worker"]}"#,
            Some(&right.revision),
            Some(&observed.revision),
        )
        .unwrap();
    assert!(worker
        .is_ancestor(&right.revision, &merged.revision)
        .unwrap());
    assert!(worker
        .is_ancestor(&left.revision, &merged.revision)
        .unwrap());
    assert_eq!(
        worker.publish(&merged.revision).unwrap(),
        PlanningPublication::Confirmed
    );
    assert_eq!(laptop.fetch().unwrap(), Some(merged));
    assert_eq!(first_before, source_state(first.path()));
    assert_eq!(second_before, source_state(second.path()));
}

#[test]
fn offline_changes_survive_and_fresh_clone_readback_recovers_lost_acknowledgement() {
    let repo = TestRepo::new();
    repo.push();
    let online = transport(repo.path());
    let remote = git(repo.path(), &["remote", "get-url", "origin"]);
    let saved = online
        .save(
            br#"{"task":"stable-id","comment":"stable-comment"}"#,
            None,
            None,
        )
        .unwrap();
    let offline = PlanningGit::new(repo.path(), "/nonexistent-loopflow-planning-fixture").unwrap();
    assert_eq!(
        offline.publish(&saved.revision).unwrap(),
        PlanningPublication::Unconfirmed
    );
    assert_eq!(offline.local().unwrap(), Some(saved.clone()));
    // A successful publication whose response the caller discards.
    online.publish(&saved.revision).unwrap();
    let tip = git(
        repo.path(),
        &["ls-remote", "origin", "refs/loopflow/planning"],
    );
    let fresh = tempfile::tempdir().unwrap();
    git(fresh.path(), &["clone", &remote, "."]);
    let recovered = transport(fresh.path());
    assert_eq!(recovered.fetch().unwrap(), Some(saved.clone()));
    assert_eq!(
        recovered.confirm(&saved.revision).unwrap(),
        PlanningPublication::Confirmed
    );
    assert_eq!(
        tip,
        git(
            repo.path(),
            &["ls-remote", "origin", "refs/loopflow/planning"]
        )
    );
}

#[test]
fn local_compare_and_swap_keeps_the_winning_edit() {
    let repo = TestRepo::new();
    let planning = transport(repo.path());
    let initial = planning.save(b"initial", None, None).unwrap();
    let first = planning
        .save(b"first", Some(&initial.revision), None)
        .unwrap();
    assert!(matches!(
        planning.save(b"stale", Some(&initial.revision), None),
        Err(PlanningGitError::ConcurrentWrite)
    ));
    assert_eq!(planning.local().unwrap(), Some(first));
}

#[test]
fn late_publication_does_not_replace_a_causal_reopening() {
    let repo = TestRepo::new();
    let planning = transport(repo.path());
    let done = planning
        .save(br#"{"task":"stable-id","status":"done"}"#, None, None)
        .unwrap();
    assert_eq!(
        planning.publish(&done.revision).unwrap(),
        PlanningPublication::Confirmed
    );
    let reopened = planning
        .save(
            br#"{"task":"stable-id","status":"ready"}"#,
            Some(&done.revision),
            None,
        )
        .unwrap();
    assert_eq!(
        planning.publish(&reopened.revision).unwrap(),
        PlanningPublication::Confirmed
    );
    assert_eq!(
        planning.publish(&done.revision).unwrap(),
        PlanningPublication::Confirmed
    );
    assert_eq!(planning.fetch().unwrap(), Some(reopened));
}

#[test]
fn source_commits_are_rejected_and_absence_does_not_delete_local_data() {
    let repo = TestRepo::new();
    let planning = transport(repo.path());
    let saved = planning.save(b"retained", None, None).unwrap();
    git(
        repo.path(),
        &["push", "origin", "HEAD:refs/loopflow/planning"],
    );
    assert!(matches!(
        planning.fetch(),
        Err(PlanningGitError::Invalid(_))
    ));
    git(repo.path(), &["push", "origin", ":refs/loopflow/planning"]);
    assert_eq!(planning.fetch().unwrap(), None);
    assert_eq!(planning.local().unwrap(), Some(saved));
}
