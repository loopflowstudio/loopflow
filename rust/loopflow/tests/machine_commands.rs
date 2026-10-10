//! Public commands with isolated stores and a simulated SSH endpoint.
mod support;
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
    fixture.json(&[
        "machine",
        "add",
        "mini",
        "--repo",
        "project's checkout",
        "--json",
    ]);
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
    let machine = fixture.json(&[
        "machine",
        "add",
        "mini",
        "--repo",
        "project's checkout",
        "--json",
    ]);
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
    let opened = fixture.json(&["--machine", "mini", "session", "connect", &id, "--json"]);
    let argv = opened["open_argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(argv[0], "ssh");
    assert!(argv.contains(&"-tt"));
    assert!(!argv.contains(&"--replace"));
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
fn add_alias_rename_connect_and_remove_preserve_identity() {
    let fixture = Machines::new();
    let added = fixture.json(&[
        "machine",
        "add",
        "mini",
        "--repo",
        "project's checkout",
        "--json",
    ]);
    assert_eq!(added["label"], "mini");
    assert_eq!(added["route"], "mini");
    assert_eq!(added["repo"], "project's checkout");
    assert_eq!(
        fixture.json(&["machine", "list", "mini", "--json"])[0]["id"],
        added["id"]
    );
    let repeated = fixture.json(&[
        "machine",
        "add",
        "mini",
        "--repo",
        "project's checkout",
        "--json",
    ]);
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
    assert!(text.contains("project's checkout"), "{text}");
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
    assert_success(&fixture.run(&["machine", "add", "mini", "--repo", "."]));
}

#[test]
fn status_reports_version_mismatch_and_unreachable_without_prompting() {
    let fixture = Machines::new();
    fixture.remote("0.0.1", "home_11111111111111111111111111111111");
    let output = fixture.run(&["machine", "add", "ssh://jack@mini:2222", "--repo", "."]);
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
    assert!(String::from_utf8_lossy(&connect.stderr).contains("versions differ"));
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
fn unregistered_and_changed_machines_cannot_receive_a_command() {
    let fixture = Machines::new();
    let unregistered = fixture.run(&["--machine", "mini", "session", "list"]);
    assert!(!unregistered.status.success());
    assert!(String::from_utf8_lossy(&unregistered.stderr).contains("not added"));
    assert_success(&fixture.run(&["machine", "add", "mini", "--repo", "."]));
    fixture.remote(
        loopflow::build_info::BUILD_VERSION,
        "home_22222222222222222222222222222222",
    );
    let output = fixture.run(&["--machine", "mini", "session", "list"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("identity changed"));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("remote cwd"));
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
        assert_success(&fixture.run(&["machine", "add", "mini", "--repo", "."]));
        executable(&fixture.root.path().join("bin/ssh"), &format!("#!/bin/sh\ncat >&2 <<'ERROR'\n{diagnostic}\nERROR\nexit 255\n"));
        let status = fixture.json(&["machine", "status", "--json"]);
        let error = status[0]["error"].as_str().unwrap();
        assert!(error.contains(category), "{error}");
        assert!(error.contains("ssh -o ControlPath=none -- 'mini'"), "{error}");
        let add = fixture.run(&["machine", "add", "other", "--repo", "."]);
        assert!(!add.status.success());
        assert!(String::from_utf8_lossy(&add.stderr).contains(category));
        assert_eq!(fixture.json(&["machine", "list", "--json"]).as_array().unwrap().len(), 1);
    }
}

#[test]
fn missing_lf_never_installs_from_status_batch_json_or_nonterminal() {
    let fixture = Machines::new();
    assert_success(&fixture.run(&["machine", "add", "mini", "--repo", "."]));
    fixture.missing_remote();
    let status = fixture.json(&["machine", "status", "--json"]);
    let error = status[0]["error"].as_str().unwrap();
    assert!(error.contains("no lf on the remote"), "{error}");
    assert!(error.contains("install.sh"), "{error}");
    assert!(!fixture
        .run(&["machine", "add", "mini", "--repo", "."])
        .status
        .success());
    assert!(!fixture
        .terminal(&["-b", "machine", "add", "mini", "--repo", "."], "\n")
        .status
        .success());
    assert!(!fixture
        .terminal(&["machine", "add", "mini", "--repo", ".", "--json"], "\n")
        .status
        .success());
    assert!(!fixture.root.path().join("installed").exists());
}

#[test]
fn interactive_add_offers_install_defaults_yes_and_records_the_new_identity() {
    let fixture = Machines::new();
    fixture.missing_remote();
    assert!(!fixture
        .terminal(&["machine", "add", "mini", "--repo", "."], "n\n")
        .status
        .success());
    assert!(!fixture.root.path().join("installed").exists());
    assert_eq!(
        fixture.json(&["machine", "list", "--json"]),
        serde_json::json!([])
    );
    assert_success(&fixture.terminal(&["machine", "add", "mini", "--repo", "."], "\n"));
    assert!(fixture.root.path().join("installed").exists());
    let machines = fixture.json(&["machine", "list", "--json"]);
    assert_eq!(machines[0]["id"], "home_11111111111111111111111111111111");
    fs::remove_file(fixture.root.path().join("installed")).unwrap();
    assert_success(&fixture.terminal(&["machine", "add", "mini", "--repo", "."], "\n"));
    assert!(!fixture.root.path().join("installed").exists());
}

#[test]
fn broken_existing_lf_and_failed_installer_do_not_register_a_machine() {
    let fixture = Machines::new();
    fixture.missing_remote();
    let bin = fixture.root.path().join("remote/.local/bin");
    executable(&bin.join("lf"), "#!/bin/sh\nexit 127\n");
    assert!(!fixture
        .terminal(&["machine", "add", "mini", "--repo", "."], "\n")
        .status
        .success());
    assert!(!fixture.root.path().join("installed").exists());
    fs::remove_file(bin.join("lf")).unwrap();
    executable(&bin.join("curl"), "#!/bin/sh\necho 'exit 1'\n");
    assert!(!fixture
        .terminal(&["machine", "add", "mini", "--repo", "."], "y\n")
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
    let added = fixture.json(&[
        "machine",
        "add",
        "mini",
        "--repo",
        "project's checkout",
        "--json",
    ]);
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
        vec![
            "machine",
            "add",
            "another",
            "--machine",
            "mini",
            "--repo",
            "~/code",
        ],
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
    use base64::Engine;

    let fixture = Machines::new();
    assert_success(&fixture.run(&["machine", "add", "mini", "--repo", "."]));
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
        let selection: Value = serde_json::from_slice(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(selection)
                .unwrap(),
        )
        .unwrap();
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
    assert_success(&fixture.run(&["machine", "add", "mini", "--repo", "."]));
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
    assert_success(&fixture.run(&["machine", "add", "mini", "--repo", "."]));
    let output = fixture.run(&["--machine", "mini", "fail"]);
    assert_eq!(output.status.code(), Some(42));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("Error:"));
    let db = rusqlite::Connection::open(fixture.root.path().join("local/loopflow.db")).unwrap();
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
    assert_success(&fixture.run(&["machine", "add", "mini", "--repo", "."]));
    let added = fixture.json(&[
        "--machine",
        "mini",
        "machine",
        "add",
        "third",
        "--repo",
        "~/code",
        "--json",
    ]);
    assert_eq!(added["label"], "third");
    assert_eq!(added["repo"], "~/code");
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
    assert_success(&fixture.run(&["machine", "add", "mini", "--repo", "."]));
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
        assert!(report["input"]["system_prompt"]
            .as_str()
            .unwrap()
            .contains("remote-only marker"));
        assert!(report["input"]["system_prompt"]
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
    support::record_flow(
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
fn flow_context_preview_reads_selected_machine_graph_without_executing_steps() {
    let fixture = preview_machines();
    let remote = fixture.root.path().join("remote");
    let repo = remote.join("projects/preview");
    fs::create_dir_all(repo.join(".lf/skills")).unwrap();
    fs::create_dir_all(repo.join(".lf/flows")).unwrap();
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
        repo.join(".lf/flows/repair.yaml"),
        "- work\n- loop: work\n  step: decide\n",
    )
    .unwrap();
    fs::write(repo.join(".lf/flows/probe.yaml"), "- xor:\n    router: route\n    paths:\n      repair:\n        description: Repair the work\n        flow: repair\n      skip:\n        description: Skip repairs\n- cmd: config set repo_root /must-not-write\n- work\n").unwrap();
    fs::write(
        repo.join(".lf/flows/command-first.yaml"),
        "- cmd: config set repo_root /must-not-write\n- work\n",
    )
    .unwrap();
    fs::write(repo.join(".lf/flows/invalid.yaml"), "- absent-skill\n").unwrap();
    let local_db = fixture.root.path().join("local/loopflow.db");
    let remote_db = remote.join("store/loopflow.db");
    let before = [checkpoint_bytes(&local_db), checkpoint_bytes(&remote_db)];
    let args = [
        "--machine",
        "mini",
        "--repo",
        "preview",
        "flow",
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
    let graph = &input["graph"];
    assert_eq!(graph["steps"][0]["kind"], "xor");
    assert_eq!(graph["steps"][0]["paths"][0]["name"], "repair");
    assert_eq!(graph["steps"][0]["paths"][1]["name"], "skip");
    let nested = &graph["steps"][0]["paths"][0]["steps"];
    assert_eq!(nested[1]["returns_to"], nested[0]["key"]);
    let inputs = input["inputs"].as_array().unwrap();
    assert_eq!(inputs.len(), 5);
    assert_eq!(inputs[0]["state"], "current");
    let prompt = inputs[0]["input"]["system_prompt"].as_str().unwrap();
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
        "flow",
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
        "flow",
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

#[test]
fn imported_execution_location_survives_delegation_and_rejects_stale_peer_reads() {
    use loopflow::durable::{RepositoryId, TaskId};
    use loopflow::engine::planning_git::PlanningDestination;
    use loopflow::planning::NewTask;
    use loopflow::store::sqlite::SqliteStore;

    let fixture = preview_machines();
    let local_repo = loopflow_test_support::TestRepo::new();
    let remote_repo = loopflow_test_support::TestRepo::new();
    let local_path = local_repo.path().canonicalize().unwrap();
    let remote_path = remote_repo.path().canonicalize().unwrap();
    let local_db = fixture.root.path().join("local/loopflow.db");
    let remote_db = fixture.root.path().join("remote/store/loopflow.db");
    let local = SqliteStore::new(&local_db).unwrap();
    let remote = SqliteStore::new(&remote_db).unwrap();
    let repository = RepositoryId::new();
    local
        .bind_repository(local_path.to_str().unwrap(), &repository)
        .unwrap();
    remote
        .bind_repository(remote_path.to_str().unwrap(), &repository)
        .unwrap();
    let project = remote
        .ensure_wave_project(remote_path.to_str().unwrap(), "location")
        .unwrap();
    let task = remote
        .create_task(&NewTask {
            id: TaskId::new(),
            project_id: project.id,
            title: "Retained elsewhere".into(),
            description: String::new(),
            due_date: None,
        })
        .unwrap();
    let owner = remote.local_machine().unwrap().id;
    // Execution is deliberately seeded, not imported or claimed by this proof.
    support::record_flow(
        &fixture.root.path().join("remote/store"),
        &remote_path,
        "prior",
        "prior-step",
        "succeeded",
    );
    let db = rusqlite::Connection::open(&remote_db).unwrap();
    db.execute("UPDATE tasks SET worktree=?2,workspace_slug='retained',branch='main',base_commit=?3,checkout_machine_id=?4,started_at=1 WHERE id=?1",
        rusqlite::params![task.id.as_str(), remote_path.to_str().unwrap(), remote_repo.head_sha(), owner.as_str()]).unwrap();
    let destination =
        PlanningDestination::new("/unused/fixture", "refs/loopflow/planning/shared/location")
            .unwrap();
    for (store, path) in [(&local, &local_path), (&remote, &remote_path)] {
        store
            .bind_peer_planning(path.to_str().unwrap(), &destination)
            .unwrap();
    }
    remote
        .select_peer_waves(
            remote_path.to_str().unwrap(),
            &destination.id(),
            std::slice::from_ref(&task.wave_id),
        )
        .unwrap();
    local
        .import_peer_planning(
            local_path.to_str().unwrap(),
            &destination.id(),
            "first",
            &remote
                .export_peer_planning(remote_path.to_str().unwrap(), &destination.id())
                .unwrap(),
        )
        .unwrap();
    let caller = local.local_machine().unwrap().id;
    assert_success(&fixture.run(&[
        "--repository",
        repository.as_str(),
        "wave",
        "place",
        task.wave_id.as_str(),
        caller.as_str(),
    ]));
    remote
        .import_peer_planning(
            remote_path.to_str().unwrap(),
            &destination.id(),
            "delegated-back",
            &local
                .export_peer_planning(local_path.to_str().unwrap(), &destination.id())
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        db.query_row(
            "SELECT machine_id FROM work_placements WHERE wave_id=?1",
            [task.wave_id.as_str()],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        caller.as_str()
    );
    assert_eq!(
        remote.task_execution_route(&task.id).unwrap().machine_id,
        owner
    );
    assert!(local.task(&task.id).unwrap().unwrap().worktree.is_none());
    let local_before = checkpoint_bytes(&local_db);
    let remote_before = checkpoint_bytes(&remote_db);
    let args = [
        "--repository",
        repository.as_str(),
        "task",
        "location",
        task.id.as_str(),
        "--peers",
        "--json",
    ];
    let reading = fixture.json(&args);
    assert_eq!(reading[0]["location"]["state"], "unrecorded");
    assert_eq!(reading[1]["location"]["state"], "recorded");
    assert_eq!(reading[1]["machine_id"], owner.as_str());
    assert_eq!(
        reading[1]["location"]["checkout"],
        remote_path.to_str().unwrap()
    );
    assert_eq!(checkpoint_bytes(&local_db), local_before);
    assert_eq!(checkpoint_bytes(&remote_db), remote_before);
    let explain = |command: &[&str]| {
        let mut args = vec![
            "--repository",
            repository.as_str(),
            "--task",
            task.id.as_str(),
        ];
        args.extend_from_slice(command);
        args.extend(["--explain", "--json"]);
        fixture.json(&args)
    };
    // Every presentation uses the same observed owner and checkout provenance.
    // Started history without a path is unavailable, not an unstarted Task or a
    // reason to retain the previous checkout reading.
    for checkout in [Some(remote_path.to_str().unwrap()), None] {
        db.execute(
            "UPDATE tasks SET worktree=?2 WHERE id=?1",
            rusqlite::params![task.id.as_str(), checkout],
        )
        .unwrap();
        let remote_before = checkpoint_bytes(&remote_db);
        let run = explain(&["task", "run"]);
        assert_eq!(
            run["resolution"]["execution_machine"]["value"],
            owner.as_str()
        );
        assert_eq!(
            run["resolution"]["execution_machine"]["source"],
            "peer_recorded_checkout"
        );
        match checkout {
            Some(path) => assert_eq!(run["resolution"]["checkout"]["value"], path),
            None => assert_eq!(run["resolution"]["checkout"]["state"], "unavailable"),
        }
        assert!(run["action"].is_null());
        for command in [
            vec!["task", "checkout"],
            vec!["task", "move", task.id.as_str(), "start"],
            vec!["desktop", "open"],
        ] {
            let report = explain(&command);
            for fact in ["execution_machine", "checkout"] {
                assert_eq!(report["resolution"][fact], run["resolution"][fact]);
            }
            if command[0] == "desktop" {
                if checkout.is_some() {
                    let url = reqwest::Url::parse(report["url"].as_str().unwrap()).unwrap();
                    let query = url
                        .query_pairs()
                        .collect::<std::collections::BTreeMap<_, _>>();
                    assert_eq!(query["machine"], owner.as_str());
                    assert_eq!(query["repository"], repository.as_str());
                    assert_eq!(query["repo"], local_path.to_str().unwrap());
                } else {
                    assert!(report["url"].is_null());
                    assert!(report["impediments"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|value| value
                            .as_str()
                            .unwrap()
                            .contains("Recorded checkout unavailable")));
                }
            } else {
                assert!(report["action"].is_null());
            }
        }
        assert_eq!(checkpoint_bytes(&local_db), local_before);
        assert_eq!(checkpoint_bytes(&remote_db), remote_before);
    }
    db.execute(
        "UPDATE tasks SET worktree=?2 WHERE id=?1",
        rusqlite::params![task.id.as_str(), remote_path.to_str().unwrap()],
    )
    .unwrap();
    assert!(!fixture.root.path().join("forbidden-effect").exists());

    let checkout = fixture.json(&[
        "--repository",
        repository.as_str(),
        "task",
        "checkout",
        task.id.as_str(),
        "--json",
    ]);
    assert_eq!(checkout["worktree"], remote_path.to_str().unwrap());
    assert!(local.task(&task.id).unwrap().unwrap().worktree.is_none());
    assert!(!fixture.root.path().join("forbidden-effect").exists());
    let local_before = checkpoint_bytes(&local_db);
    let remote_before = checkpoint_bytes(&remote_db);

    // Replaying yesterday's successful reply cannot masquerade as a new reading.
    let replay = fixture.root.path().join("replay.json");
    fs::write(
        &replay,
        serde_json::to_vec(&vec![reading[1].clone()]).unwrap(),
    )
    .unwrap();
    executable(
        &fixture.root.path().join("bin/ssh"),
        &format!("#!/bin/sh\ncat '{}'\n", replay.display()),
    );
    let stale = fixture.json(&args);
    assert_eq!(stale[1]["location"]["state"], "unavailable");
    assert!(stale[1]["location"]["reason"]
        .as_str()
        .unwrap()
        .contains("stale or mismatched"));
    for command in [
        vec!["task", "run"],
        vec!["task", "checkout"],
        vec!["desktop", "open"],
    ] {
        let report = explain(&command);
        for fact in ["execution_machine", "checkout"] {
            assert_eq!(report["resolution"][fact]["state"], "unavailable");
        }
        assert!(report["action"].is_null());
        assert!(report["url"].is_null());
    }

    // A peer reply represents exactly one local observer, never a peer inventory.
    for readings in [vec![], vec![reading[1].clone(); 2]] {
        fs::write(&replay, serde_json::to_vec(&readings).unwrap()).unwrap();
        let malformed = fixture.json(&args);
        assert_eq!(malformed[1]["location"]["state"], "unavailable");
        assert!(malformed[1]["location"]["reason"]
            .as_str()
            .unwrap()
            .contains("one local execution-location reading"));
    }

    executable(
        &fixture.root.path().join("bin/ssh"),
        "#!/bin/sh\necho unreachable >&2\nexit 255\n",
    );
    let unreachable = fixture.json(&args);
    assert_eq!(unreachable[1]["location"]["state"], "unavailable");
    assert_eq!(checkpoint_bytes(&local_db), local_before);
    assert_eq!(checkpoint_bytes(&remote_db), remote_before);
    // Actual preparation refuses before a local checkout or provider can appear.
    let launch = fixture.run(&[
        "--repository",
        repository.as_str(),
        "task",
        "checkout",
        task.id.as_str(),
        "--json",
    ]);
    assert!(!launch.status.success());
    assert!(
        String::from_utf8_lossy(&launch.stderr).contains("first-start admission is unavailable")
    );
    assert!(local.task(&task.id).unwrap().unwrap().worktree.is_none());
    assert!(!fixture.root.path().join("forbidden-effect").exists());
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
    assert!(first.changed);
    // An atomic replacement must invalidate even when dirty status stays dirty.
    repo.create_file("replacement", "new peer contents\n");
    fs::rename(path.join("replacement"), path.join("note.txt")).unwrap();
    let mut changed = false;
    for _ in 0..5 {
        if read().changed {
            changed = true;
            break;
        }
    }
    // Closing the reader's stdin must drain the remote observer, not leave it resident.
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert_success(&output);
    assert!(changed);
    assert!(!fixture.root.path().join("forbidden-effect").exists());
}
