//! Public commands with isolated stores and a simulated SSH endpoint.
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
exec env -i HOME='{}' PATH=/usr/bin:/bin bash -c "$remote_command"
"#,
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
