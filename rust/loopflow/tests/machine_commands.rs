//! Public commands with isolated stores and a simulated SSH endpoint.
use crate::support;
use std::fs;
use std::io::Write;
use std::os::fd::FromRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::Value;

struct Machines {
    root: tempfile::TempDir,
}

impl Machines {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("local");
        let remote = root.path().join("remote");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(remote.join(".local/bin")).unwrap();
        fs::create_dir_all(remote.join("project's checkout")).unwrap();
        fs::create_dir_all(root.path().join("bin")).unwrap();
        executable(
            &root.path().join("bin/ssh"),
            &format!(
                r#"#!/bin/sh
for arg in "$@"; do remote_command="$arg"; done
cd '{}' || exit 1
exec env -i HOME='{}' PATH=/usr/bin:/bin bash -c "$remote_command"
"#,
                remote.display(),
                remote.display()
            ),
        );
        // Credential side effects stay inside the fixture even on macOS.
        for program in ["gh", "security", "doppler"] {
            executable(
                &root.path().join("bin").join(program),
                "#!/bin/sh\nexit 1\n",
            );
        }
        let fixture = Self { root };
        fixture.remote(
            loopflow::build_info::BUILD_VERSION,
            "home_11111111111111111111111111111111",
        );
        fixture
    }

    fn remote(&self, version: &str, id: &str) {
        executable(
            &self.root.path().join("remote/.local/bin/lf"),
            &format!(
                r#"#!/bin/sh
case "$1" in
--version) echo 'lf {version}';;
machine) if [ "$2" = id ]; then echo '{id}'; else printf 'argument: %s\n' "$@"; fi;;
fail) exit 42;;
*) printf 'remote cwd: %s\n' "$PWD"; printf 'argument: %s\n' "$@";;
esac
"#
            ),
        );
    }

    fn command(&self, args: &[&str]) -> Command {
        let home = self.root.path().join("local");
        let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
        command
            .env_clear()
            .env("HOME", &home)
            .env("LF_HOME", &home)
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.root.path().join("bin").display()),
            )
            .current_dir(&home)
            .args(args);
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }

    fn terminal(&self, args: &[&str], answer: &str) -> Output {
        let mut master = -1;
        let mut slave = -1;
        // SAFETY: valid output pointers; null selects default terminal settings.
        assert_eq!(
            unsafe {
                libc::openpty(
                    &mut master,
                    &mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            },
            0
        );
        // SAFETY: openpty returned two fresh owned descriptors.
        let mut master = unsafe { fs::File::from_raw_fd(master) };
        // SAFETY: slave is the other fresh owned descriptor from openpty.
        let slave = unsafe { fs::File::from_raw_fd(slave) };
        master.write_all(answer.as_bytes()).unwrap();
        self.command(args)
            .stdin(Stdio::from(slave.try_clone().unwrap()))
            .stderr(Stdio::from(slave))
            .output()
            .unwrap()
    }

    fn missing_remote(&self) {
        let bin = self.root.path().join("remote/.local/bin");
        fs::rename(bin.join("lf"), bin.join("lf-published")).unwrap();
        // The download side effect is replaced, not the add/install/probe workflow.
        executable(
            &bin.join("curl"),
            &format!(
                r#"#!/bin/sh
cat <<'INSTALLER'
cp '{}' '{}'
echo installed > '{}'
INSTALLER
"#,
                bin.join("lf-published").display(),
                bin.join("lf").display(),
                self.root.path().join("installed").display()
            ),
        );
    }

    fn foreground(&self, args: &[&str]) -> Output {
        let mut master = -1;
        let mut slave = -1;
        // SAFETY: valid output pointers; null selects default terminal settings.
        assert_eq!(
            unsafe {
                libc::openpty(
                    &mut master,
                    &mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            },
            0
        );
        // SAFETY: openpty returned two fresh owned descriptors.
        let _master = unsafe { fs::File::from_raw_fd(master) };
        // SAFETY: slave is the other fresh owned descriptor from openpty.
        let slave = unsafe { fs::File::from_raw_fd(slave) };
        let mut command = self.command(args);
        command.stdin(Stdio::from(slave));
        // SAFETY: the child only calls async-signal-safe OS functions before exec.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 || libc::ioctl(0, libc::TIOCSCTTY as _, 0) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::tcgetpgrp(0) != libc::getpgrp() {
                    return Err(std::io::Error::other(
                        "fixture needs the foreground terminal",
                    ));
                }
                Ok(())
            });
        }
        command.output().unwrap()
    }

    fn json(&self, args: &[&str]) -> Value {
        let output = self.run(args);
        assert_success(&output);
        serde_json::from_slice(&output.stdout).unwrap()
    }
}

fn executable(path: &Path, content: &str) {
    fs::write(path, content).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn remote_file_input_reaches_the_command_without_replacing_the_transport_script() {
    let fixture = Machines::new();
    fixture.json(&["machine", "add", "mini", "--json"]);
    executable(
        &fixture.root.path().join("remote/.local/bin/lf"),
        "#!/bin/sh\ncat\n",
    );
    let mut child = fixture
        .command(&[
            "--machine",
            "mini",
            "task",
            "save",
            "task_00000000000000000000000000000001",
            "draft.txt",
            "--revision",
            "observed",
            "--json",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let draft = "Unicode λ; literal $(not-a-command)\nsecond line\n";
    child
        .stdin
        .take()
        .unwrap()
        .write_all(draft.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_success(&output);
    assert_eq!(String::from_utf8(output.stdout).unwrap(), draft);
}

#[test]
fn remote_session_open_returns_an_owner_pinned_terminal_command() {
    let fixture = Machines::new();
    let machine = fixture.json(&["machine", "add", "mini", "--json"]);
    let mut record: Value = serde_json::from_str::<Vec<Value>>(include_str!(
        "../../../tests/fixtures/dto/sessions.json"
    ))
    .unwrap()
    .remove(0);
    record["workspace"]["machine_id"] = machine["id"].clone();
    record["open_argv"] = serde_json::json!([
        "/usr/bin/env",
        "LF_HOME=/remote/private home",
        "/bin/sh",
        "-c",
        "printf '%s\\n' \"$LF_HOME\" \"$1\"; cat",
        "fixture",
        "literal ' quote"
    ]);
    let id = record["id"].as_str().unwrap().to_string();
    executable(
        &fixture.root.path().join("remote/.local/bin/lf"),
        &format!("#!/bin/sh\ncat <<'RECORD'\n{record}\nRECORD\n"),
    );
    let opened = fixture.json(&[
        "--machine",
        "mini",
        "session",
        "resume",
        &id,
        "--prepare",
        "--json",
    ]);
    let argv = opened["open_argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(argv[0], "ssh");
    assert!(argv.contains(&"-tt"));
    for target in [
        vec!["session", "resume", id.as_str()],
        vec!["session", "resume"],
        vec!["resume", id.as_str()],
        vec!["resume"],
    ] {
        let mut arguments = vec!["--machine", "mini"];
        arguments.extend(target);
        arguments.push("--prepare");
        let output = fixture.command(&arguments).output().unwrap();
        assert_success(&output);
        let text = String::from_utf8(output.stdout).unwrap();
        assert_eq!(shlex::split(text.trim()).unwrap(), argv);
        arguments.push("--json");
        assert_eq!(fixture.json(&arguments)["open_argv"], opened["open_argv"]);
    }
    let mut command = Command::new(fixture.root.path().join("bin/ssh"));
    command
        .args(&argv[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all("unfinished draft\n".as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_success(&output);
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "/remote/private home\nliteral ' quote\nunfinished draft\n"
    );
    assert_eq!(opened["workspace"], record["workspace"]);
}

#[test]
fn bare_connect_in_the_repository_opens_its_session_on_your_session_machine() {
    let fixture = Machines::new();
    let machine = fixture.json(&["machine", "add", "mini", "--json"]);
    let repo = loopflow_test_support::TestRepo::new();
    let path = repo.path().canonicalize().unwrap();
    let identity = fixture
        .command(&["config", "repository"])
        .current_dir(&path)
        .output()
        .unwrap();
    assert_success(&identity);
    let repository = String::from_utf8(identity.stdout)
        .unwrap()
        .trim()
        .to_string();
    let set = fixture
        .command(&[
            "--repo",
            path.to_str().unwrap(),
            "set",
            "machine.session",
            "mini",
        ])
        .current_dir(&path)
        .output()
        .unwrap();
    assert_success(&set);
    assert!(String::from_utf8_lossy(&set.stdout).contains("its Session runs on mini"));

    let mut record: Value = serde_json::from_str::<Vec<Value>>(include_str!(
        "../../../tests/fixtures/dto/sessions.json"
    ))
    .unwrap()
    .remove(0);
    record["workspace"]["machine_id"] = machine["id"].clone();
    let log = fixture.root.path().join("remote/arguments");
    executable(
        &fixture.root.path().join("remote/.local/bin/lf"),
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" >> '{}'\ncat <<'RECORD'\n{record}\nRECORD\n",
            log.display()
        ),
    );
    let opened = fixture
        .command(&["resume", "--prepare", "--json"])
        .current_dir(&path)
        .output()
        .unwrap();
    assert_success(&opened);
    let arguments = fs::read_to_string(&log).unwrap();
    let arguments = arguments.lines().collect::<Vec<_>>();
    let at = arguments
        .iter()
        .position(|arg| *arg == "--repository")
        .unwrap();
    assert_eq!(arguments[at + 1], repository);
    assert!(arguments.contains(&"resume"), "{arguments:?}");
}

#[test]
fn add_alias_rename_connect_and_remove_preserve_identity() {
    let fixture = Machines::new();
    let added = fixture.json(&["machine", "add", "mini", "--json"]);
    assert_eq!(added["label"], "mini");
    assert_eq!(added["route"], "mini");
    // A Machine is the person's, across repositories: it names no checkout.
    assert!(added.get("repo").is_none());
    assert_eq!(
        fixture.json(&["machine", "list", "mini", "--json"])[0]["id"],
        added["id"]
    );
    let repeated = fixture.json(&["machine", "add", "mini", "--json"]);
    assert_eq!(added["id"], repeated["id"]);
    assert_eq!(added["created_at"], repeated["created_at"]);
    let statuses = fixture.json(&["machine", "status", "mini", "--json"]);
    assert_eq!(
        statuses[0]["remote_version"],
        loopflow::build_info::BUILD_VERSION
    );
    assert_eq!(
        statuses[0]["local_version"],
        loopflow::build_info::BUILD_VERSION
    );
    assert!(statuses[0]["error"].is_null());
    assert_success(&fixture.run(&["machine", "rename", "mini", "builder"]));
    let connected = fixture.run(&["--machine", "builder", "session", "list"]);
    assert_success(&connected);
    assert!(!String::from_utf8_lossy(&connected.stderr).contains("versions differ"));
    let text = String::from_utf8_lossy(&connected.stdout);
    assert!(text.contains("argument: session"), "{text}");
    assert_eq!(
        fixture.run(&["--machine", "builder", "fail"]).status.code(),
        Some(42)
    );
    assert_success(&fixture.run(&["machine", "remove", "builder"]));
    assert_eq!(
        fixture.json(&["machine", "list", "--json"]),
        serde_json::json!([])
    );
    assert!(!fixture
        .run(&["--machine", "builder", "session", "list"])
        .status
        .success());
    let db = rusqlite::Connection::open(fixture.root.path().join("local/loopflow.db")).unwrap();
    let retained: String = db
        .query_row("SELECT id FROM machines WHERE route='mini'", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(retained, added["id"].as_str().unwrap());
    // A replacement at the same destination can be explicitly added after removal.
    fixture.remote(
        loopflow::build_info::BUILD_VERSION,
        "home_22222222222222222222222222222222",
    );
    assert_success(&fixture.run(&["machine", "add", "mini"]));
}

#[test]
fn status_reports_version_mismatch_and_unreachable_without_prompting() {
    let fixture = Machines::new();
    fixture.remote("0.0.1", "home_11111111111111111111111111111111");
    let output = fixture.run(&["machine", "add", "ssh://jack@mini:2222"]);
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("versions differ"));
    let statuses = fixture.json(&["machine", "status", "mini", "--json"]);
    assert_eq!(statuses[0]["remote_version"], "0.0.1");
    assert!(statuses[0]["update_command"]
        .as_str()
        .unwrap()
        .contains("lf install"));
    executable(&fixture.root.path().join("remote/.local/bin/lf"), "#!/bin/sh\nif [ \"$1\" = --version ]; then echo 'lf 0.0.1'; else echo 'unknown machine command' >&2; exit 2; fi\n");
    let statuses = fixture.json(&["machine", "status", "mini", "--json"]);
    assert_eq!(statuses[0]["remote_version"], "0.0.1");
    assert!(statuses[0]["error"]
        .as_str()
        .unwrap()
        .contains("identity probe failed"));
    let connect = fixture.run(&["--machine", "mini", "session", "list"]);
    assert!(!connect.status.success());
    assert!(String::from_utf8_lossy(&connect.stderr).contains("unknown machine command"));
    executable(
        &fixture.root.path().join("bin/ssh"),
        "#!/bin/sh\necho 'key unavailable' >&2\nexit 255\n",
    );
    let statuses = fixture.json(&["machine", "status", "--json"]);
    assert!(statuses[0]["error"]
        .as_str()
        .unwrap()
        .contains("key unavailable"));
    assert!(statuses[0]["remote_version"].is_null());
}

#[test]
fn explicit_machine_dispatch_requires_registration_but_does_not_preflight_identity() {
    let fixture = Machines::new();
    let unregistered = fixture.run(&["--machine", "mini", "session", "list"]);
    assert!(!unregistered.status.success());
    assert!(String::from_utf8_lossy(&unregistered.stderr).contains("not added"));
    assert_success(&fixture.run(&["machine", "add", "mini"]));
    fixture.remote(
        loopflow::build_info::BUILD_VERSION,
        "home_22222222222222222222222222222222",
    );
    let output = fixture.run(&["--machine", "mini", "session", "list"]);
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("remote cwd"));
    assert!(!fixture
        .run(&["machine", "observe", "anything", "mini"])
        .status
        .success());
}

#[test]
fn failures_name_the_recovery_without_prompting_or_registering() {
    for (diagnostic, category) in [
        ("Permission denied (publickey).", "needs-sign-in"),
        ("No ED25519 host key is known for mini and you have requested strict checking.\nHost key verification failed.", "host-key unknown"),
        ("WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!\nHost key verification failed.", "host-key changed"),
        ("connect to host mini port 22: Connection refused", "unreachable"),
    ] {
        let fixture = Machines::new();
        assert_success(&fixture.run(&["machine", "add", "mini"]));
        executable(&fixture.root.path().join("bin/ssh"), &format!("#!/bin/sh\ncat >&2 <<'ERROR'\n{diagnostic}\nERROR\nexit 255\n"));
        let status = fixture.json(&["machine", "status", "--json"]);
        let error = status[0]["error"].as_str().unwrap();
        assert!(error.contains(category), "{error}");
        assert!(error.contains("ssh -o ControlPath=none -- 'mini'"), "{error}");
        let add = fixture.run(&["machine", "add", "other"]);
        assert!(!add.status.success());
        assert!(String::from_utf8_lossy(&add.stderr).contains(category));
        assert_eq!(fixture.json(&["machine", "list", "--json"]).as_array().unwrap().len(), 1);
    }
}

#[test]
fn missing_lf_never_installs_from_status_batch_json_or_nonterminal() {
    let fixture = Machines::new();
    assert_success(&fixture.run(&["machine", "add", "mini"]));
    fixture.missing_remote();
    let status = fixture.json(&["machine", "status", "--json"]);
    let error = status[0]["error"].as_str().unwrap();
    assert!(error.contains("no lf on the remote"), "{error}");
    assert!(error.contains("install.sh"), "{error}");
    assert!(!fixture.run(&["machine", "add", "mini"]).status.success());
    assert!(!fixture
        .terminal(&["-b", "machine", "add", "mini"], "\n")
        .status
        .success());
    assert!(!fixture
        .terminal(&["machine", "add", "mini", "--json"], "\n")
        .status
        .success());
    assert!(!fixture.root.path().join("installed").exists());
}

#[test]
fn interactive_add_offers_install_defaults_yes_and_records_the_new_identity() {
    let fixture = Machines::new();
    fixture.missing_remote();
    assert!(!fixture
        .terminal(&["machine", "add", "mini"], "n\n")
        .status
        .success());
    assert!(!fixture.root.path().join("installed").exists());
    assert_eq!(
        fixture.json(&["machine", "list", "--json"]),
        serde_json::json!([])
    );
    assert_success(&fixture.terminal(&["machine", "add", "mini"], "\n"));
    assert!(fixture.root.path().join("installed").exists());
    let machines = fixture.json(&["machine", "list", "--json"]);
    assert_eq!(machines[0]["id"], "home_11111111111111111111111111111111");
    fs::remove_file(fixture.root.path().join("installed")).unwrap();
    assert_success(&fixture.terminal(&["machine", "add", "mini"], "\n"));
    assert!(!fixture.root.path().join("installed").exists());
}

#[test]
fn broken_existing_lf_and_failed_installer_do_not_register_a_machine() {
    let fixture = Machines::new();
    fixture.missing_remote();
    let bin = fixture.root.path().join("remote/.local/bin");
    executable(&bin.join("lf"), "#!/bin/sh\nexit 127\n");
    assert!(!fixture
        .terminal(&["machine", "add", "mini"], "\n")
        .status
        .success());
    assert!(!fixture.root.path().join("installed").exists());
    fs::remove_file(bin.join("lf")).unwrap();
    executable(&bin.join("curl"), "#!/bin/sh\necho 'exit 1'\n");
    assert!(!fixture
        .terminal(&["machine", "add", "mini"], "y\n")
        .status
        .success());
    assert_eq!(
        fixture.json(&["machine", "list", "--json"]),
        serde_json::json!([])
    );
}

#[test]
fn machine_selector_dispatches_before_local_help_and_placement() {
    let fixture = Machines::new();
    let added = fixture.json(&["machine", "add", "mini", "--json"]);
    for args in [
        vec![
            "--machine",
            "mini",
            "--task",
            "remote-only-task",
            "implement",
        ],
        vec!["--wt", "remote-only-worktree", "--machine=mini", "status"],
        vec![
            "--machine",
            "mini",
            "--wave",
            "remote-only-wave",
            "wave/operate",
        ],
        vec!["--machine", "mini", "--help"],
        vec!["--machine", added["id"].as_str().unwrap(), "--version"],
        vec!["machine", "add", "another", "--machine", "mini"],
        vec!["--machine", "mini", "install", "--help"],
        vec![
            "--machine",
            "mini",
            "commit",
            "-m",
            "literal --machine is text",
        ],
    ] {
        let output = fixture.run(&args);
        assert_success(&output);
        let stdout = String::from_utf8_lossy(&output.stdout);
        if args.last() == Some(&"--version") {
            assert!(
                stdout.contains(loopflow::build_info::BUILD_VERSION),
                "{stdout}"
            );
        } else {
            for arg in args.iter().filter(|arg| {
                !matches!(**arg, "--machine" | "--machine=mini" | "mini")
                    && **arg != added["id"].as_str().unwrap()
            }) {
                assert!(
                    stdout.contains(&format!("argument: {arg}\n")),
                    "{args:?}: {stdout}"
                );
            }
        }
    }
    assert_eq!(
        fixture
            .json(&["machine", "list", "--json"])
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn selected_machine_login_stays_restricted_and_missing_login_stops_headless() {
    use base64::prelude::*;

    let fixture = Machines::new();
    assert_success(&fixture.run(&["machine", "add", "mini"]));
    let db = rusqlite::Connection::open(fixture.root.path().join("local/loopflow.db")).unwrap();
    db.execute(
        "INSERT INTO provider_accounts (provider, account_id, login_email, credential_state, routing_state, created_at, updated_at) VALUES ('codex', 'person', 'person@example.com', 'connected', 'automatic', 0, 0)",
        [],
    ).unwrap();
    let remote = fixture.root.path().join("remote");
    fs::write(remote.join("connected"), "true").unwrap();
    executable(
        &remote.join(".local/bin/lf"),
        &format!(
            r#"#!/bin/sh
case "$1 $2 $3" in
"--version  ") echo 'lf {}';;
"machine id ") echo 'home_11111111111111111111111111111111';;
"machine credentials inspect") cat "$HOME/connected";;
*) printf 'selection: %s\nisolation: %s\n' "$LF_ACCOUNT_SELECTION" "$LF_ACCOUNT_ISOLATION"; printf 'argument: %s\n' "$@";;
esac
"#,
            loopflow::build_info::BUILD_VERSION
        ),
    );
    let args = [
        "--machine",
        "mini",
        "--account",
        "codex=person@",
        "--shared",
        "skill",
        "implement",
        "--",
        "--account=other@",
        "--shared",
    ];
    let after_skill = [
        "--machine",
        "mini",
        "implement",
        "--account",
        "codex=person@",
        "--shared",
        "--",
        "--account=other@",
        "--shared",
    ];
    for args in [args.as_slice(), after_skill.as_slice()] {
        fs::write(remote.join("connected"), "true").unwrap();
        let output = fixture.run(args);
        assert_success(&output);
        let stdout = String::from_utf8(output.stdout).unwrap();
        let selection = stdout
            .lines()
            .find_map(|line| line.strip_prefix("selection: "))
            .unwrap();
        let selection: Value =
            serde_json::from_slice(&BASE64_URL_SAFE_NO_PAD.decode(selection).unwrap()).unwrap();
        assert_eq!(
            selection,
            serde_json::json!({"Restrict": [{"provider": "codex", "account": "person@example.com"}]})
        );
        assert!(stdout.contains("isolation: isolated\n"), "{stdout}");
        assert!(stdout.contains("argument: --isolate\n"), "{stdout}");
        assert!(
            stdout.contains("argument: --\nargument: --account=other@\nargument: --shared\n"),
            "{stdout}"
        );
        assert!(!stdout.contains("argument: codex=person@"), "{stdout}");

        fs::write(remote.join("connected"), "false").unwrap();
        let output = fixture.run(args);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("foreground terminal"), "{stderr}");
        assert!(stderr.contains("lf machine connect"), "{stderr}");
        assert!(!String::from_utf8_lossy(&output.stdout).contains("argument:"));
    }

    for args in [
        vec![
            "-b",
            "machine",
            "connect",
            "mini",
            "codex",
            "person@example.com",
        ],
        vec![
            "--machine",
            "mini",
            "-b",
            "--account",
            "codex=person@",
            "implement",
        ],
    ] {
        fs::write(remote.join("connected"), "false").unwrap();
        let output = fixture.foreground(&args);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("foreground terminal without --batch"),
            "{stderr}"
        );
        assert!(!String::from_utf8_lossy(&output.stdout).contains("argument:"));
        fs::write(remote.join("connected"), "true").unwrap();
        assert_success(&fixture.foreground(&args));
    }
}

#[test]
fn transport_options_require_machine_and_retired_commands_are_gone() {
    let fixture = Machines::new();
    assert_success(&fixture.run(&["machine", "add", "mini"]));
    for args in [
        vec!["--secret", "EXAMPLE", "status"],
        vec!["--machine", "mini", "--secret", "EXAMPLE", "status"],
        vec!["--forward-agent", "status"],
        vec!["machine", "ssh", "mini", "status"],
        vec!["ssh", "mini", "status"],
    ] {
        let output = fixture.run(&args);
        assert!(!output.status.success(), "{args:?}");
        if args.contains(&"--secret") {
            let error = String::from_utf8_lossy(&output.stderr);
            assert!(error.contains("unexpected argument '--secret'"), "{error}");
        }
    }
    let help = fixture.run(&["--help"]);
    assert_success(&help);
    assert!(String::from_utf8_lossy(&help.stdout).contains("--machine"));
}

#[test]
fn remote_command_exit_is_retained_in_local_process_history() {
    let fixture = Machines::new();
    assert_success(&fixture.run(&["machine", "add", "mini"]));
    let database = fixture.root.path().join("local/loopflow.db");
    let db = rusqlite::Connection::open(&database).unwrap();
    let parent = loopflow_test_support::RunningParent::insert(&database);
    let output = parent
        .adopt(&mut fixture.command(&["--machine", "mini", "fail"]))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(42));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("Error:"));
    let outcome: (String, i32, bool) = db.query_row(
        "SELECT outcome, exit_code, completed_at IS NOT NULL FROM processes WHERE command LIKE '%--machine%fail%' ORDER BY started_at DESC LIMIT 1", [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    ).unwrap();
    assert_eq!(outcome, ("failed".to_string(), 42, true));
}

#[test]
fn machine_add_on_the_remote_updates_only_its_registry() {
    let fixture = Machines::new();
    let remote = fixture.root.path().join("remote");
    executable(
        &remote.join(".local/bin/lf"),
        &format!(
            "#!/bin/sh\nexport LF_HOME='{}' LF_BIN='{}'\nexec '{}' \"$@\"\n",
            remote.join("store").display(),
            env!("CARGO_BIN_EXE_lf"),
            env!("CARGO_BIN_EXE_lf"),
        ),
    );
    executable(
        &remote.join(".local/bin/ssh"),
        &format!(
            "#!/bin/sh\nprintf 'lf {}\\nhome_33333333333333333333333333333333\\n'\n",
            loopflow::build_info::BUILD_VERSION
        ),
    );
    assert_success(&fixture.run(&["machine", "add", "mini"]));
    let added = fixture.json(&["--machine", "mini", "machine", "add", "third", "--json"]);
    assert_eq!(added["label"], "third");
    let remote_machines = fixture.json(&["--machine", "mini", "machine", "list", "--json"]);
    assert_eq!(remote_machines.as_array().unwrap().len(), 1);
    assert_eq!(remote_machines[0]["id"], added["id"]);
    let local = fixture.json(&["machine", "list", "--json"]);
    assert_eq!(local.as_array().unwrap().len(), 1);
    assert_eq!(local[0]["label"], "mini");
}

/// Two actual CLIs joined by the fixture's local-only SSH substitute.
fn preview_machines() -> Machines {
    let fixture = Machines::new();
    let remote = fixture.root.path().join("remote");
    executable(
        &remote.join(".local/bin/lf"),
        &format!(
            "#!/bin/sh\nexport LF_HOME='{}' LF_BIN='{}'\nexec '{}' \"$@\"\n",
            remote.join("store").display(),
            env!("CARGO_BIN_EXE_lf"),
            env!("CARGO_BIN_EXE_lf"),
        ),
    );
    // Seed identity using the normal operation before taking the read baseline.
    assert_success(&fixture.run(&["machine", "add", "mini"]));
    for bin in [fixture.root.path().join("bin"), remote.join(".local/bin")] {
        for program in ["gh", "security", "doppler", "claude", "codex", "opencode"] {
            executable(
                &bin.join(program),
                &format!(
                    "#!/bin/sh\necho '{program}' >> '{}'\nexit 91\n",
                    fixture.root.path().join("forbidden-effect").display()
                ),
            );
        }
    }
    fixture
}

fn checkpoint_bytes(path: &Path) -> Vec<u8> {
    let db = rusqlite::Connection::open(path).unwrap();
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    fs::read(path).unwrap()
}

#[test]
fn remote_preview_resolves_repository_on_selected_machine_without_launch_effects() {
    let fixture = preview_machines();
    let remote = fixture.root.path().join("remote");
    let repo = remote.join("projects/project's checkout");
    fs::create_dir_all(repo.join(".lf/skills")).unwrap();
    assert!(Command::new("git")
        .args(["init", "-q"])
        .arg(&repo)
        .status()
        .unwrap()
        .success());
    fs::write(
        repo.join(".lf/skills/probe.md"),
        "Read the remote-only marker.",
    )
    .unwrap();
    fs::write(
        repo.join(".lf/config.yaml"),
        "diff: false\ndiff_files: false\npaste: false\n",
    )
    .unwrap();
    fs::write(remote.join("store/config.yaml"), "repo_root: ~/projects\n").unwrap();
    // A conflicting caller preference must not resolve the destination path.
    fs::write(
        fixture.root.path().join("local/config.yaml"),
        "repo_root: /not-the-remote-root\n",
    )
    .unwrap();
    let local_db = fixture.root.path().join("local/loopflow.db");
    let remote_db = remote.join("store/loopflow.db");
    let local_before = checkpoint_bytes(&local_db);
    let remote_before = checkpoint_bytes(&remote_db);
    let expected_id: String = rusqlite::Connection::open(&remote_db)
        .unwrap()
        .query_row("SELECT id FROM machines WHERE route='local'", [], |row| {
            row.get(0)
        })
        .unwrap();
    let selected_path = repo.canonicalize().unwrap();
    for selector in [
        "project's checkout",
        "~/projects/project's checkout",
        "./projects/project's checkout",
        repo.to_str().unwrap(),
    ] {
        let output = fixture
            .command(&[
                "--machine",
                "mini",
                "--repo",
                selector,
                "--account",
                "unconnected@example.invalid",
                "--shared",
                "skill",
                "probe",
                "literal 'draft'; $(false)",
                "--context",
                "--explain",
                "--json",
            ])
            // Preview must not decode or prepare the caller's account environment.
            .env("LF_ACCOUNT_SELECTION", "not a launch selection")
            .output()
            .unwrap();
        assert_success(&output);
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["resolution"]["machine"]["value"], expected_id);
        assert_eq!(
            report["resolution"]["repository_path"]["value"],
            selected_path.to_str().unwrap()
        );
        assert_eq!(report["resolution"]["task"]["state"], "unbound");
        assert_eq!(report["input"]["checkout"], selected_path.to_str().unwrap());
        assert!(report["input"]["task_prompt"]
            .as_str()
            .unwrap()
            .contains("remote-only marker"));
        assert!(report["input"]["task_prompt"]
            .as_str()
            .unwrap()
            .contains("literal 'draft'; $(false)"));
    }
    let missing = fixture.json(&[
        "--machine",
        "mini",
        "--repo",
        "project's checkout",
        "--task",
        "UNKNOWN-427",
        "--explain",
        "--json",
    ]);
    assert_eq!(missing["machine"]["value"], expected_id);
    assert_eq!(missing["task"]["state"], "unavailable");
    assert!(missing["task"]["reason"]
        .as_str()
        .unwrap()
        .contains("UNKNOWN-427"));
    let text = fixture.run(&[
        "--machine",
        "mini",
        "--repo",
        "project's checkout",
        "--task",
        "UNKNOWN-427",
        "--explain",
    ]);
    assert_success(&text);
    assert!(String::from_utf8_lossy(&text.stdout).contains("Task: unavailable:"));
    assert_eq!(
        checkpoint_bytes(&local_db),
        local_before,
        "preview mutated caller registry"
    );
    assert_eq!(
        checkpoint_bytes(&remote_db),
        remote_before,
        "preview mutated destination registry"
    );
    assert!(!fixture.root.path().join("forbidden-effect").exists());
    assert!(!repo.join(".lf/tmp").exists());
    assert!(!remote.join(".codex").exists());
    assert!(!remote.join(".claude").exists());
}

#[test]
fn remote_preview_preserves_absent_corrupt_and_changed_machine_evidence() {
    let fixture = preview_machines();
    let local_db = fixture.root.path().join("local/loopflow.db");
    let remote_db = fixture.root.path().join("remote/store/loopflow.db");
    let local_before = checkpoint_bytes(&local_db);
    let remote_before = checkpoint_bytes(&remote_db);
    fs::remove_file(&remote_db).unwrap();
    let absent = fixture.run(&["--machine", "mini", "--explain", "--json"]);
    assert!(!absent.status.success());
    assert!(String::from_utf8_lossy(&absent.stderr).contains("Machine identity unavailable"));
    assert!(
        !remote_db.exists(),
        "preview initialized the missing Machine"
    );
    fs::write(&remote_db, b"not a database").unwrap();
    let corrupt = fixture.run(&["--machine", "mini", "--explain"]);
    assert!(!corrupt.status.success());
    assert_eq!(fs::read(&remote_db).unwrap(), b"not a database");
    fs::write(&remote_db, &remote_before).unwrap();
    {
        let db = rusqlite::Connection::open(&remote_db).unwrap();
        db.execute(
            "UPDATE machines SET id='home_99999999999999999999999999999999' WHERE route='local'",
            [],
        )
        .unwrap();
    }
    let changed_before = checkpoint_bytes(&remote_db);
    let changed = fixture.run(&["--machine", "mini", "--explain"]);
    assert!(!changed.status.success());
    assert!(String::from_utf8_lossy(&changed.stderr).contains("expected Machine"));
    assert_eq!(checkpoint_bytes(&remote_db), changed_before);
    assert_eq!(checkpoint_bytes(&local_db), local_before);
    assert!(!fixture.root.path().join("forbidden-effect").exists());
}

#[test]
fn addressed_commands_validate_identity_before_reads_writes_and_forwarding() {
    let fixture = preview_machines();
    let local_db = fixture.root.path().join("local/loopflow.db");
    let remote_db = fixture.root.path().join("remote/store/loopflow.db");
    let local_before = checkpoint_bytes(&local_db);
    let remote_before = checkpoint_bytes(&remote_db);
    for args in [
        vec!["--help"],
        vec!["--version"],
        vec!["wave", "show", "--json"],
        vec!["--repo", "/unavailable", "--explain"],
        vec!["machine", "rename", "mini", "changed"],
        vec!["self", "doctor", "--json"],
        vec!["task", "run", "UNKNOWN-427"],
        vec!["--machine", "mini", "machine", "list", "--json"],
    ] {
        let output = fixture
            .command(&args)
            .env(
                "LF_EXPECTED_MACHINE_ID",
                "home_99999999999999999999999999999999",
            )
            .output()
            .unwrap();
        assert!(!output.status.success(), "{args:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("expected Machine"),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty(), "{args:?}");
    }
    assert_eq!(checkpoint_bytes(&local_db), local_before);
    assert_eq!(checkpoint_bytes(&remote_db), remote_before);
    assert!(!fixture.root.path().join("forbidden-effect").exists());
}

#[test]
fn remote_preview_connection_failure_never_falls_back_to_local_execution() {
    let fixture = preview_machines();
    let local_db = fixture.root.path().join("local/loopflow.db");
    let before = checkpoint_bytes(&local_db);
    executable(
        &fixture.root.path().join("bin/ssh"),
        "#!/bin/sh\nexit 255\n",
    );
    let output = fixture.run(&["--machine", "mini", "--context", "skill", "debug"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unreachable"));
    assert!(output.stdout.is_empty());
    assert_eq!(checkpoint_bytes(&local_db), before);
    assert!(!fixture.root.path().join("forbidden-effect").exists());
}

#[test]
fn remote_preview_uses_the_same_work_for_explicit_and_inferred_task_selection() {
    let fixture = preview_machines();
    let repo = loopflow_test_support::TestRepo::new();
    support::bind_task_planning(&repo);
    repo.create_branch("remote-preview");
    let remote_store = fixture.root.path().join("remote/store");
    let registered = support::register_task_with_pr(
        &remote_store,
        &repo.path().canonicalize().unwrap(),
        "remote-preview",
        &repo.head_sha(),
    );
    support::record_graph(
        &remote_store,
        repo.path(),
        "prior",
        "prior-step",
        "succeeded",
    );
    let local_db = fixture.root.path().join("local/loopflow.db");
    let remote_db = remote_store.join("loopflow.db");
    let local_before = checkpoint_bytes(&local_db);
    let remote_before = checkpoint_bytes(&remote_db);
    let read = |args: &[&str]| {
        let mut command = vec!["--machine", "mini", "--repo", repo.path().to_str().unwrap()];
        command.extend(args);
        fixture.json(&command)
    };
    let explicit = read(&["task", "run", "INF-123", "--explain", "--json"]);
    let inferred = read(&["task", "run", "--explain", "--json"]);
    for report in [&explicit, &inferred] {
        assert_eq!(
            report["resolution"]["task"]["value"],
            registered.task.id.as_str()
        );
        assert_eq!(
            report["resolution"]["checkout"]["value"],
            repo.path().canonicalize().unwrap().to_str().unwrap()
        );
    }
    assert_eq!(explicit["action"], inferred["action"]);
    assert_eq!(explicit["impediments"], inferred["impediments"]);
    assert_eq!(checkpoint_bytes(&local_db), local_before);
    assert_eq!(checkpoint_bytes(&remote_db), remote_before);
    assert!(!fixture.root.path().join("forbidden-effect").exists());
}

#[test]
fn graph_context_preview_reads_selected_machine_graph_without_executing_steps() {
    let fixture = preview_machines();
    let remote = fixture.root.path().join("remote");
    let repo = remote.join("projects/preview");
    fs::create_dir_all(repo.join(".lf/skills")).unwrap();
    fs::create_dir_all(repo.join(".lf/graphs")).unwrap();
    assert!(Command::new("git")
        .args(["init", "-q"])
        .arg(&repo)
        .status()
        .unwrap()
        .success());
    fs::write(remote.join("store/config.yaml"), "repo_root: ~/projects\n").unwrap();
    fs::write(
        fixture.root.path().join("local/config.yaml"),
        "repo_root: /caller-only\n",
    )
    .unwrap();
    fs::write(
        repo.join(".lf/config.yaml"),
        "diff: false\ndiff_files: false\npaste: false\n",
    )
    .unwrap();
    for (skill, body) in [
        ("route", "Remote router input."),
        ("work", "Future work input must not be assembled."),
        ("decide", "Future decision input."),
    ] {
        fs::write(repo.join(format!(".lf/skills/{skill}.md")), body).unwrap();
    }
    fs::write(
        repo.join(".lf/graphs/repair.yaml"),
        "- work\n- loop: work\n  step: decide\n",
    )
    .unwrap();
    fs::write(repo.join(".lf/graphs/probe.yaml"), "- xor:\n    router: route\n    paths:\n      repair:\n        description: Repair the work\n        graph: repair\n      skip:\n        description: Skip repairs\n- cmd: config set repo_root /must-not-write\n- work\n").unwrap();
    fs::write(
        repo.join(".lf/graphs/command-first.yaml"),
        "- cmd: config set repo_root /must-not-write\n- work\n",
    )
    .unwrap();
    fs::write(repo.join(".lf/graphs/invalid.yaml"), "- absent-skill\n").unwrap();
    let local_db = fixture.root.path().join("local/loopflow.db");
    let remote_db = remote.join("store/loopflow.db");
    let before = [checkpoint_bytes(&local_db), checkpoint_bytes(&remote_db)];
    let args = [
        "--machine",
        "mini",
        "--repo",
        "preview",
        "graph",
        "probe",
        "literal 'input'; $(false)",
        "--context",
    ];
    let mut json_args = args.to_vec();
    json_args.extend(["--explain", "--json"]);
    let report = fixture.json(&json_args);
    let input = &report["input"];
    assert_eq!(
        input["checkout"],
        repo.canonicalize().unwrap().to_str().unwrap()
    );
    let compiled = &input["compiled"];
    assert_eq!(compiled["steps"][0]["kind"], "xor");
    assert_eq!(compiled["steps"][0]["paths"][0]["name"], "repair");
    assert_eq!(compiled["steps"][0]["paths"][1]["name"], "skip");
    let nested = &compiled["steps"][0]["paths"][0]["steps"];
    assert_eq!(nested[1]["returns_to"], nested[0]["key"]);
    let inputs = input["inputs"].as_array().unwrap();
    assert_eq!(inputs.len(), 5);
    assert_eq!(inputs[0]["state"], "current");
    let prompt = inputs[0]["input"]["task_prompt"].as_str().unwrap();
    assert!(prompt.contains("Remote router input."));
    assert!(prompt.contains("literal 'input'; $(false)"));
    assert!(prompt.contains("Repair the work"));
    assert!(prompt.contains("Skip repairs"));
    assert!(prompt.contains("declared JSON value"));
    assert!(!prompt.contains("Future work input must not be assembled."));
    for index in [1, 2, 4] {
        assert_eq!(inputs[index]["state"], "unavailable");
        assert!(inputs[index]["reason"]
            .as_str()
            .unwrap()
            .contains("repeat-pass"));
        assert!(inputs[index].get("input").is_none());
    }
    assert_eq!(inputs[3]["state"], "not_agent");
    let text = fixture.run(&args);
    assert_success(&text);
    let text = String::from_utf8_lossy(&text.stdout);
    assert!(text.contains("path repair"));
    assert!(text.contains("returns to"));
    assert!(text.contains("unavailable"));
    let command = fixture.json(&[
        "--machine",
        "mini",
        "--repo",
        "preview",
        "graph",
        "command-first",
        "--context",
        "--json",
    ]);
    assert_eq!(command["inputs"][0]["state"], "not_agent");
    assert_eq!(command["inputs"][1]["state"], "unavailable");
    let invalid = fixture.run(&[
        "--machine",
        "mini",
        "--repo",
        "preview",
        "graph",
        "invalid",
        "--context",
    ]);
    assert!(!invalid.status.success());
    assert_eq!(
        [checkpoint_bytes(&local_db), checkpoint_bytes(&remote_db)],
        before
    );
    assert_eq!(
        fs::read_to_string(remote.join("store/config.yaml")).unwrap(),
        "repo_root: ~/projects\n"
    );
    assert!(!fixture.root.path().join("forbidden-effect").exists());
    assert!(!repo.join(".lf/tmp").exists());
}

/// Two Homes share one repository plan through Git planning; ownership entries
/// travel as marked comments and every append runs on the Task's writer.
struct SharedPlan {
    repository: loopflow::durable::RepositoryId,
    local: loopflow::store::sqlite::SqliteStore,
    remote: loopflow::store::sqlite::SqliteStore,
    local_path: std::path::PathBuf,
    remote_path: std::path::PathBuf,
    destination: loopflow::planning_git::PlanningDestination,
    _repos: Vec<loopflow_test_support::TestRepo>,
    _person: Person,
}

/// In-process writes name the same person as the fixture's `lf` commands,
/// whose Git identity is the test repository's.
struct Person(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

impl Person {
    fn jack() -> Self {
        let guard = loopflow_test_support::env_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        std::env::set_var("LF_USER_NAME", "Jack");
        Self(guard)
    }
}

impl Drop for Person {
    fn drop(&mut self) {
        std::env::remove_var("LF_USER_NAME");
    }
}

impl SharedPlan {
    fn new(fixture: &Machines) -> Self {
        let repos = vec![
            loopflow_test_support::TestRepo::new(),
            loopflow_test_support::TestRepo::new(),
        ];
        let local_path = repos[0].path().canonicalize().unwrap();
        let remote_path = repos[1].path().canonicalize().unwrap();
        Self::with(fixture, repos, local_path, remote_path)
    }

    /// Mini's checkout is a clone of the laptop's origin, so a branch pushed
    /// from one checks out on the other.
    fn cloned(fixture: &Machines) -> Self {
        let repo = loopflow_test_support::TestRepo::new();
        let local_path = repo.path().canonicalize().unwrap();
        let remote_path = fixture.root.path().join("remote/checkout");
        git(
            fixture.root.path(),
            &[
                "clone",
                "-q",
                repo.bare_path().to_str().unwrap(),
                remote_path.to_str().unwrap(),
            ],
        );
        git(&remote_path, &["config", "user.email", "test@test.com"]);
        git(&remote_path, &["config", "user.name", "Jack"]);
        let remote_path = remote_path.canonicalize().unwrap();
        Self::with(fixture, vec![repo], local_path, remote_path)
    }

    fn with(
        fixture: &Machines,
        repos: Vec<loopflow_test_support::TestRepo>,
        local_path: std::path::PathBuf,
        remote_path: std::path::PathBuf,
    ) -> Self {
        use loopflow::durable::RepositoryId;
        use loopflow::planning_git::PlanningDestination;
        use loopflow::store::sqlite::SqliteStore;
        let local = SqliteStore::new(&fixture.root.path().join("local/loopflow.db")).unwrap();
        let remote =
            SqliteStore::new(&fixture.root.path().join("remote/store/loopflow.db")).unwrap();
        let repository = RepositoryId::new();
        local
            .bind_repository(local_path.to_str().unwrap(), &repository)
            .unwrap();
        remote
            .bind_repository(remote_path.to_str().unwrap(), &repository)
            .unwrap();
        let destination =
            PlanningDestination::new("/unused/fixture", "refs/loopflow/planning/shared/owners")
                .unwrap();
        for (store, path) in [(&local, &local_path), (&remote, &remote_path)] {
            store
                .bind_peer_planning(path.to_str().unwrap(), &destination)
                .unwrap();
        }
        Self {
            repository,
            local,
            remote,
            local_path,
            remote_path,
            destination,
            _repos: repos,
            _person: Person::jack(),
        }
    }

    fn to_local(&self) {
        let journal = self
            .remote
            .export_peer_planning(self.remote_path.to_str().unwrap(), &self.destination.id())
            .unwrap();
        self.local
            .import_peer_planning(
                self.local_path.to_str().unwrap(),
                &self.destination.id(),
                &uuid::Uuid::new_v4().to_string(),
                &journal,
            )
            .unwrap();
    }

    fn to_remote(&self) {
        let journal = self
            .local
            .export_peer_planning(self.local_path.to_str().unwrap(), &self.destination.id())
            .unwrap();
        self.remote
            .import_peer_planning(
                self.remote_path.to_str().unwrap(),
                &self.destination.id(),
                &uuid::Uuid::new_v4().to_string(),
                &journal,
            )
            .unwrap();
    }

    fn task(&self) -> loopflow::durable::TaskId {
        Self::create_task(&self.remote, &self.remote_path)
    }

    /// A Task only the laptop knows: mini never synced its planning.
    fn local_task(&self) -> loopflow::durable::TaskId {
        Self::create_task(&self.local, &self.local_path)
    }

    fn create_task(
        store: &loopflow::store::sqlite::SqliteStore,
        path: &Path,
    ) -> loopflow::durable::TaskId {
        let project = store
            .ensure_wave_project(path.to_str().unwrap(), "owners")
            .unwrap();
        store
            .create_task(&loopflow::work::task::NewTask {
                repository_id: store.ensure_repository(path.to_str().unwrap()).unwrap(),
                id: loopflow::durable::TaskId::new(),
                project_id: Some(project.id),
                title: "Ownership demo".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap()
            .id
    }
}

#[test]
fn task_ownership_follows_claim_handoff_and_unclaim_through_the_writer() {
    use loopflow::durable::{OwnershipKind, TaskExecutionSource};
    let fixture = preview_machines();
    let plan = SharedPlan::new(&fixture);
    let task = plan.task();
    let mini = plan.remote.local_machine().unwrap().id;
    let laptop = plan.local.local_machine().unwrap().id;
    plan.remote.claim_task(&task).unwrap().unwrap();
    plan.to_local();
    // The laptop follows the claim; it routes to the owner, never allocating here.
    let route = plan.local.task_execution_route(&task).unwrap();
    assert_eq!(
        (route.machine_id.clone(), route.source),
        (mini.clone(), TaskExecutionSource::Owner)
    );
    assert!(plan.local.claim_task(&task).is_err());

    // Moving it to the laptop sends it from mini, the writer, over SSH.
    let handed = fixture.json(&[
        "--repository",
        plan.repository.as_str(),
        "task",
        "move",
        task.as_str(),
        "--machine",
        laptop.as_str(),
        "--json",
    ]);
    assert_eq!(handed["owner"]["machine"]["id"], laptop.as_str());
    let kinds = |store: &loopflow::store::sqlite::SqliteStore| {
        store
            .task_ownership(&task)
            .unwrap()
            .log
            .iter()
            .map(|entry| entry.kind)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        kinds(&plan.remote),
        [OwnershipKind::Claim, OwnershipKind::Handoff]
    );
    plan.to_local();
    assert_eq!(
        plan.local.task_execution_route(&task).unwrap().machine_id,
        laptop
    );

    // The laptop now writes; unclaim keeps the history and leaves it unowned.
    let unclaimed = fixture.json(&[
        "--repository",
        plan.repository.as_str(),
        "task",
        "unclaim",
        task.as_str(),
        "--json",
    ]);
    assert!(unclaimed["owner"].is_null());
    plan.to_remote();
    for store in [&plan.local, &plan.remote] {
        let ownership = store.task_ownership(&task).unwrap();
        assert!(ownership.owner.is_none());
        assert_eq!(
            kinds(store),
            [
                OwnershipKind::Claim,
                OwnershipKind::Handoff,
                OwnershipKind::Unclaim
            ]
        );
    }
    assert!(!fixture.root.path().join("forbidden-effect").exists());
}

#[test]
fn task_ownership_double_claim_keeps_the_first_claimant_and_names_the_later_claim() {
    let fixture = preview_machines();
    let plan = SharedPlan::new(&fixture);
    let task = plan.task();
    plan.to_local();
    let mini = plan.remote.local_machine().unwrap().id;
    let laptop = plan.local.local_machine().unwrap().id;
    // No owning Machine is configured, so each Machine may claim for itself.
    plan.remote.claim_task(&task).unwrap().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    plan.local.claim_task(&task).unwrap().unwrap();
    plan.to_remote();
    plan.to_local();
    let first = plan.remote.task_ownership(&task).unwrap();
    assert_eq!(first.owner.unwrap().machine.id, Some(mini.clone()));
    assert_eq!(
        first.superseded_by.unwrap().machine.id,
        Some(laptop.clone())
    );
    assert_eq!(
        plan.remote.task_execution_route(&task).unwrap().machine_id,
        mini
    );
    let steers = plan.remote.task_steers(&task).unwrap();
    assert!(steers
        .iter()
        .any(|steer| steer.text.starts_with("Also claimed later by")));
    let second = plan.local.task_ownership(&task).unwrap();
    assert_eq!(second.owner.unwrap().machine.id, Some(laptop));
    assert!(second.superseded_by.is_none());
    // Exchange settles: neither Machine re-claims a log that shows a later claim.
    plan.to_remote();
    plan.to_local();
    assert_eq!(plan.remote.task_ownership(&task).unwrap().log.len(), 2);
}

#[test]
fn task_ownership_owner_reclaims_after_a_stale_unclaim_elsewhere() {
    let fixture = preview_machines();
    let plan = SharedPlan::new(&fixture);
    let task = plan.task();
    plan.to_local();
    let mini = plan.remote.local_machine().unwrap().id;
    plan.local.claim_task(&task).unwrap().unwrap();
    plan.to_remote();
    // A stale reading on mini treats the Task as its own and unclaims it there,
    // delivering the unclaim as mini would: a marked comment in the journal.
    let entry = serde_json::json!({
        "id": format!("own_{}", uuid::Uuid::new_v4().simple()),
        "task_id": task.as_str(), "kind": "unclaim", "person": "Kim",
        "machine_id": mini.as_str(),
        "at": time::OffsetDateTime::now_utc().unix_timestamp() + 5,
    });
    plan.remote
        .append_task_comment(
            &task,
            &loopflow::work::task::comment::TaskComment {
                id: uuid::Uuid::new_v4().to_string(),
                body: format!("Kim unclaimed this\n\n<!-- loopflow-ownership:{entry} -->"),
                author: loopflow::work::task::comment::TaskCommentAuthor::Integration,
                created_at: Some(
                    (time::OffsetDateTime::now_utc() + time::Duration::seconds(5))
                        .format(&time::format_description::well_known::Rfc3339)
                        .unwrap(),
                ),
            },
        )
        .unwrap();
    plan.to_local();
    // The laptop's claim stays open locally and it claims again in the journal.
    let ownership = plan.local.task_ownership(&task).unwrap();
    assert_eq!(
        ownership.owner.unwrap().machine.id,
        Some(plan.local.local_machine().unwrap().id)
    );
    assert_eq!(
        ownership.log.last().unwrap().kind,
        loopflow::durable::OwnershipKind::Claim
    );
}

#[test]
fn task_ownership_never_routes_to_another_persons_machine() {
    use loopflow::durable::{OwningMachineSource, TaskExecutionSource};
    let fixture = preview_machines();
    let plan = SharedPlan::new(&fixture);
    let task = plan.task();
    let mini = plan.remote.local_machine().unwrap().id;
    // Kim claims on mini, a Machine this laptop also has registered. Her
    // person key, not the Machine, says whose claim it is.
    let entry = serde_json::json!({
        "id": format!("own_{}", uuid::Uuid::new_v4().simple()),
        "task_id": task.as_str(), "kind": "claim", "person": "Kim",
        "person_key": "kim@example.com",
        "machine_id": mini.as_str(),
        "at": time::OffsetDateTime::now_utc().unix_timestamp(),
    });
    plan.remote
        .append_task_comment(
            &task,
            &loopflow::work::task::comment::TaskComment {
                id: uuid::Uuid::new_v4().to_string(),
                body: format!("Kim claimed this\n\n<!-- loopflow-ownership:{entry} -->"),
                author: loopflow::work::task::comment::TaskCommentAuthor::Integration,
                created_at: Some(
                    time::OffsetDateTime::now_utc()
                        .format(&time::format_description::well_known::Rfc3339)
                        .unwrap(),
                ),
            },
        )
        .unwrap();
    plan.to_local();
    assert_eq!(
        plan.local
            .task_ownership(&task)
            .unwrap()
            .owner
            .unwrap()
            .person
            .as_deref(),
        Some("Kim")
    );
    // Someone else's claim stays theirs: no route to mini, no handoff or unclaim.
    let route = plan.local.task_execution_route(&task).unwrap();
    assert_eq!(
        (route.machine_id, route.source),
        (
            plan.local.local_machine().unwrap().id,
            TaskExecutionSource::OwningMachine(OwningMachineSource::ThisMachine)
        )
    );
    assert!(plan.local.unclaim_task(&task).is_err());
    assert!(plan.local.handoff_task(&task, &mini).is_err());
    // Claiming writes here; the later claim wins.
    plan.local.claim_task(&task).unwrap().unwrap();
    plan.to_remote();
    // Kim's claim stays Kim's on mini, which now sees the later claim.
    let later = plan
        .remote
        .task_ownership(&task)
        .unwrap()
        .superseded_by
        .unwrap();
    assert_eq!(
        later.machine.id,
        Some(plan.local.local_machine().unwrap().id)
    );
}

#[test]
fn set_machine_copies_wave_and_repository_defaults_to_your_other_machines() {
    use loopflow::durable::{MachineSetting, OwningMachineSource};
    let fixture = preview_machines();
    let plan = SharedPlan::new(&fixture);
    let task = plan.task();
    plan.to_local();
    let mini = plan.remote.local_machine().unwrap().id;
    let laptop = plan.local.local_machine().unwrap().id;
    let repository = ["--repository", plan.repository.as_str()];
    let mut wave = repository.to_vec();
    wave.extend(["--wave", "owners", "set", "machine", "mini"]);
    let output = fixture.run(&wave);
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("Wave owners:"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("copied to mini"));
    // A selected repository with no Wave sets the repository's default.
    let mut bare = repository.to_vec();
    bare.extend(["set", "machine", "mini"]);
    let output = fixture.run(&bare);
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("This repository:"));
    // The Wave Session comes back to the laptop; its Tasks stay on mini.
    let mut session = repository.to_vec();
    session.extend([
        "--wave",
        "owners",
        "set",
        "machine.session",
        laptop.as_str(),
    ]);
    let output = fixture.run(&session);
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("Wave owners: its Session runs"));
    for (store, path) in [
        (&plan.local, &plan.local_path),
        (&plan.remote, &plan.remote_path),
    ] {
        let wave = store
            .ensure_wave_project(path.to_str().unwrap(), "owners")
            .unwrap()
            .wave_id;
        assert_eq!(
            store
                .wave_owning_machine(&wave, MachineSetting::Tasks)
                .unwrap(),
            (mini.clone(), OwningMachineSource::Wave("owners".into()))
        );
        assert_eq!(
            store
                .wave_owning_machine(&wave, MachineSetting::Session)
                .unwrap(),
            (laptop.clone(), OwningMachineSource::Wave("owners".into()))
        );
        for setting in [MachineSetting::Tasks, MachineSetting::Session] {
            assert_eq!(
                store
                    .repository_owning_machine(&plan.repository, setting)
                    .unwrap(),
                (mini.clone(), OwningMachineSource::Repository)
            );
        }
    }
    let listed = fixture.json(&["machine", "list", "--json"]);
    let listing = |id: &str| {
        listed
            .as_array()
            .unwrap()
            .iter()
            .find(|listing| listing["id"] == id)
            .unwrap()
            .clone()
    };
    let scopes = listing(mini.as_str())["defaults"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            format!(
                "{} {}",
                d["scope"]["kind"].as_str().unwrap(),
                d["setting"].as_str().unwrap()
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        listing(laptop.as_str())["defaults"][0]["setting"],
        "session"
    );
    assert_eq!(
        scopes,
        ["wave tasks", "repository tasks", "repository session"]
    );
    // The laptop routes the unowned Task to mini; mini claims it for itself.
    assert_eq!(
        plan.local.task_execution_route(&task).unwrap().machine_id,
        mini
    );
    assert!(plan.local.claim_task(&task).is_err());
    plan.remote.claim_task(&task).unwrap().unwrap();
}

#[test]
fn remote_companions_keep_the_recorded_owner_and_stream_file_changes() {
    use loopflow::durable::RepositoryId;
    use loopflow::ops::task::TaskFilesFrame;
    use loopflow::store::sqlite::SqliteStore;
    use std::io::{BufRead, BufReader};
    use std::sync::mpsc;
    use std::time::Duration;

    let fixture = preview_machines();
    let repo = loopflow_test_support::TestRepo::new();
    let path = repo.path().canonicalize().unwrap();
    let remote_home = fixture.root.path().join("remote/store");
    let registered =
        support::register_task_with_pr(&remote_home, &path, "companions", &repo.head_sha());
    let store = SqliteStore::new(&remote_home.join("loopflow.db")).unwrap();
    let repository = RepositoryId::new();
    store
        .bind_repository(path.to_str().unwrap(), &repository)
        .unwrap();
    let owner = store.local_machine().unwrap().id;
    fs::write(path.join(".gitignore"), "target/\n").unwrap();
    fs::create_dir(path.join("target")).unwrap();
    let shell = fixture.root.path().join("remote/.local/bin/fixture-shell");
    executable(&shell, "#!/bin/sh\nprintf 'shell at %s\\n' \"$PWD\"\ncat\n");
    let launcher = fixture.root.path().join("remote/.local/bin/lf");
    let script = fs::read_to_string(&launcher).unwrap();
    fs::write(
        &launcher,
        script.replacen(
            "export LF_HOME",
            &format!("export SHELL='{}'\nexport LF_HOME", shell.display()),
            1,
        ),
    )
    .unwrap();
    let args = [
        "--machine",
        "mini",
        "--repository",
        repository.as_str(),
        "task",
        "shell",
        registered.task.id.as_str(),
        "--checkout",
        path.to_str().unwrap(),
    ];
    let mut child = fixture
        .command(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all("literal ' text\n".as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_success(&output);
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("shell at {}\nliteral ' text\n", path.display())
    );
    let mut stale = args;
    stale[8] = "/different-checkout";
    let rejected = fixture.run(&stale);
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("checkout changed"));
    assert!(rejected.stdout.is_empty());

    let mut child = fixture
        .command(&[
            "--machine",
            "mini",
            "--repository",
            repository.as_str(),
            "task",
            "watch-files",
            registered.task.id.as_str(),
            "--checkout",
            path.to_str().unwrap(),
            "--request",
            "owned-request",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if send.send(line.unwrap()).is_err() {
                break;
            }
        }
    });
    let read = || -> TaskFilesFrame {
        serde_json::from_str(&receive.recv_timeout(Duration::from_secs(10)).unwrap()).unwrap()
    };
    let first = read();
    assert_eq!(first.request, "owned-request");
    assert_eq!(first.task_id, registered.task.id);
    assert_eq!(first.machine_id, owner);
    assert_eq!(first.checkout, path.to_str().unwrap());
    assert_eq!(first.paths, [""]);
    // Ignored build output never invalidates; an atomic replacement must, even
    // when dirty status stays dirty.
    fs::write(path.join("target/out"), "build\n").unwrap();
    repo.create_file("replacement", "new peer contents\n");
    fs::rename(path.join("replacement"), path.join("note.txt")).unwrap();
    let mut changed = Vec::new();
    for _ in 0..5 {
        changed.extend(read().paths);
        if changed.iter().any(|path| path == "note.txt") {
            break;
        }
    }
    // Closing the reader's stdin must drain the remote observer, not leave it resident.
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert_success(&output);
    assert!(changed.iter().any(|path| path == "note.txt"));
    assert!(!changed.iter().any(|path| path.starts_with("target")));
    assert!(!fixture.root.path().join("forbidden-effect").exists());
}

/// Claude's project directory for a checkout.
fn claude_project(path: &Path) -> String {
    path.to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    assert_success(&output);
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

#[test]
fn resume_moves_a_session_to_another_machine() {
    use loopflow::store::sqlite::SqliteStore;
    let fixture = preview_machines();
    let mini = fixture.json(&["machine", "list", "--json"])[0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let repo = loopflow_test_support::TestRepo::new();
    let worktree = repo
        .create_named_worktree("move-demo")
        .canonicalize()
        .unwrap();
    let remote_checkout = fixture.root.path().join("remote/checkout");
    git(
        fixture.root.path(),
        &[
            "clone",
            "-q",
            repo.bare_path().to_str().unwrap(),
            remote_checkout.to_str().unwrap(),
        ],
    );
    git(&remote_checkout, &["config", "user.email", "test@test.com"]);
    git(&remote_checkout, &["config", "user.name", "Jack"]);
    let remote_checkout = remote_checkout.canonicalize().unwrap();
    let repository = loopflow::durable::RepositoryId::new();
    let local = SqliteStore::new(&fixture.root.path().join("local/loopflow.db")).unwrap();
    let remote = SqliteStore::new(&fixture.root.path().join("remote/store/loopflow.db")).unwrap();
    local
        .bind_repository(
            repo.path().canonicalize().unwrap().to_str().unwrap(),
            &repository,
        )
        .unwrap();
    remote
        .bind_repository(remote_checkout.to_str().unwrap(), &repository)
        .unwrap();

    // A Claude conversation in the worktree, admitted the way connecting admits it.
    let conversation = "0199a213-81c0-7800-8aa1-bbab2a035a53";
    let transcript = format!(
        "{{\"type\":\"user\",\"sessionId\":\"{conversation}\",\"cwd\":\"{}\",\"message\":{{\"role\":\"user\",\"content\":\"keep going\"}}}}\n",
        worktree.display()
    );
    let project = fixture
        .root
        .path()
        .join("local/.claude/projects")
        .join(claude_project(&worktree));
    fs::create_dir_all(project.join(format!("{conversation}/subagents"))).unwrap();
    fs::write(project.join(format!("{conversation}.jsonl")), &transcript).unwrap();
    fs::write(
        project.join(format!("{conversation}/subagents/agent.jsonl")),
        "{}\n",
    )
    .unwrap();
    let admitted = fixture
        .command(&["session", "resume", conversation, "--prepare", "--json"])
        .current_dir(&worktree)
        .output()
        .unwrap();
    assert_success(&admitted);
    let session = serde_json::from_slice::<Value>(&admitted.stdout).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    fs::write(worktree.join("notes.txt"), "uncommitted\n").unwrap();

    let moved = fixture
        .command(&[
            "--machine",
            "mini",
            "resume",
            &session,
            "--prepare",
            "--json",
        ])
        .current_dir(&worktree)
        .output()
        .unwrap();
    assert_success(&moved);
    let record: Value = serde_json::from_slice(&moved.stdout).unwrap();
    assert_eq!(record["id"], session.as_str());
    assert_eq!(record["workspace"]["machine_id"], mini.as_str());

    // The uncommitted edit reached origin on the Session's branch.
    let pushed = git(repo.bare_path(), &["show", "move-demo:notes.txt"]);
    assert_eq!(pushed, "uncommitted");
    // Mini holds the same Session in a worktree of the branch, with the transcript.
    let arrived = remote.session(&session).unwrap().unwrap();
    assert!(arrived.completed_at.is_none());
    assert_eq!(
        git(&arrived.cwd, &["branch", "--show-current"]),
        "move-demo"
    );
    assert_eq!(
        fs::read_to_string(arrived.cwd.join("notes.txt")).unwrap(),
        "uncommitted\n"
    );
    let placed = fixture
        .root
        .path()
        .join("remote/.claude/projects")
        .join(claude_project(&arrived.cwd.canonicalize().unwrap()));
    assert_eq!(
        fs::read_to_string(placed.join(format!("{conversation}.jsonl"))).unwrap(),
        transcript
    );
    assert!(placed
        .join(format!("{conversation}/subagents/agent.jsonl"))
        .is_file());
    // The laptop's row names where it went, and resuming it here follows it there.
    assert_eq!(
        local.session_moved_to(&session).unwrap().unwrap().as_str(),
        mini
    );
    let followed = fixture
        .command(&["resume", &session, "--prepare", "--json"])
        .current_dir(&worktree)
        .output()
        .unwrap();
    assert_success(&followed);
    let followed: Value = serde_json::from_slice(&followed.stdout).unwrap();
    assert_eq!(followed["workspace"]["machine_id"], mini.as_str());
    assert!(!fixture.root.path().join("forbidden-effect").exists());
}

#[test]
fn a_mid_merge_worktree_refuses_to_move() {
    let fixture = preview_machines();
    let repo = loopflow_test_support::TestRepo::new();
    let worktree = repo
        .create_named_worktree("merging")
        .canonicalize()
        .unwrap();
    let conversation = "0199a213-81c0-7800-8aa1-bbab2a035a54";
    let project = fixture
        .root
        .path()
        .join("local/.claude/projects")
        .join(claude_project(&worktree));
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join(format!("{conversation}.jsonl")),
        format!(
            "{{\"type\":\"user\",\"sessionId\":\"{conversation}\",\"cwd\":\"{}\"}}\n",
            worktree.display()
        ),
    )
    .unwrap();
    let admitted = fixture
        .command(&["session", "resume", conversation, "--prepare", "--json"])
        .current_dir(&worktree)
        .output()
        .unwrap();
    assert_success(&admitted);
    let session = serde_json::from_slice::<Value>(&admitted.stdout).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let head = git(&worktree, &["rev-parse", "HEAD"]);
    fs::write(git_dir(&worktree).join("MERGE_HEAD"), format!("{head}\n")).unwrap();

    let refused = fixture
        .command(&["--machine", "mini", "resume", &session, "--prepare"])
        .current_dir(&worktree)
        .output()
        .unwrap();
    assert!(!refused.status.success());
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(stderr.contains("mid-merge"), "{stderr}");
    assert_eq!(git(&worktree, &["rev-parse", "HEAD"]), head);
    let local =
        loopflow::store::sqlite::SqliteStore::new(&fixture.root.path().join("local/loopflow.db"))
            .unwrap();
    assert!(local.session_moved_to(&session).unwrap().is_none());
}

fn git_dir(worktree: &Path) -> std::path::PathBuf {
    std::path::PathBuf::from(git(worktree, &["rev-parse", "--absolute-git-dir"]))
}

/// A Task the laptop owns, checked out on `branch` in a worktree of the
/// laptop's checkout, with mini knowing it through planning.
fn placed_task(plan: &SharedPlan, branch: &str) -> (loopflow::durable::TaskId, std::path::PathBuf) {
    let task = plan.task();
    plan.to_local();
    let worktree = place(plan, &task, branch);
    (task, worktree)
}

/// Check `task` out on the laptop, owned there.
fn place(plan: &SharedPlan, task: &loopflow::durable::TaskId, branch: &str) -> std::path::PathBuf {
    use loopflow::work::task::{TaskPr, TaskPrId};
    let worktree = plan.local_path.with_file_name(format!(
        "{}.{branch}",
        plan.local_path.file_name().unwrap().to_str().unwrap()
    ));
    git(
        &plan.local_path,
        &[
            "worktree",
            "add",
            "-b",
            branch,
            worktree.to_str().unwrap(),
            "main",
        ],
    );
    let worktree = worktree.canonicalize().unwrap();
    let now = time::OffsetDateTime::now_utc();
    plan.local
        .place_task(
            task,
            &worktree,
            branch,
            &TaskPr {
                id: TaskPrId::new(),
                task_id: task.clone(),
                sequence: 1,
                slug: branch.into(),
                branch: branch.into(),
                base_commit: git(&worktree, &["rev-parse", "HEAD"]),
                parent_pr_id: None,
                publication: None,
                merge_commit: None,
                abandoned_at: None,
                ci_observation: None,
                github_observation: None,
                linear_attachment_id: None,
                linear_comment_id: None,
                linear_link_error: None,
                created_at: now,
                updated_at: now,
            },
        )
        .unwrap();
    if !plan.local.task_owned(task).unwrap() {
        plan.local.claim_task(task).unwrap();
    }
    worktree
}

/// A Claude conversation in `cwd`, admitted the way connecting admits it.
fn claude_conversation(fixture: &Machines, cwd: &Path, conversation: &str) -> String {
    let project = fixture
        .root
        .path()
        .join("local/.claude/projects")
        .join(claude_project(cwd));
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join(format!("{conversation}.jsonl")),
        format!(
            "{{\"type\":\"user\",\"sessionId\":\"{conversation}\",\"cwd\":\"{}\",\"message\":{{\"role\":\"user\",\"content\":\"keep going\"}}}}\n",
            cwd.display()
        ),
    )
    .unwrap();
    let admitted = fixture
        .command(&["session", "resume", conversation, "--prepare", "--json"])
        .current_dir(cwd)
        .output()
        .unwrap();
    assert_success(&admitted);
    serde_json::from_slice::<Value>(&admitted.stdout).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// The Task waits at `explore` of a captured graph it took up.
fn at_explore(db: &Path, task: &loopflow::durable::TaskId) {
    let graph = serde_json::json!({"name": "feature", "steps": [{"Skill": {"skill": {"name": "explore"}, "id": null, "interactive": true, "returns": null, "sources": []}}]});
    let db = rusqlite::Connection::open(db).unwrap();
    db.execute(
        "INSERT INTO task_graphs(task_id,graph,step,updated_at) VALUES(?1,?2,'explore',1)",
        rusqlite::params![task.as_str(), graph.to_string()],
    )
    .unwrap();
}

#[test]
fn task_move_carries_ownership_graph_and_session() {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let mini = plan.remote.local_machine().unwrap().id;
    let (task, worktree) = placed_task(&plan, "move-task");
    at_explore(&fixture.root.path().join("local/loopflow.db"), &task);
    let session = claude_conversation(&fixture, &worktree, "0199a213-81c0-7800-8aa1-bbab2a035a55");
    fs::write(worktree.join("notes.txt"), "uncommitted\n").unwrap();

    let output = fixture
        .command(&["task", "move", task.as_str(), "--machine", "mini", "--json"])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert_success(&output);
    let ownership: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(ownership["owner"]["machine"]["id"], mini.as_str());

    // The worktree's edit reached origin on the Task's branch.
    assert_eq!(
        git(plan._repos[0].bare_path(), &["show", "move-task:notes.txt"]),
        "uncommitted"
    );
    // Mini owns the Task, checked out on its branch, at the same step.
    let owner = plan.remote.task_ownership(&task).unwrap().owner.unwrap();
    assert_eq!(owner.machine.id, Some(mini.clone()));
    let arrived = plan.remote.task(&task).unwrap().unwrap();
    let checkout = arrived.worktree.clone().unwrap();
    assert_eq!(git(&checkout, &["branch", "--show-current"]), "move-task");
    assert_eq!(
        fs::read_to_string(checkout.join("notes.txt")).unwrap(),
        "uncommitted\n"
    );
    let remote_db =
        rusqlite::Connection::open(fixture.root.path().join("remote/store/loopflow.db")).unwrap();
    let step: String = remote_db
        .query_row(
            "SELECT step FROM task_graphs WHERE task_id=?1",
            [task.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(step, "explore");
    // Its conversation came along as the Task's Session, in the Task's checkout.
    let moved = plan.remote.session(&session).unwrap().unwrap();
    assert!(moved.completed_at.is_none());
    assert_eq!(moved.task_id.as_ref(), Some(&task));
    assert_eq!(
        moved.cwd.canonicalize().unwrap(),
        checkout.canonicalize().unwrap()
    );
    // The laptop routes the Task and its conversation to mini.
    assert_eq!(
        plan.local.task_execution_route(&task).unwrap().machine_id,
        mini
    );
    let resumed = fixture
        .command(&["resume", "--task", task.as_str(), "--prepare", "--json"])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert_success(&resumed);
    let resumed: Value = serde_json::from_slice(&resumed.stdout).unwrap();
    assert_eq!(resumed["id"], session.as_str());
    assert_eq!(resumed["workspace"]["machine_id"], mini.as_str());
}

#[test]
fn resume_task_on_another_machine_moves_the_task_there() {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let mini = plan.remote.local_machine().unwrap().id;
    let (task, worktree) = placed_task(&plan, "resume-task");
    at_explore(&fixture.root.path().join("local/loopflow.db"), &task);
    let session = claude_conversation(&fixture, &worktree, "0199a213-81c0-7800-8aa1-bbab2a035a56");

    let resumed = fixture
        .command(&[
            "--machine",
            "mini",
            "resume",
            "--task",
            task.as_str(),
            "--prepare",
            "--json",
        ])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert_success(&resumed);
    let resumed: Value = serde_json::from_slice(&resumed.stdout).unwrap();
    assert_eq!(resumed["id"], session.as_str());
    assert_eq!(resumed["workspace"]["machine_id"], mini.as_str());

    // The same state `lf task move` reaches: ownership, graph and Session on mini.
    let owner = plan.remote.task_ownership(&task).unwrap().owner.unwrap();
    assert_eq!(owner.machine.id, Some(mini.clone()));
    let step: String =
        rusqlite::Connection::open(fixture.root.path().join("remote/store/loopflow.db"))
            .unwrap()
            .query_row(
                "SELECT step FROM task_graphs WHERE task_id=?1",
                [task.as_str()],
                |row| row.get(0),
            )
            .unwrap();
    assert_eq!(step, "explore");
    let moved = plan.remote.session(&session).unwrap().unwrap();
    assert_eq!(moved.task_id.as_ref(), Some(&task));
    assert_eq!(
        plan.local.task_execution_route(&task).unwrap().machine_id,
        mini
    );
}

#[test]
fn task_move_refuses_a_mid_merge_worktree() {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let (task, worktree) = placed_task(&plan, "merging-task");
    let head = git(&worktree, &["rev-parse", "HEAD"]);
    fs::write(git_dir(&worktree).join("MERGE_HEAD"), format!("{head}\n")).unwrap();
    let before = plan.local.task_ownership(&task).unwrap().log.len();

    let refused = fixture
        .command(&["task", "move", task.as_str(), "--machine", "mini"])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert!(!refused.status.success());
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(stderr.contains("mid-merge"), "{stderr}");
    assert_eq!(git(&worktree, &["rev-parse", "HEAD"]), head);
    assert_eq!(plan.local.task_ownership(&task).unwrap().log.len(), before);
}

#[test]
fn a_repeated_task_send_carries_the_conversation_that_already_left() {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let (task, worktree) = placed_task(&plan, "resend-task");
    let session = claude_conversation(&fixture, &worktree, "0199a213-81c0-7800-8aa1-bbab2a035a58");
    let send = || {
        let output = fixture
            .command(&["session", "send", "--task", task.as_str(), "--to", "mini"])
            .current_dir(&plan.local_path)
            .output()
            .unwrap();
        assert_success(&output);
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };

    // The first receive failed; sending again carries the same conversation.
    assert_eq!(send()["session"]["id"], session.as_str());
    let again = send();
    assert_eq!(again["session"]["id"], session.as_str());
    assert!(again["history"]["files"]
        .as_array()
        .is_some_and(|files| !files.is_empty()));
}

#[test]
fn task_move_hands_nothing_off_when_mini_cannot_receive_it() {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let task = plan.local_task();
    let worktree = place(&plan, &task, "stay-task");
    fs::write(worktree.join("notes.txt"), "uncommitted\n").unwrap();
    let before = plan.local.task_ownership(&task).unwrap().log.len();
    let unchanged = || {
        assert_eq!(
            git(&worktree, &["status", "--porcelain"]),
            "?? notes.txt",
            "the worktree stays uncommitted"
        );
        assert_eq!(plan.local.task_ownership(&task).unwrap().log.len(), before);
        assert!(plan.local.task_owned(&task).unwrap());
    };
    let refused = |expected: &str| {
        let output = fixture
            .command(&["task", "move", task.as_str(), "--machine", "mini"])
            .current_dir(&plan.local_path)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(expected), "{stderr}");
    };

    // Mini answers but has never synced the Task.
    refused("unknown on this Machine");
    unchanged();

    // Mini does not answer at all.
    executable(
        &fixture.root.path().join("bin/ssh"),
        "#!/bin/sh\nexit 255\n",
    );
    refused("unreachable");
    unchanged();
}

#[test]
fn task_move_stops_a_live_graph() {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let (task, worktree) = placed_task(&plan, "graph-task");
    // A route whose step runs until something stops it.
    fs::create_dir_all(worktree.join(".lf/graphs")).unwrap();
    fs::write(
        worktree.join(".lf/graphs/hold.yaml"),
        format!("- cmd: task wait {task} --timeout 120s\n- interactive: demo\n"),
    )
    .unwrap();
    let mut run = fixture
        .command(&["-b", "task", "run", task.as_str(), "hold"])
        .current_dir(&worktree)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let local_db = fixture.root.path().join("local/loopflow.db");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        let waiting: i64 = rusqlite::Connection::open(&local_db)
            .unwrap()
            .query_row(
                "SELECT count(*) FROM processes WHERE command LIKE '%\"wait\"%' AND completed_at IS NULL",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if waiting > 0 {
            break;
        }
        if run.try_wait().unwrap().is_some() {
            let output = run.wait_with_output().unwrap();
            panic!(
                "the Task run ended before its step started: {}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        if std::time::Instant::now() >= deadline {
            let rows: Vec<(Option<String>, Option<i64>)> = rusqlite::Connection::open(&local_db)
                .unwrap()
                .prepare("SELECT command,completed_at FROM processes")
                .unwrap()
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            panic!("the step never started: {rows:?}");
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    // What the step had written so far travels in the checkpoint.
    fs::write(worktree.join("step-edit.txt"), "halfway\n").unwrap();

    let moved = fixture
        .command(&["task", "move", task.as_str(), "--machine", "mini"])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    let stopped = (0..100).any(|_| {
        std::thread::sleep(std::time::Duration::from_millis(100));
        run.try_wait().unwrap().is_some()
    });
    if !stopped {
        run.kill().unwrap();
    }
    assert_success(&moved);
    assert!(stopped, "the Task run outlived its stopped Graph");
    assert_eq!(
        git(
            plan._repos[0].bare_path(),
            &["show", "graph-task:step-edit.txt"]
        ),
        "halfway"
    );
    // Mini holds the Task on the same run, which reads as stopped there.
    let (run_start, carrier): (Option<String>, Option<String>) =
        rusqlite::Connection::open(fixture.root.path().join("remote/store/loopflow.db"))
            .unwrap()
            .query_row(
                "SELECT run_start,lf_process_id FROM task_graphs WHERE task_id=?1",
                [task.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
    assert!(run_start.is_some());
    assert!(carrier.is_some());
}

#[test]
fn resume_task_follows_a_task_session_moved_alone() {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let mini = plan.remote.local_machine().unwrap().id;
    let laptop = plan.local.local_machine().unwrap().id;
    let (task, worktree) = placed_task(&plan, "alone-task");
    let session = claude_conversation(&fixture, &worktree, "0199a213-81c0-7800-8aa1-bbab2a035a56");

    let moved = fixture
        .command(&[
            "--machine",
            "mini",
            "resume",
            &session,
            "--prepare",
            "--json",
        ])
        .current_dir(&worktree)
        .output()
        .unwrap();
    assert_success(&moved);
    // The conversation went; the Task stayed.
    assert_eq!(
        plan.local.task_execution_route(&task).unwrap().machine_id,
        laptop
    );
    let resumed = fixture
        .command(&["resume", "--task", task.as_str(), "--prepare", "--json"])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert_success(&resumed);
    let resumed: Value = serde_json::from_slice(&resumed.stdout).unwrap();
    assert_eq!(resumed["id"], session.as_str());
    assert_eq!(resumed["workspace"]["machine_id"], mini.as_str());
}

#[test]
fn a_moved_session_clones_its_repository_where_none_is_checked_out() {
    let fixture = preview_machines();
    let repo = loopflow_test_support::TestRepo::new();
    let worktree = repo
        .create_named_worktree("clone-demo")
        .canonicalize()
        .unwrap();
    let local =
        loopflow::store::sqlite::SqliteStore::new(&fixture.root.path().join("local/loopflow.db"))
            .unwrap();
    local
        .bind_repository(
            repo.path().canonicalize().unwrap().to_str().unwrap(),
            &loopflow::durable::RepositoryId::new(),
        )
        .unwrap();
    let session = claude_conversation(&fixture, &worktree, "0199a213-81c0-7800-8aa1-bbab2a035a57");

    let moved = fixture
        .command(&[
            "--machine",
            "mini",
            "resume",
            &session,
            "--prepare",
            "--json",
        ])
        .current_dir(&worktree)
        .output()
        .unwrap();
    assert_success(&moved);
    let remote = loopflow::store::sqlite::SqliteStore::new(
        &fixture.root.path().join("remote/store/loopflow.db"),
    )
    .unwrap();
    let arrived = remote.session(&session).unwrap().unwrap();
    let src = fixture
        .root
        .path()
        .join("remote/src")
        .canonicalize()
        .unwrap();
    assert!(
        arrived.cwd.canonicalize().unwrap().starts_with(&src),
        "{}",
        arrived.cwd.display()
    );
    assert_eq!(
        git(&arrived.cwd, &["branch", "--show-current"]),
        "clone-demo"
    );
}
