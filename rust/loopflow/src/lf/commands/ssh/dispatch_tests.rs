//! Machine dispatch decisions as table rows.
//!
//! `lf --machine <target> <args…>` reaches `bin/lf.rs` `run`, which calls
//! `lf::navigation::machine_invocation` on the raw argv before normalizing,
//! help or placement: it splits the transport flags from the arguments the
//! target receives. Unless the target is this Machine, `reorder_args` (in the
//! binary) and `ssh::parse_remote_command` parse those arguments for the
//! local side, and `ssh::run` sends them. An SSH exit of 255 becomes
//! `machine::transport_failure`, which names the recovery. Without
//! `--machine`, `work_route::forward` sends a Task's work to its owning
//! Machine; that route is read from planning (ownership rows in
//! `store/sqlite/task_ownership.rs`).
//!
//! SSH, the remote `lf`, store identity and the Machine registry stay with
//! the full-Machine tests in `tests/README.md`.

use clap::{CommandFactory, Parser};

use super::parse_remote_command;
use crate::lf::commands::machine::transport_failure;
use crate::lf::navigation::machine_invocation;
use crate::lf::Cli;

fn invoke(args: &[&str]) -> Result<Option<(Cli, Vec<String>)>, clap::Error> {
    let argv = std::iter::once("lf")
        .chain(args.iter().copied())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    machine_invocation(&argv)
}

#[test]
fn the_target_receives_everything_but_the_transport_flags() {
    for (args, target, sent) in [
        (
            vec![
                "--machine",
                "mini",
                "--task",
                "remote-only-task",
                "implement",
            ],
            "mini",
            vec!["--task", "remote-only-task", "implement"],
        ),
        (
            vec!["--wt", "remote-only-worktree", "--machine=mini", "status"],
            "mini",
            vec!["--wt", "remote-only-worktree", "status"],
        ),
        (
            vec![
                "--machine",
                "mini",
                "--wave",
                "remote-only-wave",
                "wave/operate",
            ],
            "mini",
            vec!["--wave", "remote-only-wave", "wave/operate"],
        ),
        // Help and version belong to the target, not this Machine.
        (vec!["--machine", "mini", "--help"], "mini", vec!["--help"]),
        (
            vec!["--machine", "home_1", "--version"],
            "home_1",
            vec!["--version"],
        ),
        (
            vec!["--machine", "mini", "install", "--help"],
            "mini",
            vec!["self", "install", "--help"],
        ),
        // A trailing `--machine` after a command is still the transport's.
        (
            vec!["machine", "add", "another", "--machine", "mini"],
            "mini",
            vec!["machine", "add", "another"],
        ),
        // Text after a skill and everything after `--` is the target's.
        (
            vec![
                "--machine",
                "mini",
                "commit",
                "-m",
                "literal --machine is text",
            ],
            "mini",
            vec!["commit", "-m", "literal --machine is text"],
        ),
        (
            vec![
                "--machine",
                "fixture",
                "-i",
                "--yolo",
                "--chrome",
                "on",
                "-a",
                "claude:sonnet",
                ":",
                "--",
                "--- literal prompt",
            ],
            "fixture",
            vec![
                "-i",
                "--yolo",
                "--chrome",
                "on",
                "-a",
                "claude:sonnet",
                ":",
                "--",
                "--- literal prompt",
            ],
        ),
    ] {
        let (transport, remote) = invoke(&args).unwrap().unwrap();
        assert_eq!(transport.machine.as_deref(), Some(target), "{args:?}");
        assert_eq!(remote, sent, "{args:?}");
        parse_remote_command(&remote).unwrap();
    }
}

#[test]
fn transport_flags_need_a_machine_and_retired_commands_do_not_parse() {
    // `lf ssh …` parses as a skill named `ssh`; only skill lookup refuses it.
    assert!(invoke(&["status"]).unwrap().is_none());
    assert!(invoke(&["--forward-agent", "status"]).is_err());
    let (_, remote) = invoke(&["--machine", "mini", "--secret", "EXAMPLE", "status"])
        .unwrap()
        .unwrap();
    let error = parse_remote_command(&remote).unwrap_err().to_string();
    assert!(error.contains("unexpected argument '--secret'"), "{error}");
    // Without a Machine there is no dispatch; the local parse refuses.
    assert!(invoke(&["--secret", "EXAMPLE", "status"])
        .unwrap()
        .is_none());
    for refused in [
        ["lf", "--secret", "EXAMPLE", "status"].as_slice(),
        ["lf", "machine", "ssh", "mini", "status"].as_slice(),
    ] {
        assert!(Cli::try_parse_from(refused).is_err(), "{refused:?}");
    }
    assert!(Cli::command()
        .render_help()
        .to_string()
        .contains("--machine"));
    let error = parse_remote_command(&["lf".into(), "status".into()])
        .unwrap_err()
        .to_string();
    assert!(error.contains("remote `lf` is implicit"), "{error}");
}

#[test]
fn ssh_failures_name_their_recovery() {
    for (diagnostic, category) in [
        ("Permission denied (publickey).", "needs-sign-in"),
        (
            "No ED25519 host key is known for mini and you have requested strict checking.\nHost key verification failed.",
            "host-key unknown",
        ),
        (
            "WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!\nHost key verification failed.",
            "host-key changed",
        ),
        ("connect to host mini port 22: Connection refused", "unreachable"),
        ("", "unreachable"),
    ] {
        let error = transport_failure("mini", diagnostic).to_string();
        assert!(error.contains(category), "{error}");
        assert!(error.contains("ssh -o ControlPath=none -- 'mini'"), "{error}");
        assert!(error.contains(diagnostic), "{error}");
    }
}
