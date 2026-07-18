//! In-Run authority fails closed and never degrades to User.
//!
//! User authority is the ambient fallback: `AuthenticatedRequest::cli()` treats
//! local shell presence as the user, which is correct for a local-first product
//! but means an agent body that has lost its Run lease would silently inherit
//! full User rights unless something positively marks it as a body.
//!
//! That marker used to be the legacy Session env vars, so deleting them was a
//! privilege escalation hiding inside a cleanup. `LF_RUN_CONTEXT` now carries
//! the signal alone, and these tests pin the behavior so the deletion cannot
//! quietly restore the fallback.

mod support;

use std::process::Command;
use support::EnvGuard;

fn steer_with(env: &[(&str, &str)], removals: &[&str]) -> std::process::Output {
    let _guard = EnvGuard::new(env);
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    // Well-formed but absent: authority is resolved before the target is
    // looked up, so a parse failure here would mask what these tests prove.
    command.args([
        "work",
        "steer",
        "task",
        "task_00000000000000000000000000000000",
        "do the thing",
    ]);
    for (key, value) in env {
        command.env(key, value);
    }
    for key in removals {
        command.env_remove(key);
    }
    command.output().expect("run lf work steer")
}

/// A body whose lease is gone must be refused, not silently promoted. The
/// refusal has to name the missing lease so an operator can tell this apart
/// from an ordinary authorization failure.
#[test]
fn an_in_run_process_without_a_lease_is_refused_rather_than_treated_as_the_user() {
    let output = steer_with(&[("LF_RUN_CONTEXT", "agent")], &["LF_RUN_LEASE"]);

    assert!(
        !output.status.success(),
        "a lease-less in-Run process must not succeed as User"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("LF_RUN_LEASE"),
        "refusal must name the missing lease, got: {stderr}"
    );
}

/// A malformed lease is a stale or forged credential, not an absent one. It
/// must also fail closed rather than falling through to the User branch.
#[test]
fn a_malformed_lease_fails_closed_instead_of_falling_through_to_user() {
    let output = steer_with(
        &[("LF_RUN_CONTEXT", "agent"), ("LF_RUN_LEASE", "not-a-token")],
        &[],
    );

    assert!(
        !output.status.success(),
        "a malformed lease must not degrade to User authority"
    );
}

/// The deleted Session vars must not be able to re-arm the sentinel. If this
/// ever passes by refusing, someone has keyed authority back onto Session
/// identity and the escalation guard has moved rather than been removed.
#[test]
fn legacy_session_env_alone_no_longer_marks_a_process_as_in_run() {
    let output = steer_with(
        &[("LF_TASK_SESSION_ID", "ts_stale")],
        &["LF_RUN_CONTEXT", "LF_RUN_LEASE"],
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("refusing User authority"),
        "authority must key on LF_RUN_CONTEXT, not on a legacy Session var: {stderr}"
    );
}
