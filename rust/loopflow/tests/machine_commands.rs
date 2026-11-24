//! Public commands with isolated stores and a simulated SSH endpoint.
use crate::support;
use std::fs;
use std::io::Write;
use std::os::fd::FromRawFd;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::Value;

struct Machines {
    root: tempfile::TempDir,
    _person: support::ambient::EnvGuard,
    _env: support::EnvGuard,
}

impl Machines {
    fn new() -> Self {
        let fixture = Self::uninitialized();
        support::seed_store(&fixture.root.path().join("local"));
        fixture
    }

    fn uninitialized() -> Self {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("local");
        let remote = root.path().join("remote");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(remote.join("bin")).unwrap();
        fs::create_dir_all(remote.join(".local/bin")).unwrap();
        fs::create_dir_all(remote.join("project's checkout")).unwrap();
        fs::create_dir_all(root.path().join("bin")).unwrap();
        // Put transport doubles on the incoming PATH. The remote preamble
        // appends .local/bin to find lf, after the system tools.
        executable(
            &root.path().join("bin/ssh"),
            &format!(
                r#"#!/bin/sh
for arg in "$@"; do remote_command="$arg"; done
cd '{remote}' || exit 1
exec env -i HOME='{remote}' PATH='{remote}/bin:/usr/bin:/bin' bash -c "$remote_command"
"#,
                remote = remote.display(),
            ),
        );
        // Credential side effects stay inside the fixture even on macOS.
        for program in ["gh", "security", "doppler"] {
            executable(
                &root.path().join("bin").join(program),
                "#!/bin/sh\nexit 1\n",
            );
        }
        let env = support::EnvGuard::with_home(&[], Some(&home));
        let person = support::ambient::EnvGuard::clear(&["LF_USER_NAME"]);
        std::env::set_var("LF_HOME", &home);
        std::env::set_var("LF_USER_NAME", "Jack");
        let fixture = Self {
            root,
            _person: person,
            _env: env,
        };
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
            &self.root.path().join("remote/bin/curl"),
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
fn modal_ondemand_list_records_the_sizing_mode_before_any_rental() {
    let fixture = Machines::new();
    for (name, flags) in [("burst", &[][..]), ("reserved", &["--reserved"][..])] {
        let mut args = vec![
            "ondemand",
            "add",
            name,
            "--provider",
            "modal",
            "--vcpus",
            "8",
            "--memory-gb",
            "16",
            "--max",
            "1",
            "--max-usd-per-hour",
            "1.00",
        ];
        args.extend_from_slice(flags);
        assert_success(&fixture.run(&args));
    }

    let listings = fixture.json(&["ondemand", "list", "--json"]);
    let listings = listings.as_array().unwrap();
    assert_eq!(listings.len(), 2);
    for listing in listings {
        assert_eq!(listing["hardware"]["modal_sizing"], listing["name"]);
        assert!(listing["rentals"].as_array().unwrap().is_empty());
    }
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
        "stty -echo; printf '%s\\n' \"$LF_HOME\" \"$1\"; read -r line; printf '%s\\n' \"$line\"",
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
        "--unfocused",
        "--prepare",
        "--json",
    ]);
    let argv = opened["open_argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(argv[0], env!("CARGO_BIN_EXE_lf"));
    assert!(argv.contains(&"-T"));
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
    assert_eq!(&argv[1..4], &["session", "attach", "--"]);
    let script = argv.last().unwrap();
    assert!(script.contains("LF_VIEWER_STREAM='1'"), "{script}");
    assert!(script.contains("/remote/private home"), "{script}");
    assert!(!script.contains("session remote"), "{script}");
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
    assert!(String::from_utf8_lossy(&set.stdout).contains(".lf/config.local.yaml"));

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
        .position(|arg| *arg == "--repo-id")
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
        .query_row(
            "SELECT id FROM machines WHERE id=?1",
            [added["id"].as_str().unwrap()],
            |row| row.get(0),
        )
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
fn status_reads_the_same_build_as_the_same_version() {
    let fixture = Machines::new();
    // `lf --version` prints the build version with its label.
    fixture.remote(
        loopflow::build_info::VERSION_LINE,
        "home_11111111111111111111111111111111",
    );
    assert_success(&fixture.run(&["machine", "add", "mini"]));
    let output = fixture.run(&["machine", "status", "mini"]);
    assert_success(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("versions differ"), "{stderr}");
    let statuses = fixture.json(&["machine", "status", "mini", "--json"]);
    assert_eq!(
        statuses[0]["remote_version"],
        loopflow::build_info::BUILD_VERSION
    );
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
    let fixture = Machines::uninitialized();
    let unregistered = fixture.run(&["--machine", "mini", "session", "list"]);
    assert!(!unregistered.status.success());
    let error = String::from_utf8_lossy(&unregistered.stderr);
    assert!(error.contains("not added"), "{error}");
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
    // Each diagnostic's recovery is a row in `ssh::dispatch_tests`.
    let fixture = Machines::new();
    assert_success(&fixture.run(&["machine", "add", "mini"]));
    executable(
        &fixture.root.path().join("bin/ssh"),
        "#!/bin/sh\necho 'Permission denied (publickey).' >&2\nexit 255\n",
    );
    let status = fixture.json(&["machine", "status", "--json"]);
    let error = status[0]["error"].as_str().unwrap();
    assert!(error.contains("needs-sign-in"), "{error}");
    assert!(error.contains("Permission denied (publickey)."), "{error}");
    let add = fixture.run(&["machine", "add", "other"]);
    assert!(!add.status.success());
    assert!(String::from_utf8_lossy(&add.stderr).contains("needs-sign-in"));
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
    let attempted = fixture.root.path().join("installer-attempted");
    executable(
        &fixture.root.path().join("remote/bin/curl"),
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"touch '{}'\" 'exit 1'\n",
            attempted.display()
        ),
    );
    assert!(!fixture
        .terminal(&["machine", "add", "mini"], "y\n")
        .status
        .success());
    assert!(attempted.exists());
    assert_eq!(
        fixture.json(&["machine", "list", "--json"]),
        serde_json::json!([])
    );
}

#[test]
fn selecting_the_current_machine_executes_locally() {
    let fixture = Machines::new();
    let local = fixture.json(&["machine", "id", "--json"]);
    let output = fixture.run(&[
        "--machine",
        local["id"].as_str().unwrap(),
        "config",
        "catalog",
        "--json",
    ]);
    assert_success(&output);
    let definitions: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(definitions
        .as_array()
        .unwrap()
        .iter()
        .any(|definition| definition["key"] == "machine"));
}

#[test]
fn remote_task_promotion_needs_no_provider_account() {
    let fixture = Machines::new();
    assert_success(&fixture.run(&["machine", "add", "mini"]));
    let prefix = ["--machine", "mini"];
    for operation in [
        vec![
            "task",
            "create",
            "--title",
            "Prototype",
            "--session",
            "remote-session",
        ],
        vec!["session", "bind", "remote-session", "--task", "LOO-12"],
    ] {
        let args = [prefix.as_slice(), operation.as_slice()].concat();
        let output = fixture.run(&args);
        assert_success(&output);
        let stdout = String::from_utf8_lossy(&output.stdout);
        for arg in operation {
            assert!(stdout.contains(&format!("argument: {arg}\n")), "{stdout}");
        }
    }
}

#[test]
fn remote_command_exit_reaches_the_local_caller() {
    let fixture = Machines::new();
    assert_success(&fixture.run(&["machine", "add", "mini"]));
    let output = fixture
        .command(&["--machine", "mini", "fail"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(42));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("Error:"));
}

#[test]
fn machine_add_on_the_remote_updates_only_its_registry() {
    let fixture = Machines::new();
    let remote = fixture.root.path().join("remote");
    support::seed_store(&remote.join("store"));
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
        &remote.join("bin/ssh"),
        &format!(
            "#!/bin/sh\nprintf 'lf {}\\nhome_33333333333333333333333333333333\\n'\n",
            loopflow::build_info::BUILD_VERSION
        ),
    );
    let mini = fixture.json(&["machine", "add", "mini", "--json"]);
    let added = fixture.json(&["--machine", "mini", "machine", "add", "third", "--json"]);
    // The new alias belongs to the executor's saved profile. Direct remote work
    // sees it; a forwarded request continues using the origin's declarations.
    let output = fixture
        .command(&["machine", "list", "--json"])
        .env("HOME", &remote)
        .env("LF_HOME", remote.join("store"))
        .output()
        .unwrap();
    assert_success(&output);
    let remote_machines: Value = serde_json::from_slice(&output.stdout).unwrap();
    let remote_machines = remote_machines.as_array().unwrap();
    assert_eq!(remote_machines.len(), 2);
    for (label, id) in [("mini", &mini["id"]), ("third", &added["id"])] {
        assert!(remote_machines
            .iter()
            .any(|machine| machine["label"] == label && &machine["id"] == id));
    }
    let local = fixture.json(&["machine", "list", "--json"]);
    assert_eq!(local.as_array().unwrap().len(), 1);
    assert_eq!(local[0]["label"], "mini");
}

/// Two actual CLIs joined by the fixture's local-only SSH substitute.
fn preview_machines() -> Machines {
    let fixture = Machines::new();
    let remote = fixture.root.path().join("remote");
    support::seed_store(&remote.join("store"));
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
    assert_success(&fixture.run(&["set", "--global", "default_agent", "codex"]));
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
    loopflow_execution::config::files::update(
        &fixture.root.path().join("local/config.yaml"),
        |value| {
            value["repo_root"] = "/not-the-remote-root".into();
            Ok(())
        },
    )
    .unwrap();
    let local_db = fixture.root.path().join("local/loopflow.db");
    let remote_db = remote.join("store/loopflow.db");
    let local_before = checkpoint_bytes(&local_db);
    let remote_before = checkpoint_bytes(&remote_db);
    let expected_id: String = rusqlite::Connection::open(&remote_db)
        .unwrap()
        .query_row("SELECT id FROM machines WHERE is_local=1", [], |row| {
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
                "skill",
                "probe",
                "literal 'draft'; $(false)",
                "--context",
                "--explain",
                "--json",
            ])
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
            "UPDATE machines SET id='home_99999999999999999999999999999999' WHERE is_local=1",
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
fn task_run_open_resolves_remote_selectors_before_opening_locally() {
    let fixture = Machines::new();
    assert_success(&fixture.run(&["machine", "add", "mini"]));
    let home = fixture.root.path().join("local");
    let here = loopflow_test_support::TestRepo::new();
    let selected = loopflow_test_support::TestRepo::new();
    for repo in [&here, &selected] {
        support::bind_task_planning(repo);
        repo.create_branch("opening");
    }
    let local = support::register_task_without_pr(
        &home,
        &here.path().canonicalize().unwrap(),
        "opening",
        &here.head_sha(),
    );
    let target = support::register_task_without_pr(
        &home,
        &selected.path().canonicalize().unwrap(),
        "opening",
        &selected.head_sha(),
    );
    assert_ne!(local.task.id, target.task.id);
    // Both repositories call their Task INF-123. The local cwd selects the
    // other Task; the remote response must carry the exact identity back.
    let db = rusqlite::Connection::open(home.join("loopflow.db")).unwrap();
    db.execute(
        "INSERT INTO agent_sessions(
            id,title,title_source,created_at,input_published,interactive,task_id,wave_id,cwd,provider
         ) VALUES('remote-selected-primary','Task conversation','generated',unixepoch(),1,1,?1,?2,?3,'claude')",
        rusqlite::params![
            target.task.id.as_str(),
            target.task.wave_id.as_ref().unwrap().as_str(),
            selected.path().to_str().unwrap(),
        ],
    )
    .unwrap();
    db.execute(
        "INSERT OR REPLACE INTO task_primary_sessions(task_id,session_id) VALUES(?1,'remote-selected-primary')",
        [target.task.id.as_str()],
    )
    .unwrap();
    db.execute(
        "UPDATE agent_sessions SET task_id=?1,wave_id=(SELECT p.wave_id FROM tasks t LEFT JOIN projects p ON p.id=t.project_id WHERE t.id=?1) WHERE id='remote-selected-primary'",
        [target.task.id.as_str()],
    )
    .unwrap();
    let resolution = fixture
        .command(&["--task", target.task.id.as_str(), "--explain", "--json"])
        .current_dir(selected.path())
        .output()
        .unwrap();
    assert_success(&resolution);
    let resolution: Value = serde_json::from_slice(&resolution.stdout).unwrap();
    assert_eq!(resolution["task"]["value"], target.task.id.as_str());
    let remote = fixture.root.path().join("remote");
    fs::write(
        remote.join("selection.json"),
        serde_json::to_vec(&serde_json::json!({
            "resolution": resolution, "action": null, "impediments": [], "unavailable": []
        }))
        .unwrap(),
    )
    .unwrap();
    // Simulate the remote command at the SSH boundary. These paths and
    // worktree names deliberately do not exist on the presentation Machine.
    executable(
        &remote.join(".local/bin/lf"),
        &format!(
            r#"#!/bin/sh
case "$1 $2" in
  '--version ') echo 'lf {version}'; exit 0;;
  'machine id') echo 'home_11111111111111111111111111111111'; exit 0;;
esac
case " $* " in
  *' --explain '*)
    test -e "$HOME/launched" || exit 91
    test ! -e "$HOME/read-fails" || exit 93
    cat "$HOME/selection.json";;
  *' task run '*)
    touch "$HOME/launched"
    echo 'Task INF-123 waits at review';;
  *) exit 92;;
esac
"#,
            version = loopflow::build_info::BUILD_VERSION
        ),
    );
    let attach = home.join("lf-attach");
    executable(
        &attach,
        &format!(
            r#"#!/bin/sh
if [ "$1 $2" = 'session resume' ]; then
  printf '%s\n' "$3" > "$LF_HOME/opened"
  pwd -P > "$LF_HOME/opened-cwd"
  exit 0
fi
exec '{}' "$@"
"#,
            env!("CARGO_BIN_EXE_lf")
        ),
    );
    for selection in [
        vec!["task", "run", "INF-123", "--open"],
        vec!["task", "run", "--open"],
        vec!["task", "run", "--open", "--json"],
        vec!["task", "run", "--open", "--", "INF-123"],
        vec!["--cwd", "/remote-only/checkout", "task", "run", "--open"],
        vec!["--wt", "remote-worktree", "task", "run", "--open"],
    ] {
        let mut args = vec!["--machine", "mini", "--repo", "/remote-only/repo"];
        args.extend(selection);
        let output = fixture
            .command(&args)
            .current_dir(here.path())
            .env("LF_BIN", &attach)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_success(&output);
        assert_eq!(
            fs::read_to_string(home.join("opened"))
                .unwrap_or_else(|error| panic!("{args:?}: {error}; {output:?}"))
                .trim(),
            "remote-selected-primary"
        );
        assert_eq!(
            fs::read_to_string(home.join("opened-cwd")).unwrap().trim(),
            selected.path().canonicalize().unwrap().to_str().unwrap()
        );
        fs::remove_file(home.join("opened")).unwrap();
        fs::remove_file(remote.join("launched")).unwrap();
    }
    fs::write(remote.join("read-fails"), "").unwrap();
    let output = fixture
        .command(&[
            "--machine",
            "mini",
            "--repo",
            "/remote-only/repo",
            "task",
            "run",
            "--open",
        ])
        .current_dir(here.path())
        .env("LF_BIN", &attach)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(remote.join("launched").exists());
    assert!(!home.join("opened").exists());
    assert!(String::from_utf8_lossy(&output.stderr).contains("selection could not be read"));
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
    loopflow_execution::config::files::update(
        &fixture.root.path().join("local/config.yaml"),
        |value| {
            value["repo_root"] = "/caller-only".into();
            Ok(())
        },
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
        "- loop:\n    - work\n    - decide\n",
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

/// Two Homes share one repository plan, the remote one as the hub; ownership
/// entries travel as marked comments and every append runs on the Task's writer.
struct SharedPlan {
    repository: loopflow_execution::durable::RepositoryId,
    local: loopflow_execution::store::sqlite::SqliteStore,
    remote: loopflow_execution::store::sqlite::SqliteStore,
    local_path: std::path::PathBuf,
    remote_path: std::path::PathBuf,
    _repos: Vec<loopflow_test_support::TestRepo>,
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
        use loopflow_execution::durable::RepositoryId;

        let local =
            loopflow_execution::store::open_sqlite(&fixture.root.path().join("local/loopflow.db"))
                .unwrap();
        let remote = loopflow_execution::store::open_sqlite(
            &fixture.root.path().join("remote/store/loopflow.db"),
        )
        .unwrap();
        let repository = RepositoryId::new();
        loopflow_execution::repository::location::bind_repository(
            &local,
            local_path.to_str().unwrap(),
            &repository,
        )
        .unwrap();
        loopflow_execution::repository::location::bind_repository(
            &remote,
            remote_path.to_str().unwrap(),
            &repository,
        )
        .unwrap();
        Self {
            repository,
            local,
            remote,
            local_path,
            remote_path,
            _repos: repos,
        }
    }

    /// One exchange as the local Machine makes it with the remote, its hub.
    fn exchange(&self) {
        loopflow_storage::test_fixture::exchange(
            &self.local,
            self.local_path.to_str().unwrap(),
            &self.remote,
            self.remote_path.to_str().unwrap(),
        );
    }

    fn to_local(&self) {
        self.exchange();
    }

    fn to_remote(&self) {
        self.exchange();
    }

    fn task(&self) -> loopflow_execution::durable::TaskId {
        Self::create_task(&self.remote, &self.remote_path)
    }

    /// A Task only the laptop knows: mini never synced its planning.
    fn local_task(&self) -> loopflow_execution::durable::TaskId {
        Self::create_task(&self.local, &self.local_path)
    }

    fn create_task(
        store: &loopflow_execution::store::sqlite::SqliteStore,
        path: &Path,
    ) -> loopflow_execution::durable::TaskId {
        let project = loopflow_execution::work::wave::bootstrap::ensure_wave_project(
            store,
            path.to_str().unwrap(),
            "owners",
        )
        .unwrap();
        store
            .create_task(&loopflow_execution::work::task::NewTask {
                parent_task_id: None,
                repository_id: loopflow_execution::repository::location::ensure_repository(
                    store,
                    path.to_str().unwrap(),
                )
                .unwrap(),
                id: loopflow_execution::durable::TaskId::new(),
                project_id: Some(project.id),
                title: "Ownership demo".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap()
            .id
    }
}

/// Replace the fixture's `ssh` with a relay to mini's `lf server connect`
/// that `flags/down` makes unreachable and `flags/drop-apply` makes lose the
/// answer to one `plan.apply` after mini has written it.
fn hub_ssh(fixture: &Machines, flags: &Path) {
    fs::create_dir_all(flags).unwrap();
    let remote = fixture.root.path().join("remote");
    executable(
        &fixture.root.path().join("bin/ssh"),
        &format!(
            r#"#!/usr/bin/python3
import os, subprocess, sys, threading
flags, remote = {flags:?}, {remote:?}
if os.path.exists(os.path.join(flags, "down")):
    sys.stderr.write("ssh: connect to host mini port 22: Operation timed out\n")
    sys.exit(255)
child = subprocess.Popen(["bash", "-c", sys.argv[-1]], cwd=remote,
    env={{"HOME": remote, "PATH": "/usr/bin:/bin"}},
    stdin=subprocess.PIPE, stdout=subprocess.PIPE)
drop = os.path.join(flags, "drop-apply")
applying = threading.Event()
def up():
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            break
        if os.path.exists(drop) and b'"plan.apply"' in line:
            applying.set()
        child.stdin.write(line)
        child.stdin.flush()
    child.stdin.close()
threading.Thread(target=up, daemon=True).start()
while True:
    line = child.stdout.readline()
    if not line:
        break
    if applying.is_set():
        os.remove(drop)
        child.kill()
        os._exit(255)
    sys.stdout.buffer.write(line)
    sys.stdout.buffer.flush()
sys.exit(child.wait())
"#,
            flags = flags.to_str().unwrap(),
            remote = remote.to_str().unwrap(),
        ),
    );
}

/// Another person's Machine: its own home, identity, store and clone of a
/// bare remote. It shares the fixture's stand-in `ssh`.
struct Home {
    dir: std::path::PathBuf,
    bin: std::path::PathBuf,
}

impl Home {
    fn cloned(fixture: &Machines, name: &str, email: &str, person: &str, bare: &Path) -> Self {
        let dir = fixture.root.path().join(name);
        support::seed_store(&dir);
        fs::write(
            dir.join(".gitconfig"),
            format!("[user]\n\temail = {email}\n\tname = {person}\n"),
        )
        .unwrap();
        git(
            fixture.root.path(),
            &[
                "clone",
                "-q",
                bare.to_str().unwrap(),
                dir.join("checkout").to_str().unwrap(),
            ],
        );
        Self {
            dir,
            bin: fixture.root.path().join("bin"),
        }
    }

    fn checkout(&self) -> std::path::PathBuf {
        self.dir.join("checkout")
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_lf"))
            .env_clear()
            .env("HOME", &self.dir)
            .env("LF_HOME", &self.dir)
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
            .env("PATH", format!("{}:/usr/bin:/bin", self.bin.display()))
            .current_dir(self.checkout())
            .stdin(Stdio::null())
            .args(args)
            .output()
            .unwrap()
    }

    fn ok(&self, args: &[&str]) -> String {
        let output = self.run(args);
        assert_success(&output);
        String::from_utf8(output.stdout).unwrap()
    }

    fn store(&self) -> loopflow_execution::store::sqlite::SqliteStore {
        loopflow_execution::store::open_sqlite(&self.dir.join("loopflow.db")).unwrap()
    }
}

/// LOO-938's acceptance over the real carriers: three homes clone one bare
/// remote and meet through its plan refs; the laptop then names mini its hub
/// and reaches it with `ssh mini lf server connect`.
#[test]
fn the_later_change_shows_on_both_machines_through_the_hub() {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let flags = fixture.root.path().join("ssh-flags");
    hub_ssh(&fixture, &flags);
    let laptop = |args: &[&str]| {
        fixture
            .command(args)
            .current_dir(&plan.local_path)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    };
    let ok = |args: &[&str]| {
        let output = laptop(args);
        assert_success(&output);
        String::from_utf8(output.stdout).unwrap()
    };
    let bare = plan._repos[0].bare_path();
    let third = Home::cloned(&fixture, "third", "teammate@test.com", "Ada", bare);
    let teammate = |args: &[&str]| third.ok(args);
    let teammate_store = || third.store();
    let remote_path = plan.remote_path.to_str().unwrap();

    // No hub is named: the remote carries the plan.
    let created: Value =
        serde_json::from_str(&ok(&["task", "create", "git smoke", "--json"])).unwrap();
    let task = loopflow_execution::durable::TaskId::parse(created["task"]["id"].as_str().unwrap())
        .unwrap();
    let identifier = created["task"]["identifier"].as_str().unwrap().to_owned();
    let title = |store: &loopflow_execution::store::sqlite::SqliteStore| {
        store.task(&task).unwrap().map(|task| task.title)
    };
    // The save itself pushed it, with no server and nothing asked.
    let pushed = git(bare, &["for-each-ref", "refs/loopflow/plan"]);
    assert_eq!(pushed.lines().count(), 1, "{pushed}");
    assert_eq!(ok(&["repo", "tracker"]).trim(), "Plan: Git origin.");
    assert_eq!(
        ok(&["task", "sync", &identifier]).trim(),
        format!("{identifier}: nothing unsent")
    );
    assert_eq!(ok(&["repo", "tracker"]).trim(), "Plan: Git origin.");
    // The teammate never heard of the laptop: the Task is in the remote.
    teammate(&["task", "sync", &identifier]);
    assert_eq!(title(&teammate_store()).as_deref(), Some("git smoke"));
    let listed = teammate(&["task", "list"]);
    assert!(listed.contains("git smoke"), "{listed}");
    // mini's server fetches once a minute. Each clone keeps its repository id
    // and answers to the others', which is how the hub finds its checkout.
    loopflow_execution::ops::plan_git::exchange(&plan.remote, remote_path, None).unwrap();
    assert_eq!(title(&plan.remote).as_deref(), Some("git smoke"));
    ok(&["task", "sync", &identifier]);
    let laptop_id = plan
        .local
        .repository_id(plan.local_path.to_str().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(
        plan.remote.repository_path(&laptop_id).unwrap().as_deref(),
        Some(remote_path)
    );

    // With a hub, a save is on mini when the command returns.
    let _mini = loopflow_test_support::MachineServer::start(
        env!("CARGO_BIN_EXE_lf"),
        &fixture.root.path().join("remote/store"),
    );
    assert_success(&fixture.run(&["machine", "hub", "mini"]));
    let hub_line = |status: &str| status[status.find("Hub ").expect(status)..].to_owned();
    ok(&["task", "update", &identifier, "--title", "hub smoke"]);
    assert_eq!(title(&plan.remote).as_deref(), Some("hub smoke"));
    ok(&["wave", "create", "billing"]);
    assert_eq!(hub_line(&ok(&["repo", "tracker"])).trim(), "Hub mini.");

    // mini changes the title, then stops answering; the laptop changes it later.
    let revision = plan.remote.task(&task).unwrap().unwrap().revision;
    plan.remote
        .edit_task(
            &task,
            revision,
            &loopflow_execution::work::task::TaskUpdate {
                title: Some("one".into()),
                ..Default::default()
            },
        )
        .unwrap();
    fs::write(flags.join("down"), "").unwrap();
    ok(&["task", "update", &identifier, "--title", "two"]);
    let offline = hub_line(&ok(&["repo", "tracker"]));
    assert!(offline.starts_with("Hub mini, failing since "), "{offline}");
    assert!(offline.contains(", 1 unsent.\n"), "{offline}");
    assert!(offline.contains(&format!("  {identifier}\n")), "{offline}");
    // The hub is off and Git still carries the change to the teammate.
    assert!(!laptop(&["task", "sync", &identifier]).status.success());
    teammate(&["task", "sync", &identifier]);
    assert_eq!(title(&teammate_store()).as_deref(), Some("two"));
    // A Task made offline is one more record, and the reason is said once.
    ok(&["task", "create", "made offline"]);
    let offline = hub_line(&ok(&["repo", "tracker"]));
    assert!(offline.contains(", 2 unsent.\n"), "{offline}");
    assert_eq!(
        offline.matches("Operation timed out").count(),
        1,
        "{offline}"
    );

    // mini answers again, but its answer to the send is lost after it wrote it.
    fs::remove_file(flags.join("down")).unwrap();
    fs::write(flags.join("drop-apply"), "").unwrap();
    assert!(!laptop(&["task", "sync", &identifier]).status.success());
    assert_eq!(title(&plan.remote).as_deref(), Some("two"));
    let written = plan
        .remote
        .plan_records_since(remote_path, 0, usize::MAX)
        .unwrap()
        .seq;
    // The records go again and change nothing more on mini.
    assert_eq!(
        ok(&["task", "sync", &identifier]).trim(),
        format!("{identifier}: nothing unsent")
    );
    assert_eq!(
        plan.remote
            .plan_records_since(remote_path, 0, usize::MAX)
            .unwrap()
            .seq,
        written
    );
    assert_eq!(title(&plan.local).as_deref(), Some("two"));
    assert_eq!(title(&plan.remote).as_deref(), Some("two"));
    assert_eq!(
        ok(&["repo", "tracker"]).trim(),
        "Plan: Git origin.\nHub mini."
    );

    // The teammate may fetch but not push, and names mini their hub: their
    // change reaches the remote in the laptop's ref, which holds its whole plan.
    git(
        &third.checkout(),
        &["remote", "set-url", "--push", "origin", "/refused/plan.git"],
    );
    teammate(&["machine", "add", "mini"]);
    teammate(&["machine", "hub", "mini"]);
    teammate(&["task", "update", &identifier, "--title", "three"]);
    assert_eq!(title(&plan.remote).as_deref(), Some("three"));
    assert!(!third.run(&["task", "sync", &identifier]).status.success());
    let refused = teammate(&["repo", "tracker"]);
    assert!(
        refused.starts_with("Plan: Git origin, failing since "),
        "{refused}"
    );
    assert!(refused.contains(" unsent.\n"), "{refused}");
    assert!(refused.contains(&format!("  {identifier}\n")), "{refused}");
    ok(&["task", "sync", &identifier]);
    // A fourth home has no hub and never met the teammate.
    let fourth = Home::cloned(&fixture, "fourth", "fourth@test.com", "Grace", bare);
    fourth.ok(&["task", "sync", &identifier]);
    assert_eq!(title(&fourth.store()).as_deref(), Some("three"));
}

/// Two people claim one Task before either Machine has seen the other's
/// claim. The first claim holds on both; the other Machine's run is stopped.
#[test]
fn of_two_claims_made_apart_the_first_holds_on_both_machines() {
    use loopflow_execution::work::task::TaskEventKind;
    let fixture = Machines::new();
    let repo = loopflow_test_support::TestRepo::new();
    let ada = Home::cloned(&fixture, "ada", "ada@test.com", "Ada", repo.bare_path());
    let grace = Home::cloned(
        &fixture,
        "grace",
        "grace@test.com",
        "Grace",
        repo.bare_path(),
    );
    let created: Value =
        serde_json::from_str(&ada.ok(&["task", "create", "claimed twice", "--json"])).unwrap();
    let task = loopflow_execution::durable::TaskId::parse(created["task"]["id"].as_str().unwrap())
        .unwrap();
    let identifier = created["task"]["identifier"].as_str().unwrap().to_owned();
    ada.ok(&["task", "sync", &identifier]);
    grace.ok(&["task", "sync", &identifier]);

    // Ada's claim goes to the remote with the save. Grace claims while her
    // remote does not answer, so she has not seen it.
    ada.ok(&["task", "claim", &identifier]);
    let origin = repo.bare_path().to_str().unwrap();
    git(
        &grace.checkout(),
        &["remote", "set-url", "origin", "/unreachable/plan.git"],
    );
    grace.ok(&["task", "claim", &identifier]);
    git(&grace.checkout(), &["remote", "set-url", "origin", origin]);
    let owner = |home: &Home| {
        loopflow_execution::store::ownership::task_ownership(&home.store(), &task)
            .unwrap()
            .owner
            .and_then(|owner| owner.person_key)
    };
    assert_eq!(owner(&grace).as_deref(), Some("grace@test.com"));

    ada.ok(&["task", "sync", &identifier]);
    grace.ok(&["task", "sync", &identifier]);
    ada.ok(&["task", "sync", &identifier]);
    assert_eq!(owner(&ada).as_deref(), Some("ada@test.com"));
    assert_eq!(owner(&grace).as_deref(), Some("ada@test.com"));
    let interrupted = |home: &Home| {
        home.store()
            .task_events_after(&task, 0)
            .unwrap()
            .iter()
            .any(|event| matches!(event.kind, TaskEventKind::Interrupt))
    };
    assert!(interrupted(&grace));
    assert!(!interrupted(&ada));

    // Grace has seen Ada's claim now: claiming again takes the Task over.
    grace.ok(&["task", "claim", &identifier]);
    grace.ok(&["task", "sync", &identifier]);
    ada.ok(&["task", "sync", &identifier]);
    assert_eq!(owner(&grace).as_deref(), Some("grace@test.com"));
    let superseded = loopflow_execution::store::ownership::task_ownership(&ada.store(), &task)
        .unwrap()
        .superseded_by
        .and_then(|owner| owner.person_key);
    assert_eq!(superseded.as_deref(), Some("grace@test.com"));
}

#[test]
fn task_ownership_follows_claim_handoff_and_unassign_through_the_writer() {
    use loopflow_execution::durable::{OwnershipKind, TaskExecutionSource};
    let fixture = preview_machines();
    let plan = SharedPlan::new(&fixture);
    let task = plan.task();
    let mini = loopflow_execution::store::machines::local_machine(&plan.remote)
        .unwrap()
        .id;
    let laptop = loopflow_execution::store::machines::local_machine(&plan.local)
        .unwrap()
        .id;
    loopflow_execution::store::ownership::claim_task(&plan.remote, &task)
        .unwrap()
        .unwrap();
    plan.to_local();
    // The laptop follows the claim; it routes to the owner, never allocating here.
    let route =
        loopflow_execution::store::ownership::task_execution_route(&plan.local, &task).unwrap();
    assert_eq!(
        (route.machine_id.clone(), route.source),
        (mini.clone(), TaskExecutionSource::Owner)
    );
    assert!(loopflow_execution::store::ownership::claim_task(&plan.local, &task).is_err());

    // Moving it to the laptop sends it from mini, the writer, over SSH.
    let handed = fixture.json(&[
        "--repo-id",
        plan.repository.as_str(),
        "task",
        "move",
        task.as_str(),
        "--machine",
        laptop.as_str(),
        "--json",
    ]);
    assert_eq!(handed["owner"]["machine"]["id"], laptop.as_str());
    let kinds = |store: &loopflow_execution::store::sqlite::SqliteStore| {
        loopflow_execution::store::ownership::task_ownership(store, &task)
            .unwrap()
            .log
            .iter()
            .map(|entry| entry.kind)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        kinds(&plan.remote),
        [OwnershipKind::Claim, OwnershipKind::Claim]
    );
    plan.to_local();
    assert_eq!(
        loopflow_execution::store::ownership::task_execution_route(&plan.local, &task)
            .unwrap()
            .machine_id,
        laptop
    );

    // The laptop now writes; unassign keeps the history and leaves it unowned.
    let unassigned = fixture.json(&[
        "--repo-id",
        plan.repository.as_str(),
        "task",
        "unassign",
        task.as_str(),
        "--json",
    ]);
    assert!(unassigned["owner"].is_null());
    plan.to_remote();
    for store in [&plan.local, &plan.remote] {
        let ownership = loopflow_execution::store::ownership::task_ownership(store, &task).unwrap();
        assert!(ownership.owner.is_none());
        assert_eq!(
            kinds(store),
            [
                OwnershipKind::Claim,
                OwnershipKind::Claim,
                OwnershipKind::Unassign
            ]
        );
    }
    assert!(!fixture.root.path().join("forbidden-effect").exists());
}

#[test]
fn set_machine_changes_local_preferences_without_copying_or_moving_work() {
    use loopflow_execution::durable::MachineSetting;
    let fixture = preview_machines();
    let plan = SharedPlan::new(&fixture);
    let task = plan.task();
    plan.to_local();
    let mini = loopflow_execution::store::machines::local_machine(&plan.remote)
        .unwrap()
        .id;
    let laptop = loopflow_execution::store::machines::local_machine(&plan.local)
        .unwrap()
        .id;
    let wave = loopflow_execution::work::wave::bootstrap::ensure_wave_project(
        &plan.local,
        plan.local_path.to_str().unwrap(),
        "owners",
    )
    .unwrap()
    .wave_id;
    let output = fixture.run(&[
        "--repo-id",
        plan.repository.as_str(),
        "--wave",
        "owners",
        "set",
        "machine.tasks",
        mini.as_str(),
    ]);
    assert_success(&output);
    assert_eq!(
        loopflow_execution::store::placement::wave_owning_machine(
            &plan.local,
            &wave,
            MachineSetting::Tasks
        )
        .unwrap()
        .0,
        mini
    );
    assert_eq!(
        loopflow_execution::store::placement::wave_owning_machine(
            &plan.local,
            &wave,
            MachineSetting::Session
        )
        .unwrap()
        .0,
        laptop
    );
    assert!(!plan.remote_path.join(".lf/config.local.yaml").exists());
    assert_eq!(
        loopflow_execution::store::ownership::task_execution_route(&plan.local, &task)
            .unwrap()
            .machine_id,
        mini
    );
    assert!(
        loopflow_execution::store::ownership::task_ownership(&plan.local, &task)
            .unwrap()
            .log
            .is_empty()
    );
}

#[test]
fn remote_file_observer_keeps_the_recorded_owner_and_streams_changes() {
    use loopflow_execution::durable::RepositoryId;
    use loopflow_execution::ops::task::TaskFilesFrame;

    use std::io::{BufRead, BufReader};
    use std::sync::mpsc;
    use std::time::Duration;

    let fixture = preview_machines();
    let repo = loopflow_test_support::TestRepo::new();
    let path = repo.path().canonicalize().unwrap();
    let remote_home = fixture.root.path().join("remote/store");
    let registered =
        support::register_task_with_pr(&remote_home, &path, "companions", &repo.head_sha());
    let store = loopflow_execution::store::open_sqlite(&remote_home.join("loopflow.db")).unwrap();
    let repository = RepositoryId::new();
    loopflow_execution::repository::location::bind_repository(
        &store,
        path.to_str().unwrap(),
        &repository,
    )
    .unwrap();
    let owner = loopflow_execution::store::machines::local_machine(&store)
        .unwrap()
        .id;
    fs::write(path.join(".gitignore"), "target/\n").unwrap();
    fs::create_dir(path.join("target")).unwrap();
    let rejected = fixture.run(&[
        "--machine",
        "mini",
        "--repo-id",
        repository.as_str(),
        "task",
        "watch-files",
        registered.task.id.as_str(),
        "--checkout",
        "/different-checkout",
        "--request",
        "stale-request",
    ]);
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("checkout changed"));
    assert!(rejected.stdout.is_empty());

    let mut child = fixture
        .command(&[
            "--machine",
            "mini",
            "--repo-id",
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
    let repository = loopflow_execution::durable::RepositoryId::new();
    let local =
        loopflow_execution::store::open_sqlite(&fixture.root.path().join("local/loopflow.db"))
            .unwrap();
    let remote = loopflow_execution::store::open_sqlite(
        &fixture.root.path().join("remote/store/loopflow.db"),
    )
    .unwrap();
    loopflow_execution::repository::location::bind_repository(
        &local,
        repo.path().canonicalize().unwrap().to_str().unwrap(),
        &repository,
    )
    .unwrap();
    loopflow_execution::repository::location::bind_repository(
        &remote,
        remote_checkout.to_str().unwrap(),
        &repository,
    )
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
        loopflow_execution::store::open_sqlite(&fixture.root.path().join("local/loopflow.db"))
            .unwrap();
    assert!(local.session_moved_to(&session).unwrap().is_none());
}

fn git_dir(worktree: &Path) -> std::path::PathBuf {
    std::path::PathBuf::from(git(worktree, &["rev-parse", "--absolute-git-dir"]))
}

/// A Task the laptop owns, checked out on `branch` in a worktree of the
/// laptop's checkout, with mini knowing it through planning.
fn placed_task(
    plan: &SharedPlan,
    branch: &str,
) -> (loopflow_execution::durable::TaskId, std::path::PathBuf) {
    let task = plan.task();
    plan.to_local();
    let worktree = place(plan, &task, branch);
    (task, worktree)
}

/// Check `task` out on the laptop, owned there.
fn place(
    plan: &SharedPlan,
    task: &loopflow_execution::durable::TaskId,
    branch: &str,
) -> std::path::PathBuf {
    use loopflow_execution::work::task::{TaskPr, TaskPrId};
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
    loopflow_execution::store::ownership::place_task(
        &plan.local,
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
        loopflow_execution::store::ownership::claim_task(&plan.local, task).unwrap();
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
fn at_explore(db: &Path, task: &loopflow_execution::durable::TaskId) {
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
    let mini = loopflow_execution::store::machines::local_machine(&plan.remote)
        .unwrap()
        .id;
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
    let owner = loopflow_execution::store::ownership::task_ownership(&plan.remote, &task)
        .unwrap()
        .owner
        .unwrap();
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
        loopflow_execution::store::ownership::task_execution_route(&plan.local, &task)
            .unwrap()
            .machine_id,
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
fn a_move_that_did_not_arrive_shows_in_status_and_the_same_command_delivers_it() {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let mini = loopflow_execution::store::machines::local_machine(&plan.remote)
        .unwrap()
        .id;
    let (task, worktree) = placed_task(&plan, "retry-move");
    at_explore(&fixture.root.path().join("local/loopflow.db"), &task);
    let session = claude_conversation(&fixture, &worktree, "0199a213-81c0-7800-8aa1-bbab2a035a58");
    fs::write(worktree.join("notes.txt"), "sent once\n").unwrap();
    // Mini answers the check, then falls over receiving what was sent.
    let remote_lf = fixture.root.path().join("remote/.local/bin/lf");
    let working = fs::read_to_string(&remote_lf).unwrap();
    executable(
        &remote_lf,
        &working.replacen(
            "#!/bin/sh\n",
            "#!/bin/sh\ncase \"$*\" in *'session receive'*) case \"$*\" in *--check*) ;; *) echo 'mini fell over' >&2; exit 1;; esac;; esac\n",
            1,
        ),
    );

    let failed = fixture
        .command(&["task", "move", task.as_str(), "--machine", "mini"])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert!(!failed.status.success());
    let stderr = String::from_utf8_lossy(&failed.stderr);
    let name = plan.local.task(&task).unwrap().unwrap().identifier;
    let retry = format!("lf task move {name} --machine mini");
    assert!(stderr.contains(&retry), "{stderr}");
    let take_back = format!("take back: lf task claim {name}");
    assert!(stderr.contains(&take_back), "{stderr}");

    // Status reads mini's server, where the Task runs.
    let _mini = served_mini(&fixture);
    let status = fixture
        .command(&["task", "status", task.as_str()])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert_success(&status);
    let status = String::from_utf8_lossy(&status.stdout);
    assert!(status.contains("Moving to mini since"), "{status}");
    assert!(status.contains("sent, not arrived"), "{status}");
    assert!(status.contains(&format!("retry: {retry}")), "{status}");
    assert!(status.contains(&take_back), "{status}");
    // The lists read the move the same way, not as a Task that runs on mini.
    for args in [&["task", "list"][..], &["machine", "list"][..]] {
        let listed = fixture
            .command(args)
            .current_dir(&plan.local_path)
            .output()
            .unwrap();
        assert_success(&listed);
        let listed = String::from_utf8_lossy(&listed.stdout);
        let row = listed
            .lines()
            .find(|line| line.contains(&name))
            .unwrap_or_else(|| panic!("{name} missing from:\n{listed}"));
        assert!(row.contains("Moving to mini since"), "{row}");
        assert!(row.contains("sent, not arrived"), "{row}");
    }

    // The retry delivers what was sent: nothing is committed or pushed again.
    executable(&remote_lf, &working);
    fs::write(worktree.join("notes.txt"), "edited after sending\n").unwrap();
    let retried = fixture
        .command(&["task", "move", task.as_str(), "--machine", "mini"])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert_success(&retried);
    let stderr = String::from_utf8_lossy(&retried.stderr);
    assert!(stderr.contains("move to mini"), "{stderr}");
    assert!(!stderr.contains("Committed and pushed"), "{stderr}");
    assert_eq!(
        git(
            plan._repos[0].bare_path(),
            &["show", "retry-move:notes.txt"]
        ),
        "sent once"
    );
    let owner = loopflow_execution::store::ownership::task_ownership(&plan.remote, &task)
        .unwrap()
        .owner
        .unwrap();
    assert_eq!(owner.machine.id, Some(mini));
    assert!(plan.local.task_move_in_flight(&task).unwrap().is_none());
    // The conversation that left with the first send arrived with the retry.
    let arrived = plan.remote.session(&session).unwrap().unwrap();
    assert!(arrived.completed_at.is_none());
    assert_eq!(arrived.task_id.as_ref(), Some(&task));
    let status = fixture
        .command(&["task", "status", task.as_str()])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    let status = String::from_utf8_lossy(&status.stdout);
    assert!(
        status.contains("Runs on mini; read from its server."),
        "{status}"
    );
}

#[test]
fn a_failed_conversation_import_keeps_the_move_available_for_retry() {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let mini = loopflow_execution::store::machines::local_machine(&plan.remote)
        .unwrap()
        .id;
    let (task, worktree) = placed_task(&plan, "retry-conversation");
    let conversation = "0199a213-81c0-7800-8aa1-bbab2a035a60";
    let session = claude_conversation(&fixture, &worktree, conversation);
    let blocked = fixture.root.path().join("remote/.claude");
    fs::write(&blocked, "blocks the transcript directory").unwrap();

    let failed = fixture
        .command(&["task", "move", task.as_str(), "--machine", "mini"])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert!(!failed.status.success());
    assert!(plan.remote.task(&task).unwrap().unwrap().worktree.is_some());
    assert!(!plan.remote.task_move_arrived_on(&task, &mini).unwrap());

    fs::remove_file(&blocked).unwrap();
    let retried = fixture
        .command(&["task", "move", task.as_str(), "--machine", "mini"])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert_success(&retried);
    assert!(plan.remote.task_move_arrived_on(&task, &mini).unwrap());
    let arrived = plan.remote.session(&session).unwrap().unwrap();
    let project = fixture
        .root
        .path()
        .join("remote/.claude/projects")
        .join(claude_project(&arrived.cwd));
    assert!(project.join(format!("{conversation}.jsonl")).is_file());
}

/// Mini never took the Task it was sent, and the laptop claims it back: the
/// move is over, so status shows none and moving it again begins a new move
/// instead of delivering the old one.
#[test]
fn claiming_a_task_back_ends_a_move_that_never_arrived() {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let mini = loopflow_execution::store::machines::local_machine(&plan.remote)
        .unwrap()
        .id;
    let (task, worktree) = placed_task(&plan, "claimed-back");
    fs::write(worktree.join("notes.txt"), "sent once\n").unwrap();
    let remote_lf = fixture.root.path().join("remote/.local/bin/lf");
    let working = fs::read_to_string(&remote_lf).unwrap();
    executable(
        &remote_lf,
        &working.replacen(
            "#!/bin/sh\n",
            "#!/bin/sh\ncase \"$*\" in *'session receive'*) case \"$*\" in *--check*) ;; *) echo 'mini fell over' >&2; exit 1;; esac;; esac\n",
            1,
        ),
    );
    let failed = fixture
        .command(&["task", "move", task.as_str(), "--machine", "mini"])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert!(!failed.status.success());

    let claimed = fixture
        .command(&["task", "claim", task.as_str()])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert_success(&claimed);
    let status = fixture
        .command(&["task", "status", task.as_str()])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert_success(&status);
    let status = String::from_utf8_lossy(&status.stdout);
    assert!(!status.contains("Moving to mini"), "{status}");

    executable(&remote_lf, &working);
    fs::write(worktree.join("notes.txt"), "edited after claiming\n").unwrap();
    let moved = fixture
        .command(&["task", "move", task.as_str(), "--machine", "mini"])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert_success(&moved);
    assert_eq!(
        git(
            plan._repos[0].bare_path(),
            &["show", "claimed-back:notes.txt"]
        ),
        "edited after claiming"
    );
    let owner = loopflow_execution::store::ownership::task_ownership(&plan.remote, &task)
        .unwrap()
        .owner
        .unwrap();
    assert_eq!(owner.machine.id, Some(mini));
    assert!(plan.local.task_move_in_flight(&task).unwrap().is_none());
}

/// Mini took the Task in, but its answer never reached the laptop, which
/// still holds the move as sent. Claiming it back on the laptop asks mini
/// first: it arrived, so the claim is mini's to write, and the Task stays on
/// mini alone rather than running on both.
#[test]
fn claiming_back_a_task_whose_arrival_answer_was_lost_leaves_it_on_its_target() {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let mini = loopflow_execution::store::machines::local_machine(&plan.remote)
        .unwrap()
        .id;
    let (task, _worktree) = placed_task(&plan, "answer-lost");
    let remote_lf = fixture.root.path().join("remote/.local/bin/lf");
    let working = fs::read_to_string(&remote_lf).unwrap();
    executable(
        &remote_lf,
        &working.replacen(
            "exec ",
            "case \"$*\" in *'session receive') \"$LF_BIN\" \"$@\" >/dev/null || exit 1; echo 'connection dropped' >&2; exit 1;; esac\nexec ",
            1,
        ),
    );
    let failed = fixture
        .command(&["task", "move", task.as_str(), "--machine", "mini"])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert!(!failed.status.success());
    assert!(plan.remote.task_move_arrived_on(&task, &mini).unwrap());
    assert!(plan.local.task_move_in_flight(&task).unwrap().is_some());

    executable(&remote_lf, &working);
    // The claim and the status read reach mini's server, where the Task runs.
    let _mini = served_mini(&fixture);
    let claimed = fixture
        .command(&["task", "claim", task.as_str()])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert_success(&claimed);
    let owner = loopflow_execution::store::ownership::task_ownership(&plan.remote, &task)
        .unwrap()
        .owner
        .unwrap();
    assert_eq!(owner.machine.id, Some(mini.clone()));
    let local = loopflow_execution::store::ownership::task_ownership(&plan.local, &task)
        .unwrap()
        .owner
        .unwrap();
    assert_eq!(local.machine.id, Some(mini));
    assert!(plan.local.task_move_in_flight(&task).unwrap().is_none());
    let status = fixture
        .command(&["task", "status", task.as_str()])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert_success(&status);
    let status = String::from_utf8_lossy(&status.stdout);
    assert!(
        status.contains("Runs on mini; read from its server."),
        "{status}"
    );
    assert!(!status.contains("Moving to mini"), "{status}");
}

/// The laptop sent the Task and mini fell over receiving it. The same move
/// addressed to mini from the laptop (`lf --machine mini task move X --machine
/// mini`) runs on the laptop, which holds what it sent, and delivers it.
#[test]
fn a_sent_move_forwarded_to_its_target_delivers_what_was_sent() {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let mini = loopflow_execution::store::machines::local_machine(&plan.remote)
        .unwrap()
        .id;
    let (task, worktree) = placed_task(&plan, "forwarded-retry");
    at_explore(&fixture.root.path().join("local/loopflow.db"), &task);
    fs::write(worktree.join("notes.txt"), "sent once\n").unwrap();
    let remote_lf = fixture.root.path().join("remote/.local/bin/lf");
    let working = fs::read_to_string(&remote_lf).unwrap();
    executable(
        &remote_lf,
        &working.replacen(
            "#!/bin/sh\n",
            "#!/bin/sh\ncase \"$*\" in *'session receive'*) case \"$*\" in *--check*) ;; *) echo 'mini fell over' >&2; exit 1;; esac;; esac\n",
            1,
        ),
    );
    let failed = fixture
        .command(&["task", "move", task.as_str(), "--machine", "mini"])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert!(!failed.status.success());
    executable(&remote_lf, &working);

    fs::write(worktree.join("notes.txt"), "edited after sending\n").unwrap();
    let retried = fixture
        .command(&[
            "--machine",
            "mini",
            "task",
            "move",
            task.as_str(),
            "--machine",
            "mini",
        ])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert_success(&retried);
    let stderr = String::from_utf8_lossy(&retried.stderr);
    assert!(stderr.contains("move to mini"), "{stderr}");
    assert!(!stderr.contains("Committed and pushed"), "{stderr}");
    assert_eq!(
        git(
            plan._repos[0].bare_path(),
            &["show", "forwarded-retry:notes.txt"]
        ),
        "sent once"
    );
    let owner = loopflow_execution::store::ownership::task_ownership(&plan.remote, &task)
        .unwrap()
        .owner
        .unwrap();
    assert_eq!(owner.machine.id, Some(mini));
    assert!(plan.local.task_move_in_flight(&task).unwrap().is_none());
}

/// The laptop began moving the Task and was killed before sending it. The same
/// move addressed to mini from the laptop runs on the laptop, which holds the
/// move, and carries it on.
#[test]
fn a_leaving_move_forwarded_to_its_target_carries_on() {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let mini = loopflow_execution::store::machines::local_machine(&plan.remote)
        .unwrap()
        .id;
    let (task, worktree) = placed_task(&plan, "forwarded-leaving");
    loopflow_execution::store::ownership::begin_task_move(&plan.local, &task, &mini).unwrap();
    fs::write(worktree.join("notes.txt"), "halfway\n").unwrap();

    let retried = fixture
        .command(&[
            "--machine",
            "mini",
            "task",
            "move",
            task.as_str(),
            "--machine",
            "mini",
        ])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert_success(&retried);
    let stderr = String::from_utf8_lossy(&retried.stderr);
    assert!(stderr.contains("Carrying on"), "{stderr}");
    assert_eq!(
        git(
            plan._repos[0].bare_path(),
            &["show", "forwarded-leaving:notes.txt"]
        ),
        "halfway"
    );
    let owner = loopflow_execution::store::ownership::task_ownership(&plan.remote, &task)
        .unwrap()
        .owner
        .unwrap();
    assert_eq!(owner.machine.id, Some(mini));
    assert!(plan.local.task_move_in_flight(&task).unwrap().is_none());
}

/// The move reached mini's check but never arrived. Mini remembers which
/// Machine the move comes from, so the same command run on mini asks the
/// laptop for it and delivers it there.
#[test]
fn a_move_retried_on_its_target_asks_the_machine_it_comes_from() {
    checked_move_on_target(false);
}

#[test]
fn a_move_checked_on_its_target_stays_a_past_check_after_source_claim_back() {
    checked_move_on_target(true);
}

fn checked_move_on_target(claim_back: bool) {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let mini = loopflow_execution::store::machines::local_machine(&plan.remote)
        .unwrap()
        .id;
    let (task, worktree) = placed_task(&plan, "retry-on-mini");
    // Mini reaches the laptop over its own SSH.
    let local = fixture.root.path().join("local");
    fs::create_dir_all(local.join(".local/bin")).unwrap();
    executable(
        &local.join(".local/bin/lf"),
        &format!(
            "#!/bin/sh\nexport LF_HOME='{}' LF_BIN='{}'\nexec '{}' \"$@\"\n",
            local.display(),
            env!("CARGO_BIN_EXE_lf"),
            env!("CARGO_BIN_EXE_lf"),
        ),
    );
    executable(
        &fixture.root.path().join("remote/.local/bin/ssh"),
        &format!(
            "#!/bin/sh\nfor arg in \"$@\"; do remote_command=\"$arg\"; done\ncd '{}' || exit 1\nexec env -i HOME='{}' PATH=/usr/bin:/bin bash -c \"$remote_command\"\n",
            local.display(),
            local.display()
        ),
    );
    // A person at mini runs lf there.
    let remote = fixture.root.path().join("remote");
    let on_mini = |args: &[&str]| {
        Command::new(remote.join(".local/bin/lf"))
            .env_clear()
            .env("HOME", &remote)
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", remote.join(".local/bin").display()),
            )
            .current_dir(&plan.remote_path)
            .args(args)
            .output()
            .unwrap()
    };
    assert_success(&on_mini(&["machine", "add", "laptop"]));

    at_explore(&fixture.root.path().join("local/loopflow.db"), &task);
    fs::write(worktree.join("notes.txt"), "sent once\n").unwrap();
    let remote_lf = fixture.root.path().join("remote/.local/bin/lf");
    let working = fs::read_to_string(&remote_lf).unwrap();
    executable(
        &remote_lf,
        &working.replacen(
            "#!/bin/sh\n",
            "#!/bin/sh\ncase \"$*\" in *'session receive'*) case \"$*\" in *--check*) ;; *) echo 'mini fell over' >&2; exit 1;; esac;; esac\n",
            1,
        ),
    );
    let failed = fixture
        .command(&["task", "move", task.as_str(), "--machine", "mini"])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert!(!failed.status.success());
    executable(&remote_lf, &working);

    let check = plan.remote.task_move_check(&task).unwrap().unwrap();
    assert_eq!(
        check.from_machine_id,
        loopflow_execution::store::machines::local_machine(&plan.local)
            .unwrap()
            .id
    );
    let status = on_mini(&["task", "status", task.as_str()]);
    assert_success(&status);
    let status = String::from_utf8(status.stdout).unwrap();
    let checked_line = format!(
        "Checked move from laptop at {}",
        chrono::DateTime::from_timestamp(check.at, 0)
            .unwrap()
            .with_timezone(&chrono::Local)
            .format("%b %-d %H:%M")
    );
    assert!(status.contains(&checked_line), "{status}");
    assert!(status.contains("Unowned"), "{status}");
    assert!(status.contains("Not checked out on "), "{status}");
    assert!(!status.contains("Moving to"), "{status}");
    let retry = status
        .lines()
        .find_map(|line| line.trim().strip_prefix("retry: lf "))
        .unwrap_or_else(|| panic!("{status}"));
    let identifier = plan.remote.task(&task).unwrap().unwrap().identifier;
    assert!(
        retry.starts_with(&format!("task move {identifier} --machine ")),
        "{status}"
    );
    if claim_back {
        let claimed = fixture
            .command(&["task", "claim", task.as_str()])
            .current_dir(&plan.local_path)
            .output()
            .unwrap();
        assert_success(&claimed);
        assert_eq!(
            loopflow_execution::store::ownership::task_ownership(&plan.local, &task)
                .unwrap()
                .owner
                .unwrap()
                .machine
                .id,
            Some(
                loopflow_execution::store::machines::local_machine(&plan.local)
                    .unwrap()
                    .id
            )
        );
        let after = on_mini(&["task", "status", task.as_str()]);
        assert_success(&after);
        let after = String::from_utf8(after.stdout).unwrap();
        assert!(after.contains(&checked_line), "{after}");
        assert!(after.contains(retry), "{after}");
        assert!(after.contains("Unowned"), "{after}");
        assert!(after.contains("Not checked out on "), "{after}");
        assert!(!after.contains("Moving to"), "{after}");
        assert!(
            loopflow_execution::store::ownership::task_ownership(&plan.remote, &task)
                .unwrap()
                .owner
                .is_none()
        );
        return;
    }
    // The laptop's retry line names mini by the laptop's label; on mini the
    // refusal prints the check and the command that works there.
    let laptop_line = on_mini(&["task", "move", task.as_str(), "--machine", "mini"]);
    assert!(!laptop_line.status.success());
    let refusal = String::from_utf8_lossy(&laptop_line.stderr);
    assert!(refusal.contains(&checked_line), "{refusal}");
    assert!(refusal.contains(retry), "{refusal}");
    let retried = on_mini(&retry.split(' ').collect::<Vec<_>>());
    assert_success(&retried);
    let stderr = String::from_utf8_lossy(&retried.stderr);
    assert!(stderr.contains("move to"), "{stderr}");
    let owner = loopflow_execution::store::ownership::task_ownership(&plan.remote, &task)
        .unwrap()
        .owner
        .unwrap();
    assert_eq!(owner.machine.id, Some(mini));
    assert_eq!(plan.remote.task_move_check(&task).unwrap(), None);
    let arrived_status = on_mini(&["task", "status", task.as_str()]);
    assert_success(&arrived_status);
    assert!(!String::from_utf8_lossy(&arrived_status.stdout).contains("Checked move from"));
    // The laptop heard it arrived.
    assert!(plan.local.task_move_in_flight(&task).unwrap().is_none());
    assert_eq!(
        git(
            plan._repos[0].bare_path(),
            &["show", "retry-on-mini:notes.txt"]
        ),
        "sent once"
    );
}

/// Mini sends the Task back to the laptop, and its reply never arrives. Mini
/// recorded what it sent, so the same command delivers the conversation
/// that left with the first send.
#[test]
fn a_task_sent_without_its_reply_delivers_its_conversation_when_repeated() {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let laptop_id = loopflow_execution::store::machines::local_machine(&plan.local)
        .unwrap()
        .id;
    let (task, worktree) = placed_task(&plan, "lost-reply");
    at_explore(&fixture.root.path().join("local/loopflow.db"), &task);
    let session = claude_conversation(&fixture, &worktree, "0199a213-81c0-7800-8aa1-bbab2a035a59");
    let move_to = |machine: &str| {
        fixture
            .command(&["task", "move", task.as_str(), "--machine", machine])
            .current_dir(&plan.local_path)
            .output()
            .unwrap()
    };
    assert_success(&move_to("mini"));
    assert!(plan
        .remote
        .session(&session)
        .unwrap()
        .unwrap()
        .completed_at
        .is_none());

    // Mini sends, then its reply is lost on the way back.
    let remote_lf = fixture.root.path().join("remote/.local/bin/lf");
    let working = fs::read_to_string(&remote_lf).unwrap();
    let lf = env!("CARGO_BIN_EXE_lf");
    executable(
        &remote_lf,
        &working.replacen(
            "\nexec ",
            &format!("\ncase \"$*\" in *'session send'*--to*) case \"$*\" in *--arrived*) ;; *) '{lf}' \"$@\" >/dev/null; echo 'connection reset' >&2; exit 255;; esac;; esac\nexec "),
            1,
        ),
    );
    let laptop = gethostname::gethostname().to_string_lossy().into_owned();
    let laptop = laptop.split('.').next().unwrap().to_string();
    let lost = move_to(&laptop);
    assert!(!lost.status.success());
    let sent = plan.remote.task_move_in_flight(&task).unwrap().unwrap();
    assert_eq!(sent.to_machine_id, laptop_id);
    // The conversation left mini with that send.
    assert!(plan
        .remote
        .session(&session)
        .unwrap()
        .unwrap()
        .completed_at
        .is_some());

    executable(&remote_lf, &working);
    let delivered = move_to(&laptop);
    assert_success(&delivered);
    let stderr = String::from_utf8_lossy(&delivered.stderr);
    assert!(stderr.contains("its Session moved with it"), "{stderr}");
    // The conversation is open here again.
    assert!(plan
        .local
        .session(&session)
        .unwrap()
        .unwrap()
        .completed_at
        .is_none());
    assert_eq!(
        loopflow_execution::store::ownership::task_execution_route(&plan.local, &task)
            .unwrap()
            .machine_id,
        laptop_id
    );
    // Mini heard it arrived: nothing is in flight there any more.
    assert!(plan.remote.task_move_in_flight(&task).unwrap().is_none());
}

#[test]
fn a_machine_without_the_checkout_shows_no_path_for_it() {
    let fixture = preview_machines();
    let plan = SharedPlan::new(&fixture);
    let (task, worktree) = placed_task(&plan, "laptop-only");
    plan.to_remote();
    let output = fixture
        .command(&["--machine", "mini", "task", "status", task.as_str()])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert_success(&output);
    let status = String::from_utf8_lossy(&output.stdout);
    assert!(status.contains("Not checked out on "), "{status}");
    assert!(
        !status.contains(worktree.to_str().unwrap()),
        "mini shows the laptop's path:\n{status}"
    );
    // The laptop still has it.
    let here = fixture
        .command(&["task", "status", task.as_str()])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert_success(&here);
    let here = String::from_utf8_lossy(&here.stdout);
    assert!(!here.contains("Not checked out on "), "{here}");
}

#[test]
fn the_laptop_says_where_a_moved_task_runs_and_prunes_its_checkout() {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let (task, worktree) = placed_task(&plan, "left-task");
    let identifier = plan.local.task(&task).unwrap().unwrap().identifier;
    let laptop = |args: &[&str]| {
        // Prune reads open paths with /usr/sbin/lsof.
        let output = fixture
            .command(args)
            .env(
                "PATH",
                format!(
                    "{}:/usr/bin:/bin:/usr/sbin",
                    fixture.root.path().join("bin").display()
                ),
            )
            .current_dir(&plan.local_path)
            .output()
            .unwrap();
        assert_success(&output);
        String::from_utf8_lossy(&output.stdout).into_owned()
    };
    // Before the move, the checkout is the unfinished Task's.
    let kept = laptop(&["wt", "prune", "--dry-run"]);
    assert!(
        kept.contains(&format!("Task {identifier} is unfinished")),
        "{kept}"
    );

    laptop(&["task", "move", task.as_str(), "--machine", "mini"]);

    // Status reads mini's server, where the Task runs.
    let _mini = served_mini(&fixture);
    let status = laptop(&["task", "status", task.as_str()]);
    assert!(
        status.contains(&format!(
            "· {identifier}\nRuns on mini; read from its server."
        )),
        "{status}"
    );
    let list = laptop(&["task", "list"]);
    let row = list
        .lines()
        .find(|line| line.starts_with(&identifier))
        .unwrap_or_else(|| panic!("{identifier} missing from:\n{list}"));
    assert!(row.contains("Runs on mini since "), "{row}");
    let machines = laptop(&["machine", "list"]);
    assert!(
        machines.contains(&format!("  {identifier} since ")),
        "{machines}"
    );
    let listed = fixture.json(&["machine", "list", "mini", "--json"]);
    assert_eq!(listed[0]["tasks"][0]["id"], task.as_str());
    assert!(
        listed[0]["tasks"][0]["since"].as_i64().unwrap() > 0,
        "{listed}"
    );
    // The branch is pushed and mini checked it out, so prune takes the checkout.
    let pruned = laptop(&["wt", "prune", "--dry-run"]);
    assert!(
        pruned.contains(&format!("Task {identifier} runs on mini")),
        "{pruned}"
    );
    laptop(&["wt", "prune"]);
    assert!(!worktree.exists());
}

#[test]
fn resume_task_on_another_machine_moves_the_task_there() {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let mini = loopflow_execution::store::machines::local_machine(&plan.remote)
        .unwrap()
        .id;
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
    let owner = loopflow_execution::store::ownership::task_ownership(&plan.remote, &task)
        .unwrap()
        .owner
        .unwrap();
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
        loopflow_execution::store::ownership::task_execution_route(&plan.local, &task)
            .unwrap()
            .machine_id,
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
    let before = loopflow_execution::store::ownership::task_ownership(&plan.local, &task)
        .unwrap()
        .log
        .len();

    let refused = fixture
        .command(&["task", "move", task.as_str(), "--machine", "mini"])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert!(!refused.status.success());
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(stderr.contains("mid-merge"), "{stderr}");
    assert_eq!(git(&worktree, &["rev-parse", "HEAD"]), head);
    assert_eq!(
        loopflow_execution::store::ownership::task_ownership(&plan.local, &task)
            .unwrap()
            .log
            .len(),
        before
    );
}

#[test]
fn task_move_hands_nothing_off_when_mini_cannot_receive_it() {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let task = plan.local_task();
    let worktree = place(&plan, &task, "stay-task");
    fs::write(worktree.join("notes.txt"), "uncommitted\n").unwrap();
    let before = loopflow_execution::store::ownership::task_ownership(&plan.local, &task)
        .unwrap()
        .log
        .len();
    let unchanged = || {
        assert_eq!(
            git(&worktree, &["status", "--porcelain"]),
            "?? notes.txt",
            "the worktree stays uncommitted"
        );
        assert_eq!(
            loopflow_execution::store::ownership::task_ownership(&plan.local, &task)
                .unwrap()
                .log
                .len(),
            before
        );
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

    // Mini answers, but its plan has never heard of the Task.
    refused("not found");
    unchanged();

    // Mini does not answer at all.
    executable(
        &fixture.root.path().join("bin/ssh"),
        "#!/bin/sh\nexit 255\n",
    );
    refused("unreachable");
    unchanged();
}

/// The move hands the Task off before it stops anything, so nothing here
/// restarts it meanwhile. When the checkpoint cannot be pushed, it hands the
/// Task back: this Machine owns it again and its conversation stays open.
#[test]
fn task_move_hands_the_task_back_when_its_checkpoint_cannot_be_pushed() {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let (task, worktree) = placed_task(&plan, "refused-task");
    let session = claude_conversation(&fixture, &worktree, "0199a213-81c0-7800-8aa1-bbab2a035a58");
    fs::write(worktree.join("notes.txt"), "uncommitted\n").unwrap();
    executable(
        &plan._repos[0].bare_path().join("hooks/pre-receive"),
        "#!/bin/sh\necho 'push refused' >&2\nexit 1\n",
    );

    let output = fixture
        .command(&["task", "move", task.as_str(), "--machine", "mini"])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("push refused"), "{stderr}");

    let local = loopflow_execution::store::machines::local_machine(&plan.local)
        .unwrap()
        .id;
    let ownership =
        loopflow_execution::store::ownership::task_ownership(&plan.local, &task).unwrap();
    assert_eq!(ownership.owner.unwrap().machine.id, Some(local.clone()));
    let kinds: Vec<_> = ownership
        .log
        .iter()
        .rev()
        .take(2)
        .map(|entry| entry.machine_id.clone())
        .collect();
    assert_eq!(
        kinds,
        [
            Some(local),
            Some(
                loopflow_execution::store::machines::local_machine(&plan.remote)
                    .unwrap()
                    .id
            )
        ],
        "the log reads: handed to mini, handed back"
    );
    assert!(plan
        .local
        .session(&session)
        .unwrap()
        .unwrap()
        .completed_at
        .is_none());
    assert!(plan
        .remote
        .task(&task)
        .unwrap()
        .is_none_or(|task| task.worktree.is_none()));
}

/// A laptop whose plan is on mini, its hub, the same person on both
/// Machines, and a Task the laptop created. Mini has no checkout of the
/// repository yet, so the Task waits on the laptop to be sent.
fn hub_plan_task(
    fixture: &Machines,
) -> (
    loopflow_test_support::TestRepo,
    loopflow_execution::durable::TaskId,
    String,
) {
    fs::write(
        fixture.root.path().join("remote/.gitconfig"),
        "[user]\n\temail = test@test.com\n\tname = Jack\n",
    )
    .unwrap();
    let repo = loopflow_test_support::TestRepo::new();
    assert_success(&fixture.run(&["machine", "hub", "mini"]));
    let created = fixture
        .command(&["task", "create", "--title", "Cold move", "--json"])
        .current_dir(repo.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_success(&created);
    let created: Value = serde_json::from_slice(&created.stdout).unwrap();
    let task = loopflow_execution::durable::TaskId::parse(created["task"]["id"].as_str().unwrap())
        .unwrap();
    let identifier = created["task"]["identifier"].as_str().unwrap().to_string();
    (repo, task, identifier)
}

/// The check clones the repository, the laptop sends the Task's plan to mini,
/// its hub, and the move lands. Only planning and origin are shared.
#[test]
fn task_move_to_a_mini_without_the_repository_takes_it_in_from_planning() {
    let fixture = preview_machines();
    let lf = env!("CARGO_BIN_EXE_lf");
    let local_home = fixture.root.path().join("local");
    let remote_home = fixture.root.path().join("remote/store");
    let _laptop = loopflow_test_support::MachineServer::start(lf, &local_home);
    let _mini = loopflow_test_support::MachineServer::start(lf, &remote_home);
    let _ended = EndRecorded(vec![local_home.clone(), remote_home.clone()]);
    let (repo, task, identifier) = hub_plan_task(&fixture);

    let moved = fixture
        .command(&["task", "move", &identifier, "--machine", "mini"])
        .current_dir(repo.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_success(&moved);
    let stderr = String::from_utf8_lossy(&moved.stderr);
    assert!(
        stderr.contains(&format!("Checked mini: it can take {identifier}")),
        "{stderr}"
    );
    assert!(stderr.contains("Arrived on mini"), "{stderr}");

    let remote = loopflow_execution::store::open_sqlite(&remote_home.join("loopflow.db")).unwrap();
    assert!(
        remote.task(&task).unwrap().is_some(),
        "mini never took the Task in"
    );
    assert_eq!(
        loopflow_execution::store::ownership::task_ownership(&remote, &task)
            .unwrap()
            .owner
            .unwrap()
            .machine
            .id,
        Some(
            loopflow_execution::store::machines::local_machine(&remote)
                .unwrap()
                .id
        )
    );
    assert!(
        fixture
            .root
            .path()
            .join("remote/src")
            .read_dir()
            .unwrap()
            .next()
            .is_some(),
        "mini has no clone"
    );
}

/// The laptop's run turned auto on, and it travels with the Task's Graph.
/// Once the Task moved back, mini's recovery must not start it again.
#[test]
fn mini_starts_nothing_for_a_task_that_moved_back() {
    let fixture = preview_machines();
    let lf = env!("CARGO_BIN_EXE_lf");
    let local_home = fixture.root.path().join("local");
    let remote_home = fixture.root.path().join("remote/store");
    let _laptop = loopflow_test_support::MachineServer::start(lf, &local_home);
    let _mini = loopflow_test_support::MachineServer::start(lf, &remote_home);
    let _ended = EndRecorded(vec![local_home.clone(), remote_home.clone()]);
    let (repo, task, identifier) = hub_plan_task(&fixture);
    fs::create_dir_all(repo.path().join(".lf/graphs")).unwrap();
    fs::write(
        repo.path().join(".lf/graphs/hold.yaml"),
        format!("- cmd: task status {task}\n- cmd: task wait {task} --timeout 120s\n- interactive: demo\n"),
    )
    .unwrap();
    git(repo.path(), &["add", ".lf/graphs/hold.yaml"]);
    git(repo.path(), &["commit", "-qm", "hold"]);
    git(repo.path(), &["push", "-q", "origin", "main"]);
    let mut run = fixture
        .command(&["-b", "task", "run", &identifier, "hold"])
        .current_dir(repo.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    assert!(
        wait_step_live(&local_home.join("loopflow.db")),
        "the laptop's step never started"
    );
    let move_to = |machine: &str| {
        let output = fixture
            .command(&["task", "move", &identifier, "--machine", machine])
            .current_dir(repo.path())
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_success(&output);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!stderr.contains("did not continue"), "{stderr}");
    };
    move_to("mini");
    assert!(
        exits_within_ten_seconds(&mut run),
        "the laptop's run outlived the move"
    );
    assert!(
        wait_step_live(&remote_home.join("loopflow.db")),
        "mini's step never started"
    );
    let remote_db = remote_home.join("loopflow.db");
    assert_eq!(
        first_steps_run(&remote_db, &task),
        0,
        "shipping replayed the first step"
    );
    let laptop = gethostname::gethostname().to_string_lossy().into_owned();
    move_to(laptop.split('.').next().unwrap());
    assert!(
        wait_step_live(&local_home.join("loopflow.db")),
        "the laptop did not resume shipping"
    );

    assert_eq!(live_graph(&remote_db, &task), None, "a Graph runs on mini");
    let ships: i64 = rusqlite::Connection::open(&remote_db)
        .unwrap()
        .query_row(
            "SELECT count(*) FROM graph_processes WHERE task_id=?1",
            [task.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        ships, 1,
        "mini should run the Task only once, while it owns the Task"
    );
}

/// A Task move that continues a run starts it for real on mini: each home
/// gets its own server, so no launch installs one.
#[test]
fn task_move_leaves_a_live_graph_running_on_mini() {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let lf = env!("CARGO_BIN_EXE_lf");
    let local_home = fixture.root.path().join("local");
    let remote_home = fixture.root.path().join("remote/store");
    let _laptop = loopflow_test_support::MachineServer::start(lf, &local_home);
    let _mini = loopflow_test_support::MachineServer::start(lf, &remote_home);
    // Ends the run on mini (and anything left on the laptop), also when an
    // assertion fails.
    let _ended = EndRecorded(vec![local_home.clone(), remote_home.clone()]);
    let (task, worktree) = placed_task(&plan, "graph-task");
    let mut run = start_hold_run(&fixture, &task, &worktree);
    let remote_db = remote_home.join("loopflow.db");
    // What the step had written so far travels in the checkpoint.
    fs::write(worktree.join("step-edit.txt"), "halfway\n").unwrap();

    let moved = fixture
        .command(&["task", "move", task.as_str(), "--machine", "mini"])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    let stopped = exits_within_ten_seconds(&mut run);
    assert_success(&moved);
    assert!(stopped, "the Task run outlived its stopped Graph");
    let stderr = String::from_utf8_lossy(&moved.stderr);
    for stage in [
        "Checked mini: it can take",
        "at `",
        "Committed and pushed",
        "Arrived on mini",
        "Running on mini as Graph run",
    ] {
        assert!(stderr.contains(stage), "{stage:?} missing from:\n{stderr}");
    }
    assert_eq!(
        git(
            plan._repos[0].bare_path(),
            &["show", "graph-task:step-edit.txt"]
        ),
        "halfway"
    );

    // The Graph was running on mini when the move returned.
    assert!(
        live_graph(&remote_db, &task).is_some(),
        "no live Graph on mini when the move returned"
    );
    // Mini's Graph stands at the step the laptop stopped, not the route's start.
    let stopped_at = stderr
        .lines()
        .find_map(|line| line.strip_prefix("Stopped ")?.split_once(" at `"))
        .and_then(|(_, step)| step.strip_suffix('`'))
        .unwrap_or_else(|| panic!("no stopped step in:\n{stderr}"));
    assert_eq!(stopped_at, format!("task wait {task} --timeout 120s"));
    let mini_status = fixture
        .command(&[
            "--machine",
            "mini",
            "task",
            "status",
            task.as_str(),
            "--json",
        ])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert_success(&mini_status);
    let mini_status: Value = serde_json::from_slice(&mini_status.stdout).unwrap();
    assert_eq!(
        mini_status["graph_processes"][0]["step"], stopped_at,
        "{mini_status}"
    );
    // Once mini's `task wait` runs, a restarted route would already have
    // recorded its `task status` step.
    assert!(wait_step_live(&remote_db), "mini's step never started");
    assert_eq!(
        first_steps_run(&remote_db, &task),
        0,
        "mini ran the route's first step again"
    );
}

/// A move killed after it stopped the Graph, before it pushed: the laptop's
/// claim names mini and the move is leaving, with the step it stopped. Status
/// says so, and the same command carries the move on: the checkpoint is
/// pushed and the Graph continues on mini at the stopped step.
#[test]
fn a_move_killed_while_leaving_shows_in_status_and_the_same_command_carries_it_on() {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let lf = env!("CARGO_BIN_EXE_lf");
    let local_home = fixture.root.path().join("local");
    let remote_home = fixture.root.path().join("remote/store");
    let _laptop = loopflow_test_support::MachineServer::start(lf, &local_home);
    let _mini = loopflow_test_support::MachineServer::start(lf, &remote_home);
    let _ended = EndRecorded(vec![local_home.clone(), remote_home.clone()]);
    let mini = loopflow_execution::store::machines::local_machine(&plan.remote)
        .unwrap()
        .id;
    let (task, worktree) = placed_task(&plan, "leaving-task");
    let mut run = start_hold_run(&fixture, &task, &worktree);
    let step = format!("task wait {task} --timeout 120s");
    loopflow_execution::store::ownership::begin_task_move(&plan.local, &task, &mini).unwrap();
    loopflow_test_support::end_recorded_processes(&local_home, Path::new(lf));
    assert!(
        exits_within_ten_seconds(&mut run),
        "the laptop's run outlived its stop"
    );
    plan.local
        .record_move_stopped(&task, &serde_json::json!({"step": step}).to_string())
        .unwrap();
    fs::write(worktree.join("step-edit.txt"), "halfway\n").unwrap();

    let name = plan.local.task(&task).unwrap().unwrap().identifier;
    let retry = format!("lf task move {name} --machine mini");
    let status = fixture
        .command(&["task", "status", task.as_str()])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert_success(&status);
    let status = String::from_utf8_lossy(&status.stdout);
    assert!(status.contains("Moving to mini since"), "{status}");
    assert!(status.contains(": not sent"), "{status}");
    assert!(status.contains(&format!("retry: {retry}")), "{status}");

    let moved = fixture
        .command(&["task", "move", task.as_str(), "--machine", "mini"])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert_success(&moved);
    let stderr = String::from_utf8_lossy(&moved.stderr);
    for stage in [
        "Carrying on".to_string(),
        format!("Arrived on mini (Graph at `{step}`)"),
        "Running on mini as Graph run".into(),
    ] {
        assert!(stderr.contains(&stage), "{stage:?} missing from:\n{stderr}");
    }
    assert_eq!(
        git(
            plan._repos[0].bare_path(),
            &["show", "leaving-task:step-edit.txt"]
        ),
        "halfway"
    );
    assert_eq!(plan.local.task_move_in_flight(&task).unwrap(), None);
    let owner = loopflow_execution::store::ownership::task_ownership(&plan.remote, &task)
        .unwrap()
        .owner
        .unwrap();
    assert_eq!(owner.machine.id, Some(mini));
    let mini_status = fixture
        .command(&[
            "--machine",
            "mini",
            "task",
            "status",
            task.as_str(),
            "--json",
        ])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert_success(&mini_status);
    let mini_status: Value = serde_json::from_slice(&mini_status.stdout).unwrap();
    assert_eq!(
        mini_status["graph_processes"][0]["step"],
        step.as_str(),
        "{mini_status}"
    );
}

/// Back is the same act: the laptop runs `lf task move X --machine laptop`
/// while mini owns the Task, and mini's Graph continues on the laptop.
#[test]
fn a_moved_task_comes_back_with_the_same_command() {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let lf = env!("CARGO_BIN_EXE_lf");
    let local_home = fixture.root.path().join("local");
    let remote_home = fixture.root.path().join("remote/store");
    let _laptop = loopflow_test_support::MachineServer::start(lf, &local_home);
    let _mini = loopflow_test_support::MachineServer::start(lf, &remote_home);
    let _ended = EndRecorded(vec![local_home.clone(), remote_home.clone()]);
    let (task, worktree) = placed_task(&plan, "back-task");
    let mut run = start_hold_run(&fixture, &task, &worktree);
    let move_to = |machine: &str| {
        let output = fixture
            .command(&["task", "move", task.as_str(), "--machine", machine])
            .current_dir(&plan.local_path)
            .output()
            .unwrap();
        assert_success(&output);
        String::from_utf8_lossy(&output.stderr).into_owned()
    };
    move_to("mini");
    let stopped = exits_within_ten_seconds(&mut run);
    assert!(stopped, "the laptop's run outlived the move to mini");
    assert!(
        wait_step_live(&remote_home.join("loopflow.db")),
        "mini's step never started"
    );

    let laptop = gethostname::gethostname().to_string_lossy().into_owned();
    let laptop = laptop.split('.').next().unwrap().to_string();
    let stderr = move_to(&laptop);
    let step = format!("task wait {task} --timeout 120s");
    let identifier = plan.local.task(&task).unwrap().unwrap().identifier;
    for stage in [
        format!("Stopped {identifier} at `{step}`"),
        "Committed and pushed".into(),
        format!("Arrived on {laptop} (this Machine) (Graph at `{step}`)"),
        format!("Running on {laptop} (this Machine) as Graph run"),
    ] {
        assert!(stderr.contains(&stage), "{stage:?} missing from:\n{stderr}");
    }
    // Mini's Graph stopped, and the laptop's runs at the step mini was on.
    assert_eq!(
        live_graph(&remote_home.join("loopflow.db"), &task),
        None,
        "mini's Graph kept running"
    );
    let here = fixture
        .command(&["task", "status", task.as_str(), "--json"])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert_success(&here);
    let here: Value = serde_json::from_slice(&here.stdout).unwrap();
    assert_eq!(here["graph_processes"][0]["step"], step.as_str(), "{here}");
    assert!(
        wait_step_live(&local_home.join("loopflow.db")),
        "the laptop's step never started again"
    );
    // Mini says where the Task went. Forwarded from the laptop, the request
    // reads the laptop's preferences, which never name the laptop itself.
    let laptop_id = loopflow_execution::store::machines::local_machine(&plan.local)
        .unwrap()
        .id;
    let forwarded = fixture
        .command(&["--machine", "mini", "task", "status", task.as_str()])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert_success(&forwarded);
    let forwarded = String::from_utf8_lossy(&forwarded.stdout);
    assert!(
        forwarded.contains(&format!("{identifier}  Runs on {laptop_id} since ")),
        "{forwarded}"
    );
    // On mini, with the laptop registered there, it reads by that name.
    executable(
        &fixture.root.path().join("remote/.local/bin/ssh"),
        &format!(
            "#!/bin/sh\nprintf 'lf {}\\n{laptop_id}\\n'\n",
            loopflow::build_info::BUILD_VERSION
        ),
    );
    let on_mini = |args: &[&str]| {
        let output = fixture
            .command(args)
            .env("HOME", fixture.root.path().join("remote"))
            .env("LF_HOME", &remote_home)
            .env(
                "PATH",
                format!(
                    "{}:/usr/bin:/bin",
                    fixture.root.path().join("remote/.local/bin").display()
                ),
            )
            .current_dir(&plan.remote_path)
            .output()
            .unwrap();
        assert_success(&output);
        String::from_utf8_lossy(&output.stdout).into_owned()
    };
    on_mini(&["machine", "add", "laptop"]);
    // On mini, status reads the laptop's server, where the Task runs.
    fs::create_dir_all(local_home.join(".local/bin")).unwrap();
    executable(
        &local_home.join(".local/bin/lf"),
        &format!(
            "#!/bin/sh\nexport LF_HOME='{}' LF_BIN='{lf}'\nexec '{lf}' \"$@\"\n",
            local_home.display(),
        ),
    );
    executable(
        &fixture.root.path().join("remote/.local/bin/ssh"),
        &format!(
            "#!/bin/sh\nfor arg in \"$@\"; do remote_command=\"$arg\"; done\ncd '{}' || exit 1\nexec env -i HOME='{}' PATH=/usr/bin:/bin bash -c \"$remote_command\"\n",
            local_home.display(),
            local_home.display()
        ),
    );
    let there = on_mini(&["task", "status", task.as_str()]);
    assert!(
        there.contains(&format!(
            "· {identifier}\nRuns on laptop; read from its server."
        )),
        "{there}"
    );
}

/// Run a route on the laptop whose second step, `task wait`, holds until
/// something stops it; returns once that step is live.
fn start_hold_run(
    fixture: &Machines,
    task: &loopflow_execution::durable::TaskId,
    worktree: &Path,
) -> std::process::Child {
    fs::create_dir_all(worktree.join(".lf/graphs")).unwrap();
    fs::write(
        worktree.join(".lf/graphs/hold.yaml"),
        // A first step that finishes, so restarting at the stopped step differs
        // from restarting the route.
        format!("- cmd: task status {task}\n- cmd: task wait {task} --timeout 120s\n- interactive: demo\n"),
    )
    .unwrap();
    let mut run = fixture
        .command(&["task", "run", task.as_str(), "hold"])
        .current_dir(worktree)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if !wait_step_live(&fixture.root.path().join("local/loopflow.db")) {
        let _ = run.kill();
        let output = run.wait_with_output().unwrap();
        panic!(
            "the laptop's step never started: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    run
}

/// Whether `run` exits within ten seconds; kills it if not.
fn exits_within_ten_seconds(run: &mut std::process::Child) -> bool {
    let exited = (0..100).any(|_| {
        std::thread::sleep(std::time::Duration::from_millis(100));
        run.try_wait().unwrap().is_some()
    });
    if !exited {
        run.kill().unwrap();
    }
    exited
}

/// The carrier id of `task`'s running Graph in `db`.
fn live_graph(db: &Path, task: &loopflow_execution::durable::TaskId) -> Option<String> {
    use rusqlite::OptionalExtension;
    rusqlite::Connection::open(db)
        .unwrap()
        .query_row(
            "SELECT lf_process_id FROM task_graphs WHERE task_id=?1 AND run_pid IS NOT NULL",
            [task.as_str()],
            |row| row.get(0),
        )
        .optional()
        .unwrap()
}

fn first_steps_run(db: &Path, task: &loopflow_execution::durable::TaskId) -> i64 {
    rusqlite::Connection::open(db)
        .unwrap()
        .query_row(
            "SELECT count(*) FROM graph_process_steps s
             JOIN graph_processes g ON g.lf_process_id=s.graph_lf_process_id
             WHERE g.task_id=?1 AND s.node=0",
            [task.as_str()],
            |row| row.get(0),
        )
        .unwrap()
}

/// Whether the hold Graph's `task wait` step (node 1) recorded in `db` is
/// running within a minute.
fn wait_step_live(db: &Path) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while std::time::Instant::now() < deadline {
        let waiting: i64 = rusqlite::Connection::open(db)
            .and_then(|sql| {
                sql.query_row(
                    "SELECT count(*) FROM graph_process_steps
                     WHERE node=1 AND pid IS NOT NULL AND ended_at IS NULL",
                    [],
                    |row| row.get(0),
                )
            })
            .unwrap_or(0);
        if waiting > 0 {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    false
}

#[test]
fn resuming_a_task_conversation_on_mini_moves_its_task() {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let mini = loopflow_execution::store::machines::local_machine(&plan.remote)
        .unwrap()
        .id;
    let laptop = loopflow_execution::store::machines::local_machine(&plan.local)
        .unwrap()
        .id;
    let (task, worktree) = placed_task(&plan, "conversation-task");
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
    // The conversation took its Task along: both Machines say it runs on mini.
    assert_eq!(
        loopflow_execution::store::ownership::task_execution_route(&plan.local, &task)
            .unwrap()
            .machine_id,
        mini
    );
    assert_eq!(
        loopflow_execution::store::ownership::task_execution_route(&plan.remote, &task)
            .unwrap()
            .machine_id,
        mini
    );
    // Status reads mini's server, where the Task runs.
    let _mini = served_mini(&fixture);
    let status = fixture
        .command(&["task", "status", task.as_str()])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert_success(&status);
    let status = String::from_utf8_lossy(&status.stdout);
    assert!(
        status.contains("Runs on mini; read from its server."),
        "{status}"
    );
    let arrived = plan.remote.session(&session).unwrap().unwrap();
    assert_eq!(arrived.task_id.as_ref(), Some(&task));
    // The move was sent from the laptop and arrived on mini.
    assert!(plan.local.task_move_in_flight(&task).unwrap().is_none());
    for db in ["local/loopflow.db", "remote/store/loopflow.db"] {
        let moves: Vec<(String, String, String)> =
            rusqlite::Connection::open(fixture.root.path().join(db))
                .unwrap()
                .prepare(
                    "SELECT from_machine_id,to_machine_id,state FROM task_moves WHERE task_id=?1",
                )
                .unwrap()
                .query_map([task.as_str()], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
        assert_eq!(
            moves,
            [(laptop.to_string(), mini.to_string(), "arrived".to_string())],
            "{db}"
        );
    }
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

/// Mini is a rental that ends while it holds the Task: the Task comes back to
/// the laptop, and so does resuming its conversation, which never forwards to
/// the Machine it last moved to.
#[test]
fn a_handed_back_tasks_conversation_resumes_here() {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let mini = loopflow_execution::store::machines::local_machine(&plan.remote)
        .unwrap()
        .id;
    let laptop = loopflow_execution::store::machines::local_machine(&plan.local)
        .unwrap()
        .id;
    let (task, worktree) = placed_task(&plan, "handed-back");
    let session = claude_conversation(&fixture, &worktree, "0199a213-81c0-7800-8aa1-bbab2a035a5c");
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
    assert!(plan.local.session_moved_to(&session).unwrap().is_none());

    let rental = loopflow_execution::rental::RentalId::new();
    rusqlite::Connection::open(fixture.root.path().join("local/loopflow.db"))
        .unwrap()
        .execute(
            "INSERT INTO rentals(id,ondemand,provider,provider_name,by_hand,hardware,
                usd_per_hour,cap_usd,deadline,state,created_at,machine_id)
             VALUES(?1,'code','daytona',?1,0,
                '{\"vcpus\":4,\"memory_gb\":8,\"disk_gb\":10,\"gpu\":null,\"spot\":false}',
                '0.24','1.00',4000000000,'running',1,?2)",
            [rental.as_str(), mini.as_str()],
        )
        .unwrap();
    let (_, returned) = loopflow_execution::store::ownership::set_rental_state(
        &plan.local,
        &rental,
        loopflow_execution::rental::RentalState::Ended,
    )
    .unwrap();
    assert_eq!(returned, std::slice::from_ref(&task));
    assert_eq!(
        loopflow_execution::store::ownership::task_execution_route(&plan.local, &task)
            .unwrap()
            .machine_id,
        laptop
    );

    let resumed = fixture
        .command(&["resume", &session, "--prepare", "--json"])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    assert_success(&resumed);
    let resumed: Value = serde_json::from_slice(&resumed.stdout).unwrap();
    assert_eq!(resumed["id"], session.as_str());
    assert_eq!(resumed["workspace"]["machine_id"], laptop.as_str());
}

/// A Task with two conversations: resuming the one that is not its primary
/// on mini moves the Task with that one, and mini resumes it.
#[test]
fn resuming_a_tasks_other_conversation_on_mini_carries_that_one() {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let mini = loopflow_execution::store::machines::local_machine(&plan.remote)
        .unwrap()
        .id;
    let (task, worktree) = placed_task(&plan, "two-conversations");
    let first = claude_conversation(&fixture, &worktree, "0199a213-81c0-7800-8aa1-bbab2a035a5a");
    let other = claude_conversation(&fixture, &worktree, "0199a213-81c0-7800-8aa1-bbab2a035a5b");
    assert_ne!(first, other);

    let resumed = fixture
        .command(&["--machine", "mini", "resume", &other, "--prepare", "--json"])
        .current_dir(&worktree)
        .output()
        .unwrap();
    assert_success(&resumed);
    let resumed: Value = serde_json::from_slice(&resumed.stdout).unwrap();
    assert_eq!(resumed["id"], other.as_str());
    assert_eq!(
        loopflow_execution::store::ownership::task_execution_route(&plan.remote, &task)
            .unwrap()
            .machine_id,
        mini
    );
    let there = plan.remote.session(&other).unwrap().unwrap();
    assert!(there.completed_at.is_none());
    assert_eq!(there.task_id.as_ref(), Some(&task));
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
        loopflow_execution::store::open_sqlite(&fixture.root.path().join("local/loopflow.db"))
            .unwrap();
    loopflow_execution::repository::location::bind_repository(
        &local,
        repo.path().canonicalize().unwrap().to_str().unwrap(),
        &loopflow_execution::durable::RepositoryId::new(),
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
    let remote = loopflow_execution::store::open_sqlite(
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

/// A child process the fixture stops when it ends.
struct Child(std::process::Child);

impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The laptop's server, ready, and its socket.
fn served(fixture: &Machines) -> (Child, String) {
    use std::time::{Duration, Instant};
    let server = Child(fixture.command(&["server", "run"]).spawn().unwrap());
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let output = fixture.run(&["server", "status"]);
        let status: Value = serde_json::from_slice(&output.stdout).unwrap();
        if status["ready"] == true {
            return (server, status["socket"].as_str().unwrap().to_owned());
        }
        assert!(
            Instant::now() < deadline,
            "server did not become ready: {status}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn served_ownership_routes_to_its_writer_and_never_falls_back_locally() {
    use loopflow::lf::commands::write::Write as Mutation;
    use loopflow::server::wire::Operation;
    use loopflow_execution::machine::client::Connection;
    use std::io::Read;
    use std::time::{Duration, Instant};

    let fixture = preview_machines();
    let (plan, task) = task_on_mini(&fixture);
    let _mini = served_mini(&fixture);
    let (server, socket) = served(&fixture);
    let send = |claimed| {
        let mut connection = Connection::open_at(Path::new(&socket)).unwrap();
        connection
            .set_read_timeout(Some(Duration::from_secs(20)))
            .unwrap();
        connection.request(Operation::Write(if claimed {
            Mutation::TaskClaim {
                directory: plan.local_path.clone(),
                issue: task.to_string(),
            }
        } else {
            Mutation::TaskUnassign {
                directory: plan.local_path.clone(),
                issue: task.to_string(),
            }
        }))
    };
    let local = loopflow_execution::store::ownership::task_ownership(&plan.local, &task).unwrap();
    let ownership = send(true).unwrap().result;
    assert_eq!(
        ownership["owner"]["machine"]["id"],
        loopflow_execution::store::machines::local_machine(&plan.remote)
            .unwrap()
            .id
            .as_str()
    );
    assert_eq!(
        loopflow_execution::store::ownership::task_ownership(&plan.local, &task).unwrap(),
        local
    );
    // A CLI mutation that takes no body must finish with stdin still open,
    // as it is at a terminal. Sharing request I/O must not wait for EOF.
    let mut cli = Child(
        fixture
            .command(&["task", "claim", task.as_str(), "--json"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let _input = cli.0.stdin.take().unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = cli.0.try_wait().unwrap() {
            let mut error = String::new();
            cli.0
                .stderr
                .take()
                .unwrap()
                .read_to_string(&mut error)
                .unwrap();
            assert!(status.success(), "{error}");
            break;
        }
        assert!(
            Instant::now() < deadline,
            "remote claim waited for stdin EOF"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(cli);
    executable(
        &fixture.root.path().join("bin/ssh"),
        "#!/bin/sh\necho 'peer unavailable' >&2\nexit 255\n",
    );
    let refused = send(false)
        .unwrap_err()
        .downcast::<loopflow_execution::engine::Error>()
        .unwrap();
    assert!(
        refused.message.contains("peer unavailable") || refused.message.contains("unavailable"),
        "{refused:?}"
    );
    assert_eq!(
        loopflow_execution::store::ownership::task_ownership(&plan.local, &task).unwrap(),
        local
    );
    assert!(
        loopflow_execution::store::ownership::task_ownership(&plan.remote, &task)
            .unwrap()
            .owner
            .is_some()
    );
    drop(server);
}

/// A Task mini runs and mini's records claim, read back to the laptop.
fn task_on_mini(fixture: &Machines) -> (SharedPlan, loopflow_execution::durable::TaskId) {
    let plan = SharedPlan::new(fixture);
    let task = plan.task();
    loopflow_execution::store::ownership::claim_task(&plan.remote, &task)
        .unwrap()
        .unwrap();
    plan.to_local();
    (plan, task)
}

/// Mini's own server, ready, as `lf server connect` there finds it.
fn served_mini(fixture: &Machines) -> Child {
    use std::time::{Duration, Instant};
    let remote = fixture.root.path().join("remote");
    let lf = |args: &[&str]| {
        let mut command = Command::new(remote.join(".local/bin/lf"));
        command
            .env_clear()
            .env("HOME", &remote)
            .env("PATH", "/usr/bin:/bin")
            .current_dir(&remote)
            .args(args);
        command
    };
    let server = Child(lf(&["server", "run"]).spawn().unwrap());
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let status: Value =
            serde_json::from_slice(&lf(&["server", "status"]).output().unwrap().stdout).unwrap();
        if status["ready"] == true {
            return server;
        }
        assert!(
            Instant::now() < deadline,
            "mini's server did not become ready: {status}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Mini's `lf server connect` stood in: it welcomes the connection, records
/// the hello and the request it reads, and answers with `result`.
fn answering_entry(fixture: &Machines, record: &Path, result: &Value) {
    let welcome: Value = serde_json::from_str(include_str!(
        "../../loopflow-execution/tests/fixtures/dto/server/welcome.json"
    ))
    .unwrap();
    let mut response: Value = serde_json::from_str(include_str!(
        "../../loopflow-execution/tests/fixtures/dto/server/response.json"
    ))
    .unwrap();
    response["id"] = "@ID@".into();
    response["result"] = result.clone();
    let remote_lf = fixture.root.path().join("remote/.local/bin/lf");
    let working = fs::read_to_string(&remote_lf).unwrap();
    executable(
        &remote_lf,
        &working.replacen(
            "#!/bin/sh\n",
            &format!(
                r#"#!/bin/sh
case "$*" in *'server connect'*)
cat <<'WELCOME'
{welcome}
WELCOME
read -r hello; read -r request
printf '%s\n%s\n' "$hello" "$request" > '{input}'
id=$(printf '%s' "$request" | sed 's/^{{"type":"request","id":"\([^"]*\)".*/\1/')
sed "s/@ID@/$id/" <<'ANSWER'
{response}
ANSWER
exit 0;; esac
"#,
                input = record.join("input").display(),
            ),
            1,
        ),
    );
}

/// What a stood-in entry read: the hello, then the request.
fn entry_input(record: &Path) -> (Value, Value) {
    let input = fs::read_to_string(record.join("input")).unwrap();
    let mut lines = input
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap());
    (lines.next().unwrap(), lines.next().unwrap())
}

#[test]
fn a_placed_task_retries_on_its_owner_after_the_launch_connection_breaks() {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let task = plan.local_task();
    plan.to_remote();
    assert_success(&fixture.run(&["pool", "add", "lab"]));
    assert_success(&fixture.run(&["machine", "add", "mini", "--pool", "lab"]));
    let ssh = fixture.root.path().join("bin/ssh");
    let working = fs::read_to_string(&ssh).unwrap();
    executable(&ssh, "#!/bin/sh\nexit 88\n");
    let run = |placement: &[&str]| {
        let mut args = vec!["task", "run", task.as_str()];
        args.extend(placement);
        fixture
            .command(&args)
            .current_dir(&plan.local_path)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    };
    assert!(!run(&["--pool", "lab"]).status.success());
    let owner = loopflow_execution::store::ownership::task_ownership(&plan.local, &task).unwrap();
    assert_eq!(owner.log.len(), 1);
    let mini = loopflow_execution::store::machines::local_machine(&plan.remote)
        .unwrap()
        .id;
    assert_eq!(owner.log[0].machine_id.as_ref(), Some(&mini));

    let unreachable = run(&["--rent", "modal"]);
    assert!(!unreachable.status.success());
    assert!(
        String::from_utf8_lossy(&unreachable.stderr).contains("could not read"),
        "{unreachable:?}"
    );

    executable(&ssh, &working);
    let record = fixture.root.path().join("entry");
    fs::create_dir(&record).unwrap();
    let mut detail: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/dto/task_detail.json")).unwrap();
    detail["task"]["id"] = task.to_string().into();
    answering_entry(&fixture, &record, &detail);
    assert_success(&run(&["--rent", "modal"]));
    let (_, request) = entry_input(&record);
    assert_eq!(request["operation"]["kind"], "task.status");
    assert_eq!(request["operation"]["issue"], task.as_str());
    assert_eq!(
        loopflow_execution::store::ownership::task_ownership(&plan.local, &task).unwrap(),
        owner
    );
}

#[test]
fn a_hold_for_a_task_mini_runs_is_answered_by_minis_server() {
    let fixture = preview_machines();
    let (plan, task) = task_on_mini(&fixture);
    let _mini = served_mini(&fixture);
    let remote = fixture.root.path().join("remote");
    let record = fixture.root.path().join("ssh-record");
    fs::create_dir_all(&record).unwrap();
    executable(
        &fixture.root.path().join("bin/ssh"),
        &format!(
            r#"#!/bin/sh
for arg in "$@"; do remote_command="$arg"; done
printf '%s\n' "$remote_command" >> '{record}/commands'
cd '{remote}' || exit 1
tee -a '{record}/input' | env -i HOME='{remote}' PATH=/usr/bin:/bin bash -c "$remote_command"
"#,
            record = record.display(),
            remote = remote.display(),
        ),
    );
    let held = fixture.json(&["task", "auto", "off", task.as_str(), "--json"]);
    assert_eq!(
        held,
        serde_json::json!({"issue": task.as_str(), "was_on": false})
    );

    let commands = fs::read_to_string(record.join("commands")).unwrap();
    assert!(
        commands
            .lines()
            .any(|line| line == "exec 'lf' 'server' 'connect'"),
        "{commands}"
    );
    assert!(!commands.contains("auto"), "{commands}");
    let (hello, request) = entry_input(&record);
    assert_eq!(hello["client"]["kind"], "machine");
    assert_eq!(request["sender"]["repository"], plan.repository.as_str());
    assert_eq!(request["operation"]["kind"], "task.auto.off");
    assert_eq!(request["operation"]["issue"], task.as_str());
}

/// `lf task status --subscribe` for a Task mini runs prints the readings of
/// mini's server, over the same pipe a write takes.
#[test]
fn a_status_subscription_for_a_task_mini_runs_reads_minis_server() {
    use std::io::BufRead;
    let fixture = preview_machines();
    let (plan, task) = task_on_mini(&fixture);
    let _mini = served_mini(&fixture);
    let remote = fixture.root.path().join("remote");
    let record = fixture.root.path().join("ssh-record");
    fs::create_dir_all(&record).unwrap();
    executable(
        &fixture.root.path().join("bin/ssh"),
        &format!(
            r#"#!/bin/sh
for arg in "$@"; do remote_command="$arg"; done
printf '%s\n' "$remote_command" >> '{record}/commands'
cd '{remote}' || exit 1
exec env -i HOME='{remote}' PATH=/usr/bin:/bin bash -c "$remote_command"
"#,
            record = record.display(),
            remote = remote.display(),
        ),
    );
    let mut watching = Child(
        fixture
            .command(&[
                "task",
                "status",
                task.as_str(),
                "--subscribe",
                "--select",
                "task.id",
            ])
            .current_dir(&plan.local_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut line = String::new();
    std::io::BufReader::new(watching.0.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let frame: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(
        frame["body"],
        serde_json::json!({"task": {"id": task.as_str()}}),
        "{frame}"
    );
    // Its stdin is closed, so it ends after that first reading, as a local one does.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let status = loop {
        if let Some(status) = watching.0.try_wait().unwrap() {
            break status;
        }
        assert!(std::time::Instant::now() < deadline, "still subscribed");
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    assert!(status.success(), "{status}");
    let commands = fs::read_to_string(record.join("commands")).unwrap();
    assert!(
        commands
            .lines()
            .any(|line| line == "exec 'lf' 'server' 'connect'"),
        "{commands}"
    );
    assert!(!commands.contains("status"), "{commands}");
}

/// A one-shot `lf task status` for a Task mini runs is mini's reading: a
/// rename there shows before the laptop's planning copy hears of it.
#[test]
fn a_status_read_for_a_task_mini_runs_shows_a_rename_made_there() {
    let fixture = preview_machines();
    let (plan, task) = task_on_mini(&fixture);
    let current = plan.remote.task(&task).unwrap().unwrap();
    plan.remote
        .edit_task(
            &task,
            current.revision,
            &loopflow_execution::work::task::TaskUpdate {
                title: Some("Renamed on mini".into()),
                ..Default::default()
            },
        )
        .unwrap();
    let _mini = served_mini(&fixture);
    let remote = fixture.root.path().join("remote");
    let record = fixture.root.path().join("ssh-record");
    fs::create_dir_all(&record).unwrap();
    executable(
        &fixture.root.path().join("bin/ssh"),
        &format!(
            r#"#!/bin/sh
for arg in "$@"; do remote_command="$arg"; done
printf '%s\n' "$remote_command" >> '{record}/commands'
cd '{remote}' || exit 1
exec env -i HOME='{remote}' PATH=/usr/bin:/bin bash -c "$remote_command"
"#,
            record = record.display(),
            remote = remote.display(),
        ),
    );
    let status = |args: &[&str]| {
        let mut argv = vec!["task", "status", task.as_str()];
        argv.extend(args);
        let output = fixture
            .command(&argv)
            .current_dir(&plan.local_path)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };

    let json: Value = serde_json::from_str(&status(&["--json", "--select", "task.title"])).unwrap();
    assert_eq!(
        json,
        serde_json::json!({"task": {"title": "Renamed on mini"}})
    );
    let human = status(&[]);
    assert!(human.starts_with("Renamed on mini · "), "{human}");
    assert!(human.contains("Runs on mini"), "{human}");

    let commands = fs::read_to_string(record.join("commands")).unwrap();
    assert_eq!(
        commands
            .lines()
            .filter(|line| *line == "exec 'lf' 'server' 'connect'")
            .count(),
        2,
        "{commands}"
    );
    assert_eq!(
        plan.local.task(&task).unwrap().unwrap().title,
        "Ownership demo"
    );
}

/// A served `task.status` for a Task mini runs, asked on the laptop's socket,
/// is mini's answer.
#[test]
fn a_served_status_for_a_task_mini_runs_is_minis_answer() {
    use loopflow::lf::commands::read::Read;
    use loopflow::server::wire::Operation;
    use loopflow_execution::machine::client::Connection;

    let fixture = preview_machines();
    let (plan, task) = task_on_mini(&fixture);
    let answer = serde_json::json!({"task": {"title": "Renamed on mini"}});
    let record = fixture.root.path().join("entry");
    fs::create_dir_all(&record).unwrap();
    answering_entry(&fixture, &record, &answer);

    let (server, socket) = served(&fixture);
    let mut connection = Connection::open_at(Path::new(&socket)).unwrap();
    connection
        .set_read_timeout(Some(std::time::Duration::from_secs(20)))
        .unwrap();
    let read = connection
        .request(Operation::Read(Read::TaskStatus {
            directory: plan.local_path.clone(),
            issue: task.to_string(),
        }))
        .unwrap()
        .result;
    drop(server);

    assert_eq!(read, answer);
    let (_, request) = entry_input(&record);
    assert_eq!(request["operation"]["kind"], "task.status");
    assert_eq!(request["sender"]["repository"], plan.repository.as_str());
}

/// A served `task.run` for a Task mini runs is mini's `task.run`: the laptop
/// starts nothing and returns mini's answer.
#[test]
fn a_served_run_for_a_task_mini_runs_starts_it_there() {
    use loopflow::lf::commands::write::Write as Mutation;
    use loopflow::server::wire::Operation;
    use loopflow_execution::machine::client::Connection;

    let fixture = preview_machines();
    let (plan, task) = task_on_mini(&fixture);
    let mut running: Value = serde_json::from_str(include_str!(
        "../../loopflow-execution/tests/fixtures/dto/server/task_running.json"
    ))
    .unwrap();
    running["result"]["issue"] = task.as_str().into();
    let record = fixture.root.path().join("entry");
    fs::create_dir_all(&record).unwrap();
    answering_entry(&fixture, &record, &running["result"]);

    let (server, socket) = served(&fixture);
    let mut connection = Connection::open_at(Path::new(&socket)).unwrap();
    connection
        .set_read_timeout(Some(std::time::Duration::from_secs(20)))
        .unwrap();
    let started = connection
        .request(Operation::Write(Mutation::TaskRun {
            directory: plan.local_path.clone(),
            issue: task.to_string(),
            graph: None,
            from: None,
            agent: None,
            approve: false,
        }))
        .unwrap()
        .result;
    drop(server);

    assert_eq!(started, running["result"]);
    let (_, request) = entry_input(&record);
    assert_eq!(request["operation"]["kind"], "task.run");
    assert_eq!(request["operation"]["issue"], task.as_str());
    assert_eq!(
        request["sender"]["selection"]["participant"], "Jack",
        "{request}"
    );
    assert_eq!(
        live_graph(&fixture.root.path().join("local/loopflow.db"), &task),
        None
    );
}

#[test]
fn an_lf_without_the_entry_names_its_version_and_update() {
    let fixture = preview_machines();
    let (plan, task) = task_on_mini(&fixture);
    let mini = loopflow_execution::store::machines::local_machine(&plan.remote)
        .unwrap()
        .id;
    let ran = fixture.root.path().join("ran-locally");
    executable(
        &fixture.root.path().join("remote/.local/bin/lf"),
        &format!(
            r#"#!/bin/sh
case "$*" in
*"server connect"*) echo "error: unrecognized subcommand 'connect'" >&2; exit 2;;
esac
case "$1" in
--version) echo 'lf 0.15.2';;
machine) echo '{mini}';;
*) touch '{}';;
esac
"#,
            ran.display()
        ),
    );
    let output = fixture.run(&["task", "auto", "off", task.as_str(), "--json"]);
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("mini runs lf 0.15.2, which cannot take this operation; update it with `ssh -- 'mini' 'lf install'`"),
        "{error}"
    );
    assert!(!ran.exists());
}

#[test]
fn an_lf_that_dies_before_answering_says_so_and_what_it_printed() {
    let fixture = preview_machines();
    let (_plan, task) = task_on_mini(&fixture);
    let remote_lf = fixture.root.path().join("remote/.local/bin/lf");
    let working = fs::read_to_string(&remote_lf).unwrap();
    executable(
        &remote_lf,
        &working.replacen(
            "exec ",
            "case \"$*\" in *\"server connect\"*) echo 'panicked at main.rs' >&2; exit 101;; esac\nexec ",
            1,
        ),
    );
    let output = fixture.run(&["task", "auto", "off", task.as_str(), "--json"]);
    assert_eq!(output.status.code(), Some(1));
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains(
            "lf on mini exited with status 101 before answering; the destination said: panicked at main.rs; check `ssh mini lf server status`"
        ),
        "{error}"
    );
}

#[test]
fn an_unreachable_destination_names_the_ssh_check_not_its_server() {
    let fixture = preview_machines();
    let (_plan, task) = task_on_mini(&fixture);
    let remote_lf = fixture.root.path().join("remote/.local/bin/lf");
    let working = fs::read_to_string(&remote_lf).unwrap();
    executable(
        &remote_lf,
        &working.replacen(
            "exec ",
            "case \"$*\" in *\"server connect\"*) echo 'ssh: connect to host mini port 22: Connection refused' >&2; exit 255;; esac\nexec ",
            1,
        ),
    );
    let output = fixture.run(&["task", "auto", "off", task.as_str(), "--json"]);
    assert_eq!(output.status.code(), Some(1));
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains(
            "unreachable: run `ssh -o ControlPath=none -- 'mini'` to check the address, network and SSH service; ssh: connect to host mini port 22: Connection refused"
        ),
        "{error}"
    );
    assert!(!error.contains("destination said"), "{error}");
    assert!(!error.contains("lf server status"), "{error}");
}

#[test]
fn a_served_resume_opens_the_session_on_your_session_machine_as_a_typed_operation() {
    use loopflow::lf::commands::write::Write as Mutation;
    use loopflow::server::wire::Operation;
    use loopflow_execution::machine::client::Connection;

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
    let repository = String::from_utf8(identity.stdout).unwrap();
    let set = fixture
        .command(&["set", "machine.session", "mini"])
        .current_dir(&path)
        .output()
        .unwrap();
    assert_success(&set);

    let mut record: Value = serde_json::from_str::<Vec<Value>>(include_str!(
        "../../../tests/fixtures/dto/sessions.json"
    ))
    .unwrap()
    .remove(0);
    record["workspace"]["machine_id"] = machine["id"].clone();
    let record_dir = fixture.root.path().join("entry");
    fs::create_dir_all(&record_dir).unwrap();
    answering_entry(&fixture, &record_dir, &record);

    let (_server, socket) = served(&fixture);
    let mut connection = Connection::open_at(Path::new(&socket)).unwrap();
    connection
        .set_read_timeout(Some(std::time::Duration::from_secs(20)))
        .unwrap();
    let opened = connection
        .request(Operation::Write(Mutation::SessionResume {
            directory: path.clone(),
            id: None,
            task: None,
            wave: None,
            agent: None,
        }))
        .unwrap()
        .result;

    let (_, request) = entry_input(&record_dir);
    assert_eq!(request["sender"]["repository"], repository.trim());
    assert_eq!(request["operation"]["kind"], "session.resume");
    let argv: Vec<&str> = opened["open_argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|arg| arg.as_str().unwrap())
        .collect();
    assert_eq!(argv[0], env!("CARGO_BIN_EXE_lf"));
    assert_eq!(&argv[1..4], &["session", "attach", "--"]);
    assert!(argv.last().unwrap().contains("LF_VIEWER_STREAM='1'"));
    assert_eq!(opened["workspace"], record["workspace"]);
}

/// The held terminal of `owner` under `home`, as the owner names its socket.
fn server_socket(home: &Path, name: &str) -> std::path::PathBuf {
    use sha2::Digest;
    let digest = hex::encode(sha2::Sha256::digest(
        home.canonicalize().unwrap().as_os_str().as_encoded_bytes(),
    ));
    // SAFETY: geteuid has no preconditions.
    std::path::PathBuf::from(format!(
        "/tmp/lf-{}-{}/{name}.sock",
        unsafe { libc::geteuid() },
        &digest[..16]
    ))
}

/// Watch held `terminal` through the server's entrance as a viewer without
/// input control until its stream shows `line`, returning everything it showed.
fn watch_for(home: &Path, terminal: &str, line: String) -> std::thread::JoinHandle<String> {
    use std::io::Read;
    let mut socket = std::os::unix::net::UnixStream::connect(server_socket(home, "held")).unwrap();
    socket
        .write_all(
            format!(
                "{}\n",
                serde_json::json!({"terminal": terminal, "revision": loopflow_execution::transport::registry::REVISION})
            )
            .as_bytes(),
        )
        .unwrap();
    let mut answer = Vec::new();
    let mut byte = [0];
    while socket.read(&mut byte).unwrap() == 1 && byte[0] != b'\n' {
        answer.push(byte[0]);
    }
    let answer: serde_json::Value = serde_json::from_slice(&answer).unwrap();
    assert_eq!(answer["answer"], "hosted", "{terminal}: {answer}");
    let request = serde_json::json!({"id":1,"kind":"attach","rows":24,"cols":80}).to_string();
    let mut frame = vec![0x11];
    frame.extend((request.len() as u32).to_be_bytes());
    frame.extend(request.as_bytes());
    socket.write_all(&frame).unwrap();
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(1)))
        .unwrap();
    std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        let mut shown = Vec::new();
        let mut buffer = Vec::new();
        let mut chunk = [0; 8192];
        while std::time::Instant::now() < deadline
            && !String::from_utf8_lossy(&shown).contains(&line)
        {
            match socket.read(&mut chunk) {
                Ok(0) => break,
                Ok(read) => buffer.extend_from_slice(&chunk[..read]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(error) => panic!("viewer read failed: {error}"),
            }
            // Frames: a tag, a big-endian length, the payload. 0x02 and 0x03
            // carry terminal bytes; 0x06 carries lf's own lines, such as a
            // departure notice.
            while buffer.len() >= 5 {
                let length = u32::from_be_bytes(buffer[1..5].try_into().unwrap()) as usize;
                if buffer.len() < 5 + length {
                    break;
                }
                if matches!(buffer[0], 0x02 | 0x03 | 0x06) {
                    shown.extend_from_slice(&buffer[5..5 + length]);
                }
                buffer.drain(..5 + length);
            }
        }
        String::from_utf8_lossy(&shown).into_owned()
    })
}

/// Ends what the fixture's homes recorded, also when an assertion fails.
struct EndRecorded(Vec<std::path::PathBuf>);

impl Drop for EndRecorded {
    fn drop(&mut self) {
        for home in &self.0 {
            loopflow_test_support::end_recorded_processes(
                home,
                Path::new(env!("CARGO_BIN_EXE_lf")),
            );
        }
    }
}

/// The laptop terminal showing the Task's conversation is told where it went
/// and how to open it there.
#[test]
fn the_terminal_showing_a_moved_task_says_how_to_open_it_on_mini() {
    let fixture = preview_machines();
    let plan = SharedPlan::cloned(&fixture);
    let lf = env!("CARGO_BIN_EXE_lf");
    let local_home = fixture.root.path().join("local");
    let remote_home = fixture.root.path().join("remote/store");
    let _laptop = loopflow_test_support::MachineServer::start(lf, &local_home);
    let _mini = loopflow_test_support::MachineServer::start(lf, &remote_home);
    let _ended = EndRecorded(vec![local_home.clone(), remote_home]);
    let (task, worktree) = placed_task(&plan, "viewed-task");
    let identifier = plan.local.task(&task).unwrap().unwrap().identifier;
    // A stand-in Claude that waits on its terminal; it never asks about trust.
    fs::write(
        local_home.join(".claude.json"),
        serde_json::json!({"projects": {"/": {"hasTrustDialogAccepted": true}}}).to_string(),
    )
    .unwrap();
    executable(
        &fixture.root.path().join("bin/claude"),
        r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
printf '%s\n' "$*" > "$HOME/claude-args"
stty -echo < /dev/tty
printf 'ready\r\n'
read -r answer < /dev/tty
"#,
    );
    // The conversation runs in a held terminal, as a Task conversation a
    // terminal views does.
    let path = format!(
        "{}:/usr/bin:/bin",
        fixture.root.path().join("bin").display()
    );
    let started = loopflow_execution::machine::client::Connection::open_at(&server_socket(
        &local_home,
        "machine",
    ))
    .unwrap()
    .request(loopflow::server::wire::Operation::Server(
        loopflow::server::wire::ServerOperation::SessionStart {
            session: None,
            request: uuid::Uuid::new_v4().to_string(),
            argv: [lf, "-i", "-a", "claude", ":", "keep going"]
                .iter()
                .map(|arg| arg.to_string())
                .collect(),
            cwd: worktree.clone(),
            env: [
                ("HOME", local_home.to_str().unwrap()),
                ("LF_HOME", local_home.to_str().unwrap()),
                ("LF_BIN", lf),
                ("PATH", path.as_str()),
                ("RUST_LOG", "off"),
            ]
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect(),
            rows: 24,
            cols: 80,
            placement: None,
        },
    ))
    .unwrap();
    let terminal = started.result["terminal_id"].as_str().unwrap().to_owned();
    let args = local_home.join("claude-args");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while fs::read_to_string(&args).map_or(true, |text| text.is_empty()) {
        assert!(
            std::time::Instant::now() < deadline,
            "Claude never started in {terminal}"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let local_db = rusqlite::Connection::open(local_home.join("loopflow.db")).unwrap();
    let (session, task_of): (String, Option<String>) = local_db
        .query_row(
            "SELECT id,task_id FROM agent_sessions WHERE held_terminal_id=?1",
            [&terminal],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    if task_of.as_deref() != Some(task.as_str()) {
        assert_success(
            &fixture
                .command(&["session", "bind", &session, "--task", task.as_str()])
                .current_dir(&worktree)
                .output()
                .unwrap(),
        );
    }
    // The transcript Claude would have written for the conversation lf named.
    let args = fs::read_to_string(&args).unwrap();
    let words: Vec<&str> = args.split_whitespace().collect();
    let thread = words
        .windows(2)
        .find(|pair| matches!(pair[0], "--session-id" | "--resume"))
        .map(|pair| pair[1])
        .unwrap_or_else(|| panic!("no conversation id in Claude's arguments: {args}"));
    let project = local_home
        .join(".claude/projects")
        .join(claude_project(&worktree));
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join(format!("{thread}.jsonl")),
        format!(
            "{{\"type\":\"user\",\"sessionId\":\"{thread}\",\"cwd\":\"{}\",\"message\":{{\"role\":\"user\",\"content\":\"keep going\"}}}}\n",
            worktree.display()
        ),
    )
    .unwrap();
    let notice = format!("This conversation moved to mini: lf resume --task {identifier}");
    let viewer = watch_for(&local_home, &terminal, notice.clone());

    let moved = fixture
        .command(&["task", "move", task.as_str(), "--machine", "mini"])
        .current_dir(&plan.local_path)
        .output()
        .unwrap();
    let shown = viewer.join().unwrap();

    assert_success(&moved);
    assert!(shown.contains(&notice), "{notice:?} missing from:\n{shown}");
}

#[test]
fn ending_a_rental_its_provider_terminated_settles_and_leaves_the_default_listings() {
    use loopflow_execution::rental::{RentalId, RentalState};
    use loopflow_execution::store::{machines, open_sqlite};
    use serde_json::json;

    let fixture = preview_machines();
    let root = fixture.root.path();
    assert_success(&fixture.run(&[
        "ondemand",
        "add",
        "modal",
        "--provider",
        "modal",
        "--vcpus",
        "2",
        "--memory-gb",
        "4",
        "--max",
        "1",
        "--max-usd-per-hour",
        "1.00",
    ]));
    let local = open_sqlite(&root.join("local/loopflow.db")).unwrap();
    let pod = open_sqlite(&root.join("remote/store/loopflow.db")).unwrap();
    let origin = machines::local_machine(&pod).unwrap().id;
    drop(pod);
    let rental = RentalId::new();
    let created = chrono::Utc::now().timestamp() - 3600;
    local
        .conn
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO rentals(id,ondemand,provider,provider_name,provider_id,by_hand,hardware,
            usd_per_hour,cap_usd,deadline,state,created_at,machine_id)
         VALUES(?1,'modal','modal',?1,'sb-fixture',1,
            '{\"vcpus\":2,\"memory_gb\":4,\"disk_gb\":8,\"spot\":false,\"modal_sizing\":\"burst\"}',
            '0.24','1.00',?3,'running',?4,?2)",
            rusqlite::params![
                rental.as_str(),
                origin.as_str(),
                created + 4 * 3600,
                created
            ],
        )
        .unwrap();

    // Modal terminated the sandbox: SSH reaches nothing and the provider knows no such sandbox.
    fs::rename(root.join("remote"), root.join("unreachable")).unwrap();
    executable(
        &root.join("bin/uv"),
        "#!/bin/sh\ncat >/dev/null\nprintf 'null\\n'\n",
    );
    let ended = fixture
        .command(&["machine", "end", "mini"])
        .env("MODAL_TOKEN_ID", "fixture")
        .output()
        .unwrap();
    assert_success(&ended);
    let said = String::from_utf8_lossy(&ended.stdout);
    assert!(
        said.contains("mini was already gone (ended; ceiling estimate $0.24"),
        "{said}"
    );
    assert_eq!(local.rental(&rental).unwrap().state, RentalState::Ended);

    // The default listings leave its dead route out; its label still resolves.
    assert_eq!(fixture.json(&["machine", "list", "--json"]), json!([]));
    assert_eq!(fixture.json(&["machine", "status", "--json"]), json!([]));
    let listed = fixture.command(&["machine", "list"]).output().unwrap();
    assert_success(&listed);
    assert_eq!(String::from_utf8_lossy(&listed.stdout), "No Machines.\n");
    assert_eq!(
        fixture.json(&["machine", "list", "mini", "--json"])[0]["label"],
        "mini"
    );
    assert_eq!(
        fixture.json(&["machine", "status", "mini", "--json"])[0]["state"],
        "ended_rental"
    );
    let ondemands = fixture.json(&["ondemand", "list", "--json"]);
    let rentals = &ondemands[0]["rentals"];
    assert_eq!(rentals[0]["machine"], "mini", "{ondemands}");
    assert_eq!(rentals[0]["state"], "ended");
}

#[test]
fn a_lost_rentals_transcript_and_usage_survive_on_its_collecting_machine() {
    use loopflow_execution::rental::{recovery, RentalId, RentalState};
    use loopflow_execution::store::{machines, open_sqlite};
    use serde_json::json;

    let fixture = preview_machines();
    let root = fixture.root.path();
    let remote = root.join("remote");
    let local = open_sqlite(&root.join("local/loopflow.db")).unwrap();
    let pod = open_sqlite(&remote.join("store/loopflow.db")).unwrap();
    let origin = machines::local_machine(&pod).unwrap().id;
    let rental = RentalId::new();
    local
        .conn
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO rentals(id,ondemand,provider,provider_name,provider_id,by_hand,hardware,
            usd_per_hour,cap_usd,deadline,state,created_at,machine_id)
         VALUES(?1,'modal','modal',?1,'sb-fixture',1,
            '{\"vcpus\":2,\"memory_gb\":4,\"disk_gb\":8,\"spot\":false}',
            '0.24','1.00',4000000000,'running',1,?2)",
            [rental.as_str(), origin.as_str()],
        )
        .unwrap();

    let id = "ee47b779adc741ca89d6c3ae288cc68c";
    let thread = "0199a213-81c0-4800-8aa1-bbab2a035a01";
    let started = chrono::Utc::now().timestamp() - 3600;
    let session = pod.test_session_at(id, "64f34281b74d4594ac53ec251cb05619", started - 5);
    pod.conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE agent_sessions SET provider='claude',provider_thread=?2,
                start_failure='Waiting for a harness: codex has no usable login' WHERE id=?1",
            [id, thread],
        )
        .unwrap();
    let request = r#"Count the marbles in "jar A""#;
    let project = remote.join(".claude/projects/-fixture");
    fs::create_dir_all(&project).unwrap();
    let transcript = project.join(format!("{thread}.jsonl"));
    fs::write(
        &transcript,
        json!({"type":"user","userType":"external","uuid":"first-turn",
        "timestamp":"2026-10-16T10:00:00Z","message":{"role":"user","content":request}})
        .to_string(),
    )
    .unwrap();
    let repo = loopflow_test_support::TestRepo::new();
    let local_repo = repo.path().canonicalize().unwrap();
    let task_id = SharedPlan::create_task(&local, &local_repo);
    let local_repo = local_repo.to_str().unwrap().to_owned();
    let task = local.task(&task_id).unwrap().unwrap();
    let project = local
        .project(task.project_id.as_ref().unwrap())
        .unwrap()
        .unwrap();
    pod.record_turn_start(id, &thread.into(), "first-turn", started);
    let turn_origin = loopflow_storage::session::SessionTurnOrigin {
        session_id: id.into(),
        lf_process_id: loopflow_model::id::LfProcessId::new(),
        agent_process_id: loopflow_model::id::AgentProcessId::new(),
        captured_event: None,
        task_id: Some(task_id.to_string()),
        wave_id: None,
        project_id: None,
        repository: None,
        skill: None,
        provider: Some("claude".into()),
        graph: None,
        step: None,
    };
    pod.record_session_turn_origin(&thread.into(), "first-turn", &turn_origin);
    // The Session's captured input ran two more turns, one before it joined the Task.
    let input = session.artifact_key.clone().unwrap();
    let captured = pod.captured_sequence(&input).unwrap().unwrap();
    for (turn, at, task, input_tokens, output_tokens) in [
        ("unscoped-turn", started + 10, None, 13, 4),
        (
            "captured-turn",
            started + 20,
            Some(task_id.to_string()),
            11,
            2,
        ),
    ] {
        pod.record_turn_start(id, &thread.into(), turn, at);
        pod.record_session_turn_origin(
            &thread.into(),
            turn,
            &loopflow_storage::session::SessionTurnOrigin {
                lf_process_id: loopflow_model::id::LfProcessId::new(),
                agent_process_id: loopflow_model::id::AgentProcessId::new(),
                captured_event: Some(captured),
                task_id: task,
                ..turn_origin.clone()
            },
        );
        pod.record_turn_completion(
            id,
            &thread.into(),
            turn,
            at + 1,
            &json!({"status":"completed","context":{"first_request_tokens":input_tokens,
                "peak_request_tokens":input_tokens,"tool_results":0,"tool_output_tokens":0,
                "compactions":0,"compaction_tokens":null}}),
        );
        assert!(pod.record_turn_usage(
            id,
            &thread.into(),
            turn,
            at + 1,
            &json!({"final":true,"usage":{"input_tokens":input_tokens,"output_tokens":output_tokens}})
        ));
    }
    pod.retain_session_observation(
        &session,
        &loopflow_storage::session::SessionObservation {
            artifact_key: session.artifact_key.clone().unwrap(),
            source: "silence:sample".into(),
            observed_at: started + 1,
            task_id: None,
            wave_id: None,
            payload: json!({"source":"silence","detail":"waiting for the count"}),
        },
    )
    .unwrap();

    // The rental assembled the input's context; collection copies its capture.
    let capture =
        loopflow_execution::session_record::record_dir(&remote.join("store"), &input).unwrap();
    fs::create_dir_all(&capture).unwrap();
    fs::write(
        capture.join("context.json"),
        json!({"context":{"system":null,"task":{"assets":[]},"decisions":[]}}).to_string(),
    )
    .unwrap();

    let _env = support::ambient::EnvGuard::clear(&["PATH", "MODAL_TOKEN_ID"]);
    std::env::set_var(
        "PATH",
        format!("{}:/usr/bin:/bin", root.join("bin").display()),
    );
    let (server, _) = served(&fixture);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let first = loop {
        if let Some(cursor) = local.copied_cursor(&origin).unwrap() {
            break cursor;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the server did not collect its running rental"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    drop(server);
    let ssh = root.join("bin/ssh");
    let relay = fs::read_to_string(&ssh).unwrap();
    // SSH delivers valid lines, then loses the connection before confirming the export.
    executable(
        &ssh,
        &format!("{}\nexit 255\n", relay.replace("exec env -i", "env -i")),
    );
    assert!(pod.record_turn_usage(
        id,
        &thread.into(),
        "first-turn",
        started + 2,
        &json!({
            "final":true,"usage":{"input_tokens":31,"output_tokens":7,"cost_usd":0.25}
        })
    ));
    recovery::check().unwrap();
    assert_eq!(local.copied_cursor(&origin).unwrap(), Some(first));
    // The first turn's late usage did not arrive; the input's turns did.
    assert_eq!(
        local
            .saved_copied_history(id)
            .unwrap()
            .unwrap()
            .usage
            .input_tokens,
        Some(24)
    );
    executable(&ssh, &relay);
    recovery::check().unwrap();
    let latest = local.copied_cursor(&origin).unwrap().unwrap();
    assert!(latest.history > first.history);

    // Nothing on the origin can answer these reads, including after a server restart.
    let lost = root.join("unreachable");
    fs::rename(&remote, &lost).unwrap();
    let (server, socket) = served(&fixture);
    let history = fixture.json(&["session", "history", "show", id, "--json"]);
    assert_eq!(history["session"]["origin"], origin.as_str());
    assert_eq!(history["turns"].as_array().unwrap().len(), 3);
    assert!(history["events"]
        .to_string()
        .contains("waiting for the count"));
    let text = fixture.run(&["session", "history", "show", id, "--transcript"]);
    assert_success(&text);
    assert!(String::from_utf8_lossy(&text.stdout).contains("Count the marbles"));
    let search = |options: &[&str]| {
        let mut args = vec!["session", "history", "search", "MARBLES", "--json"];
        args.extend_from_slice(options);
        fixture.json(&args)["hits"].clone()
    };
    for scope in [
        ["--since", "1d"],
        ["--task", task.identifier.as_str()],
        ["--project", project.slug.as_str()],
        ["--wave", "owners"],
    ] {
        let hits = search(&scope);
        assert_eq!(hits.as_array().unwrap().len(), 1, "{scope:?}: {hits}");
        assert_eq!(hits[0]["session_id"], id);
        assert_eq!(hits[0]["snippet"], request);
        assert_eq!(
            hits[0]["origin"],
            json!({"machine_id":origin, "label":"mini"})
        );
    }
    for scope in [
        ["--task", "MISSING-123"],
        ["--wave", "unrelated"],
        ["--project", "unrelated"],
        ["--since", "2099-01-01T00:00:00Z"],
        ["--limit", "0"],
    ] {
        assert_eq!(search(&scope), json!([]), "{scope:?}");
    }
    assert_eq!(
        search(&["--task", &task.identifier, "--since", "30m"]),
        json!([]),
        "collecting an hour-old input now must not move it into the last half hour"
    );
    let result = fixture.run(&["session", "history", "search", r#""JAR A""#]);
    assert_success(&result);
    assert!(String::from_utf8_lossy(&result.stdout).contains("from mini"));
    let scoped_usage = |scope: &[&str]| {
        let mut args = vec!["session", "usage", "--json"];
        args.extend_from_slice(scope);
        fixture.json(&args)
    };
    // Work scopes count only the input's turns that ran for the Task.
    for (scope, input_usage) in [
        (["--task", task.identifier.as_str()], (11, 2)),
        (["--wave", "owners"], (11, 2)),
        (["--project", project.slug.as_str()], (11, 2)),
        (["--days", "1"], (24, 6)),
    ] {
        let rows = scoped_usage(&scope);
        let rows: Vec<_> = rows
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["session_id"] == id)
            .collect();
        assert_eq!(rows.len(), 2, "{scope:?}: {rows:?}");
        let (turn, captured_input) = (rows[0], rows[1]);
        for row in &rows {
            assert_eq!(row["origin"], json!({"machine_id":origin, "label":"mini"}));
        }
        assert_eq!(turn["artifact_key"], Value::Null);
        assert_eq!(turn["task_identifier"], task.identifier);
        assert_eq!(turn["observed_at"], started);
        assert_eq!(turn["usage"]["input_tokens"], 31);
        assert_eq!(turn["usage"]["output_tokens"], 7);
        assert_eq!(turn["usage"]["cost_usd"], 0.25);
        assert_eq!(captured_input["artifact_key"], input);
        assert_eq!(captured_input["observed_at"], started - 5);
        // The Task lives here, so its rows name this Machine's checkout.
        for row in &rows {
            assert_eq!(row["repo"], local_repo, "{scope:?} {row}");
        }
        assert_eq!(
            captured_input["usage"]["input_tokens"], input_usage.0,
            "{scope:?}"
        );
        assert_eq!(
            captured_input["usage"]["output_tokens"], input_usage.1,
            "{scope:?}"
        );
        assert_eq!(captured_input["usage"]["cost_usd"], Value::Null);
    }
    let mut connection =
        loopflow_execution::machine::client::Connection::open_at(Path::new(&socket)).unwrap();
    let desktop = connection
        .request(loopflow::server::wire::Operation::Read(
            loopflow::lf::commands::read::Read::SessionUsage {
                days: 0,
                wave: None,
                project: None,
                task: Some(task.identifier.clone()),
            },
        ))
        .unwrap()
        .result;
    assert_eq!(
        desktop,
        scoped_usage(&["--days", "0", "--task", &task.identifier])
    );
    // The copied capture and turn contexts explain the step with nothing missing.
    let context = scoped_usage(&["--context", "--task", &task.identifier]);
    let step = context["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["input"] == input)
        .unwrap_or_else(|| panic!("{context}"));
    assert_eq!(step["gaps"], json!([]), "{step}");
    assert_eq!(step["first_request_tokens"], 13);
    assert_eq!(step["peak_request_tokens"], 13);
    // The Session's own history view reads the same copied step.
    let shown = fixture.json(&["session", "history", "show", id, "--context", "--json"]);
    assert_eq!(shown, *step);
    // Why the rental could not start it outlives the rental.
    let listed = fixture.run(&["session", "list"]);
    assert_success(&listed);
    let listed = String::from_utf8_lossy(&listed.stdout);
    assert!(
        listed.contains("from mini\n  could not start: codex has no usable login\n"),
        "{listed}"
    );
    let human = fixture.run(&["session", "usage", "--task", &task.identifier]);
    assert_success(&human);
    assert!(String::from_utf8_lossy(&human.stdout).contains("from mini"));
    let weekly = scoped_usage(&["--weekly", "--task", &task.identifier]);
    let weekly_input: i64 = weekly["weeks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|week| week["total"]["input_tokens"].as_i64().unwrap())
        .sum();
    assert_eq!(weekly_input, 42);
    let usage = || fixture.json(&["session", "usage", "--session", id, "--json"]);
    let before = usage();
    assert_eq!(before["origin_machine_id"], origin.as_str());
    assert!(before["collected_at"].is_number());
    assert_eq!(before["usage"]["input_tokens"], 55);
    assert_eq!(before["usage"]["output_tokens"], 13);
    assert_eq!(before["usage"]["cost_usd"], 0.25);
    assert_eq!(local.rental(&rental).unwrap().state, RentalState::Running);
    drop(server);

    // Restore the route to exercise ordinary give-back on the same records.
    fs::rename(lost, &remote).unwrap();
    recovery::check().unwrap();
    // The provider still runs it until the give-back deletes it.
    executable(
        &root.join("bin/uv"),
        "#!/bin/sh\ncat >/dev/null\nfor op; do :; done\n\
         [ \"$op\" = get ] && echo '{\"id\":\"sb-fixture\",\"gone\":false,\"route\":null}' || echo null\n",
    );
    let ended = fixture
        .command(&["machine", "end", "mini"])
        .env("MODAL_TOKEN_ID", "fixture")
        .output()
        .unwrap();
    assert_success(&ended);
    assert!(String::from_utf8_lossy(&ended.stdout).contains("Returned mini"));
    assert_eq!(local.rental(&rental).unwrap().state, RentalState::Ended);
    assert_eq!(usage()["usage"], before["usage"]);
    let transcript = fixture.json(&["session", "history", "show", id, "--transcript", "--json"]);
    assert_eq!(transcript["turns"].as_array().unwrap().len(), 1);
    assert_eq!(transcript["origin_machine_id"], origin.as_str());

    // Continuing the provider thread locally re-observes the first turn's
    // transcript. Its dispatch, Work and usage were recorded on the rental.
    local.test_session(id, "14208d263a454517a4e9eb974ed4dc93");
    local.record_turn_start(id, &thread.into(), "first-turn", started);
    let second = started + 120;
    local.record_turn_start(id, &thread.into(), "second-turn", second);
    local.record_session_turn_origin(
        &thread.into(),
        "second-turn",
        &loopflow_storage::session::SessionTurnOrigin {
            lf_process_id: loopflow_model::id::LfProcessId::new(),
            agent_process_id: loopflow_model::id::AgentProcessId::new(),
            ..turn_origin
        },
    );
    local.record_turn_completion(
        id,
        &thread.into(),
        "second-turn",
        second + 1,
        &json!({"status":"completed"}),
    );
    assert!(local.record_turn_usage(
        id,
        &thread.into(),
        "second-turn",
        second + 1,
        &json!({"final":true,"usage":{"input_tokens":5,"output_tokens":3}})
    ));
    let combined = scoped_usage(&["--task", &task.identifier]);
    let rows = combined.as_array().unwrap();
    assert_eq!(rows.len(), 3, "{combined}");
    assert_eq!(rows[0]["origin"], Value::Null);
    assert_eq!(rows[0]["usage"]["input_tokens"], 5);
    assert_eq!(rows[0]["usage"]["cost_usd"], Value::Null);
    assert_eq!(rows[1]["origin"]["machine_id"], origin.as_str());
    assert_eq!(rows[1]["usage"]["input_tokens"], 31);
    assert_eq!(rows[2]["origin"]["machine_id"], origin.as_str());
    assert_eq!(rows[2]["artifact_key"], input);
    assert_eq!(rows[2]["usage"]["input_tokens"], 11);
    let mixed = usage();
    assert!(mixed["origin_machine_id"].is_null());
    assert_eq!(mixed["origins"].as_array().unwrap().len(), 2);
    assert_eq!(mixed["usage"]["streams"], 4);
    assert_eq!(mixed["usage"]["input_tokens"], 60);
    assert_eq!(mixed["usage"]["output_tokens"], 16);
    assert_eq!(mixed["usage"]["cost_usd"], 0.25);
    let recent = |since| {
        loopflow_execution::store::sessions::recent_conversation_history(
            &local,
            None,
            None,
            Some(&task.identifier),
            since,
            1,
        )
        .unwrap()
    };
    let (rows, truncated) = recent(started);
    assert!(truncated);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].origin.as_ref().unwrap().machine_id, origin);
    let (rows, truncated) = recent(second);
    assert!(!truncated);
    assert_eq!(rows.len(), 1);
    assert!(rows[0].origin.is_none());
}
