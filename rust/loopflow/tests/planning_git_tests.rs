use std::fs;
use std::path::Path;
use std::process::Command;

use loopflow::engine::planning_git::{
    PlanningDestination, PlanningGit, PlanningGitError, PlanningPublication,
};
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

const PLANNING_REF: &str = "refs/loopflow/planning/users/00000000-0000-4000-8000-000000000001";

fn transport(repo: &Path) -> PlanningGit {
    PlanningGit::new(
        repo,
        &PlanningDestination::resolve(repo, "origin", PLANNING_REF).unwrap(),
    )
    .unwrap()
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
    git(
        second.path(),
        &["clone", first.bare_path().to_str().unwrap(), "."],
    );
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
    let saved = online
        .save(
            br#"{"task":"stable-id","comment":"stable-comment"}"#,
            None,
            None,
        )
        .unwrap();
    let unavailable = repo.bare_path().with_extension("offline");
    fs::rename(repo.bare_path(), &unavailable).unwrap();
    assert_eq!(
        online.publish(&saved.revision).unwrap(),
        PlanningPublication::Unconfirmed
    );
    assert_eq!(online.local().unwrap(), Some(saved.clone()));
    fs::rename(&unavailable, repo.bare_path()).unwrap();
    // A successful publication whose response the caller discards.
    online.publish(&saved.revision).unwrap();
    let tip = git(repo.path(), &["ls-remote", "origin", PLANNING_REF]);
    let fresh = tempfile::tempdir().unwrap();
    git(
        fresh.path(),
        &["clone", repo.bare_path().to_str().unwrap(), "."],
    );
    let recovered = transport(fresh.path());
    assert_eq!(recovered.fetch().unwrap(), Some(saved.clone()));
    assert_eq!(
        recovered.confirm(&saved.revision).unwrap(),
        PlanningPublication::Confirmed
    );
    assert_eq!(
        tip,
        git(repo.path(), &["ls-remote", "origin", PLANNING_REF])
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
        &["push", "origin", &format!("HEAD:{PLANNING_REF}")],
    );
    assert!(matches!(
        planning.fetch(),
        Err(PlanningGitError::Invalid(_))
    ));
    git(
        repo.path(),
        &["push", "origin", &format!(":{PLANNING_REF}")],
    );
    assert_eq!(planning.fetch().unwrap(), None);
    assert_eq!(planning.local().unwrap(), Some(saved));
}

#[test]
fn selected_destinations_never_mix_retained_or_published_plans() {
    let repo = TestRepo::new();
    let personal = transport(repo.path());
    let shared = PlanningGit::new(
        repo.path(),
        &PlanningDestination::resolve(repo.path(), "origin", "refs/loopflow/planning/shared/team")
            .unwrap(),
    )
    .unwrap();
    let private = personal.save(b"private fixture", None, None).unwrap();
    personal.publish(&private.revision).unwrap();
    assert_eq!(shared.local().unwrap(), None);
    assert_eq!(shared.fetch().unwrap(), None);
    assert!(shared.publish(&private.revision).is_err());
    let joined = shared.save(b"shared fixture", None, None).unwrap();
    shared.publish(&joined.revision).unwrap();
    assert_eq!(personal.fetch().unwrap(), Some(private.clone()));
    assert_eq!(personal.local().unwrap(), Some(private));
    assert_eq!(shared.fetch().unwrap(), Some(joined));
    assert!(PlanningDestination::resolve(repo.path(), "origin", "refs/loopflow/planning").is_err());
    assert!(PlanningDestination::resolve(
        repo.path(),
        "origin",
        "refs/loopflow/planning/users/display-name"
    )
    .is_err());
}

#[test]
fn saved_binding_survives_alias_redirection_without_publishing_to_the_replacement() {
    let repo = TestRepo::new();
    let replacement = TestRepo::new();
    let destination = PlanningDestination::resolve(repo.path(), "origin", PLANNING_REF).unwrap();
    let saved_binding = serde_json::to_vec(&destination).unwrap();
    let planning = PlanningGit::new(repo.path(), &destination).unwrap();
    let saved = planning.save(b"private planning", None, None).unwrap();
    git(
        repo.path(),
        &[
            "remote",
            "set-url",
            "origin",
            replacement.bare_path().to_str().unwrap(),
        ],
    );
    let restored = serde_json::from_slice(&saved_binding).unwrap();
    let reopened = PlanningGit::new(repo.path(), &restored).unwrap();
    assert_eq!(
        reopened.publish(&saved.revision).unwrap(),
        PlanningPublication::Confirmed
    );
    assert_eq!(reopened.fetch().unwrap(), Some(saved.clone()));
    assert!(git(replacement.path(), &["ls-remote", "origin", PLANNING_REF]).is_empty());
    let redirected = transport(repo.path());
    assert_eq!(redirected.local().unwrap(), None);
    assert!(redirected.publish(&saved.revision).is_err());
}

#[test]
fn divergent_fetch_and_push_endpoints_cannot_be_bound() {
    let repo = TestRepo::new();
    let replacement = TestRepo::new();
    git(
        repo.path(),
        &[
            "remote",
            "set-url",
            "--push",
            "origin",
            replacement.bare_path().to_str().unwrap(),
        ],
    );
    assert!(PlanningDestination::resolve(repo.path(), "origin", PLANNING_REF).is_err());
}
