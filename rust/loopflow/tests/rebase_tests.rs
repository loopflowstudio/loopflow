mod support;

use loopflow::ops::{rebase_with_recovery, NullProgress, OpsError, RebaseOptions};
use loopflow_test_support::TestRepo;
use support::EnvGuard;

#[test]
fn rebase_onto_main_succeeds() {
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");

    repo.checkout("main");
    repo.create_file("main.txt", "main");
    repo.stage_all();
    repo.commit("main work");
    repo.push();

    repo.checkout("feature");
    rebase_with_recovery(
        repo.path(),
        &RebaseOptions {
            onto: "origin/main".to_string(),
            push: false,
        },
        &NullProgress,
    )
    .expect("rebase");
}

#[test]
fn rebase_conflict_launches_agent() {
    // Stub agent exits 0, simulating successful conflict resolution
    let _env = EnvGuard::new(&[("claude", "#!/bin/sh\nexit 0\n")]);
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("conflict.txt", "feature line\n");
    repo.stage_all();
    repo.commit("feature work");

    repo.checkout("main");
    repo.create_file("conflict.txt", "main line\n");
    repo.stage_all();
    repo.commit("main work");
    repo.push();

    repo.checkout("feature");
    rebase_with_recovery(
        repo.path(),
        &RebaseOptions {
            onto: "origin/main".to_string(),
            push: false,
        },
        &NullProgress,
    )
    .expect("agent succeeded, rebase should report success");
}

#[test]
fn rebase_conflict_agent_failure_returns_error() {
    // Stub agent exits 1, simulating failed conflict resolution
    let _env = EnvGuard::new(&[("claude", "#!/bin/sh\nexit 1\n")]);
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("conflict.txt", "feature line\n");
    repo.stage_all();
    repo.commit("feature work");

    repo.checkout("main");
    repo.create_file("conflict.txt", "main line\n");
    repo.stage_all();
    repo.commit("main work");
    repo.push();

    repo.checkout("feature");
    let result = rebase_with_recovery(
        repo.path(),
        &RebaseOptions {
            onto: "origin/main".to_string(),
            push: false,
        },
        &NullProgress,
    );

    assert!(
        matches!(result, Err(OpsError::AgentFailed(_))),
        "expected OpsError::AgentFailed, got: {:?}",
        result
    );
}

#[test]
fn rebase_with_push() {
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.push_new_branch("feature");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");

    repo.checkout("main");
    repo.create_file("main.txt", "main");
    repo.stage_all();
    repo.commit("main work");
    repo.push();

    repo.checkout("feature");
    rebase_with_recovery(
        repo.path(),
        &RebaseOptions {
            onto: "origin/main".to_string(),
            push: true,
        },
        &NullProgress,
    )
    .expect("rebase");
}
