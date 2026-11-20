//! A copy of the Machine: `lf-world copy` and the server that serves it.
//! The Machine here is a built World.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use loopflow_execution::machine::client::Connection;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::world_recipes::{clean, success};

fn digest(path: &Path) -> String {
    hex::encode(Sha256::digest(std::fs::read(path).unwrap()))
}

/// Every checkout's commit and uncommitted files.
fn checkouts(repo: &Path) -> String {
    let git = |cwd: &Path, args: &[&str]| {
        success(
            Command::new("git")
                .current_dir(cwd)
                .args(args)
                .output()
                .unwrap(),
        )
    };
    let listed = git(repo, &["worktree", "list", "--porcelain"]);
    listed
        .lines()
        .filter_map(|line| line.strip_prefix("worktree "))
        .map(|checkout| {
            let status = git(
                Path::new(checkout),
                &["status", "--porcelain", "--untracked-files=all"],
            );
            format!("{checkout}\n{status}")
        })
        .collect::<String>()
        + &listed
}

struct Copied {
    root: tempfile::TempDir,
    machine: PathBuf,
    copy: PathBuf,
    /// A `claude` that records that it ran, ahead of everything else.
    path: String,
    provider_ran: PathBuf,
    /// The provider of a Task conversation running on the Machine.
    running: String,
}

/// Write `script` as the `claude` in a new `name` under `root`, and return
/// the `PATH` that finds it first.
fn provider(root: &Path, name: &str, script: &str) -> String {
    let bin = root.join(name);
    std::fs::create_dir(&bin).unwrap();
    std::fs::write(
        bin.join("claude"),
        format!(
            "#!/bin/sh\n[ \"$1\" = --version ] && echo '2.1.0 (Claude Code)' && exit 0\n{script}\n"
        ),
    )
    .unwrap();
    std::fs::set_permissions(
        bin.join("claude"),
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .unwrap();
    format!("{}:/usr/bin:/bin", bin.display())
}

fn alive(pid: &str) -> bool {
    Command::new("/bin/kill")
        .args(["-0", pid])
        .output()
        .unwrap()
        .status
        .success()
}

impl Drop for Copied {
    fn drop(&mut self) {
        let _ = std::fs::write(self.root.path().join("stop"), "");
    }
}

impl Copied {
    /// A Machine with one Task conversation running and one that was
    /// prepared and never started, which a server on a writable store starts
    /// at its first reading.
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join(".gitconfig"),
            "[user]\n\tname = Demo\n\temail = demo@example.com\n",
        )
        .unwrap();
        let machine = root.path().join("machine");
        success(
            clean(env!("CARGO_BIN_EXE_lf-world"), root.path())
                .args(["build", "one-task", machine.to_str().unwrap()])
                .output()
                .unwrap(),
        );
        let provider_ran = root.path().join("provider-ran");
        let path = provider(
            root.path(),
            "recording",
            &format!("echo \"$@\" >> '{}'", provider_ran.display()),
        );
        let mut copied = Self {
            path,
            copy: root.path().join("copy"),
            root,
            machine,
            provider_ran,
            running: String::new(),
        };
        let root = copied.root.path().to_path_buf();

        // The provider holds until the fixture ends, as a real one does.
        let pid = root.join("running-pid");
        let holding = provider(
            &root,
            "holding",
            &format!(
                "echo $$ > '{}'\nwhile [ -d '{root}' ] && [ ! -e '{root}/stop' ]; do sleep 0.1; done",
                pid.display(),
                root = root.display()
            ),
        );
        let server = loopflow_test_support::MachineServer::start_with_path(
            env!("CARGO_BIN_EXE_lf"),
            &copied.machine.join("home"),
            &holding,
        );
        copied.check_out("Running conversation");
        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        while !pid.exists() || std::fs::read_to_string(&pid).unwrap().is_empty() {
            assert!(
                std::time::Instant::now() < deadline,
                "the Machine's server did not start the Task conversation"
            );
            std::thread::sleep(Duration::from_millis(100));
        }
        copied.running = std::fs::read_to_string(&pid).unwrap().trim().to_string();
        // Its terminal outlives the server; with no server, the next one waits.
        drop(server);
        assert!(alive(&copied.running));
        copied.check_out("Never started");
        copied
    }

    /// Create a Task on the Machine and place it in a checkout, which
    /// prepares its conversation.
    fn check_out(&self, title: &str) {
        let created: Value = serde_json::from_str(&success(
            self.lf_in(
                &self.machine,
                &["task", "create", "--title", title, "--json"],
            )
            .output()
            .unwrap(),
        ))
        .unwrap();
        let issue = created["task"]["identifier"].as_str().unwrap();
        success(
            self.lf_in(&self.machine, &["checkout", issue, "--json"])
                .output()
                .unwrap(),
        );
    }

    /// `lf-world copy` of the Machine, and the environment it printed.
    fn copy(&self) -> Value {
        serde_json::from_str(&success(
            clean(env!("CARGO_BIN_EXE_lf-world"), self.root.path())
                .env("LF_HOME", self.machine.join("home"))
                .args(["copy", self.copy.to_str().unwrap()])
                .output()
                .unwrap(),
        ))
        .unwrap()
    }

    /// The copy's `lf`, with the recording provider on `PATH`.
    fn lf(&self, args: &[&str]) -> Command {
        self.lf_in(&self.copy, args)
    }

    /// `lf` in `world`, run from the Machine's repository, with the
    /// recording provider on `PATH`.
    fn lf_in(&self, world: &Path, args: &[&str]) -> Command {
        let mut command = clean(env!("CARGO_BIN_EXE_lf"), self.root.path());
        command
            .env("LF_HOME", world.join("home"))
            .env("CLAUDE_CONFIG_DIR", world.join("claude"))
            .env("CODEX_HOME", world.join("codex"))
            .env("PATH", &self.path)
            .current_dir(self.machine.join("repo"))
            .args(args);
        command
    }

    fn serve(&self) -> (loopflow_test_support::MachineServer, Connection) {
        let server = loopflow_test_support::MachineServer::start_with_path(
            env!("CARGO_BIN_EXE_lf"),
            &self.copy.join("home"),
            &self.path,
        );
        let status: Value =
            serde_json::from_str(&success(self.lf(&["server", "status"]).output().unwrap()))
                .unwrap();
        let connection =
            Connection::open_at(Path::new(status["socket"].as_str().unwrap())).unwrap();
        connection
            .set_read_timeout(Some(Duration::from_secs(60)))
            .unwrap();
        (server, connection)
    }
}

#[test]
fn a_copy_serves_the_machines_tasks_and_changes_nothing_outside_itself() {
    let copied = Copied::new();
    let repo = copied.machine.join("repo");
    let source = copied.machine.join("home/loopflow.db");
    let (source_before, checkouts_before) = (digest(&source), checkouts(&repo));

    let env = copied.copy();
    let store = PathBuf::from(env["LF_HOME"].as_str().unwrap()).join("loopflow.db");
    assert!(std::fs::metadata(&store).unwrap().permissions().readonly());
    let copy_before = digest(&store);

    let (_server, mut connection) = copied.serve();
    let plan = connection
        .request(json!({
            "kind": "plan.list", "directory": repo, "wave": null, "task": null, "all": true
        }))
        .unwrap()
        .result;
    let tasks = plan["tasks"]["items"].as_array().unwrap();
    let task = tasks
        .iter()
        .find(|task| task["task"]["name"] == "Search Session transcripts")
        .unwrap_or_else(|| panic!("{plan}"));
    let issue = task["task"]["identifier"].as_str().unwrap();
    let sessions: Value = serde_json::from_str(&success(
        copied
            .lf(&["session", "list", "--json", "--all"])
            .output()
            .unwrap(),
    ))
    .unwrap();
    let session = sessions[0]["id"].as_str().unwrap();

    for change in [
        json!({
            "kind": "task.run", "directory": repo, "issue": issue, "from": null,
            "agent": "claude", "approve": false, "graph": null
        }),
        json!({ "kind": "task.interrupt", "directory": repo, "issue": issue }),
        json!({
            "kind": "session.continue", "session": session,
            "request": "3f2a7c1e-9b4d-4e8a-a1c6-5d0e7f9b2c34", "rows": 40, "cols": 60
        }),
        json!({
            "kind": "session.start", "session": session,
            "request": "0b9c1f0e-6d1c-4a43-9f0e-2f6c8f0f5a11",
            "argv": [env!("CARGO_BIN_EXE_lf"), "session", "resume", session],
            "cwd": repo, "env": [["PATH", copied.path]], "rows": 24, "cols": 80,
            "placement": "desktop-shell"
        }),
        json!({ "kind": "request.upload", "bytes": 200 }),
    ] {
        let refusal = connection.request(&change).unwrap_err().to_string();
        assert!(
            refusal.starts_with("This is a copy of the Machine from ")
                && refusal.ends_with("; make changes in Loopflow or with lf."),
            "{change}: {refusal}"
        );
    }

    // The Machine has a conversation prepared and never started. A server
    // that started its writers here would start no provider either, since
    // the store stops each one first, and would log those failed writes from
    // boot on.
    let log = std::fs::read_to_string(copied.copy.join("home/logs/lf.log")).unwrap();
    assert!(log.contains("ready on a copy of the Machine"), "{log}");
    assert!(!log.contains("readonly database"), "{log}");

    // A pane's `lf` reaches the store before any provider.
    let resumed = copied.lf(&["session", "resume", session]).output().unwrap();
    assert!(!resumed.status.success());
    assert!(!copied.provider_ran.exists());
    assert!(alive(&copied.running));
    assert_eq!(digest(&store), copy_before);
    assert_eq!(digest(&source), source_before);
    assert_eq!(checkouts(&repo), checkouts_before);
}

#[test]
fn a_copy_replaces_the_last_one_and_only_a_world() {
    let copied = Copied::new();
    copied.copy();
    std::fs::write(copied.copy.join("left-behind"), "").unwrap();
    copied.copy();
    assert!(!copied.copy.join("left-behind").exists());

    let other = copied.root.path().join("documents");
    std::fs::create_dir(&other).unwrap();
    std::fs::write(other.join("keep"), "").unwrap();
    let refused = clean(env!("CARGO_BIN_EXE_lf-world"), copied.root.path())
        .env("LF_HOME", copied.machine.join("home"))
        .args(["copy", other.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert!(other.join("keep").exists());

    let absent = clean(env!("CARGO_BIN_EXE_lf-world"), copied.root.path())
        .env("LF_HOME", copied.root.path().join("nowhere"))
        .args(["copy", copied.copy.to_str().unwrap()])
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&absent.stderr).into_owned();
    assert!(said.contains("there is no Machine to copy"), "{said}");
    assert!(said.contains("lf-world build"), "{said}");
}
