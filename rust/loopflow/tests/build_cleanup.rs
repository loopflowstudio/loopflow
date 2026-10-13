use std::fs::{self, File, FileTimes};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, SystemTime};

use loopflow::git::{worktree_add, WorktreeBranch};
use loopflow_test_support::{lf_binary, TestRepo};
use rusqlite::{params, Connection};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

use crate::support::register_task_without_pr;

struct Fixture {
    repo: TestRepo,
    directory: TempDir,
    home: TempDir,
    checkout: PathBuf,
    task: String,
}

impl Fixture {
    fn new() -> Self {
        let repo = TestRepo::new();
        fs::write(
            repo.path().join(".gitignore"),
            "target/\n.build/\nswift/.build/\nwebsite/node_modules/\n",
        )
        .unwrap();
        let status = Command::new("git")
            .current_dir(repo.path())
            .args(["add", ".gitignore"])
            .status()
            .unwrap();
        assert!(status.success());
        assert!(Command::new("git")
            .current_dir(repo.path())
            .args(["commit", "-m", "ignore generated output"])
            .output()
            .unwrap()
            .status
            .success());
        let directory = tempfile::tempdir().unwrap();
        let checkout = directory.path().canonicalize().unwrap().join("completed");
        worktree_add(
            repo.path(),
            &checkout,
            "finished",
            WorktreeBranch::New {
                start_point: "HEAD",
            },
        )
        .unwrap();
        let home = tempfile::tempdir().unwrap();
        let registered = register_task_without_pr(home.path(), &checkout, "finished", "HEAD");
        let task = registered.task.id.to_string();
        let fixture = Self {
            repo,
            directory,
            home,
            checkout,
            task,
        };
        fixture.state(true);
        fixture.output();
        fs::write(
            fixture.checkout.join("untracked-notes"),
            "retained source edits",
        )
        .unwrap();
        fs::write(
            fixture.checkout.join(".gitignore"),
            "target/\n.build/\nswift/.build/\nwebsite/node_modules/\n# dirty source\n",
        )
        .unwrap();
        fs::create_dir_all(fixture.home.path().join("runs/retained")).unwrap();
        fs::write(
            fixture.home.path().join("runs/retained/events.jsonl"),
            "retained native history",
        )
        .unwrap();
        fixture
    }

    fn database(&self) -> PathBuf {
        self.home.path().join("loopflow.db")
    }

    fn state(&self, done: bool) {
        Connection::open(self.database())
            .unwrap()
            .execute(
                "UPDATE tasks SET planning_completed=?2 WHERE id=?1",
                params![self.task, done],
            )
            .unwrap();
    }

    fn output(&self) {
        fs::create_dir_all(self.checkout.join("target/debug")).unwrap();
        fs::write(
            self.checkout.join("target/debug/artifact"),
            vec![b'x'; 128 * 1024],
        )
        .unwrap();
        age(&self.checkout.join("target"));
    }

    fn command(&self) -> Command {
        let mut command = Command::new(lf_binary());
        command
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap())
            .env("HOME", self.home.path())
            .env("LF_HOME", self.home.path())
            .env("LF_BIN", lf_binary())
            .stdin(Stdio::null())
            .current_dir(self.repo.path());
        command
    }

    fn prune(&self, extra: &[&str]) -> Value {
        let output = self
            .command()
            .args(["wt", "prune", "--builds", "--json"])
            .args(extra)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }

    fn entry(&self, report: &Value) -> Value {
        report["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["path"] == self.checkout.join("target").to_string_lossy().as_ref())
            .unwrap()
            .clone()
    }

    fn preserved(&self) {
        assert_eq!(
            fs::read_to_string(self.checkout.join("untracked-notes")).unwrap(),
            "retained source edits"
        );
        assert!(fs::read_to_string(self.checkout.join(".gitignore"))
            .unwrap()
            .ends_with("# dirty source\n"));
        assert_eq!(
            fs::read_to_string(self.home.path().join("runs/retained/events.jsonl")).unwrap(),
            "retained native history"
        );
        let conn = Connection::open(self.database()).unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT planning_completed FROM tasks WHERE id=?1",
                [&self.task],
                |r| r.get::<_, i32>(0)
            )
            .unwrap(),
            1
        );
        assert!(self.checkout.join(".git").exists());
        assert!(self.directory.path().exists());
    }
}

fn age(path: &Path) {
    if path.is_dir() {
        for child in fs::read_dir(path).unwrap() {
            age(&child.unwrap().path());
        }
    }
    let old = SystemTime::now() - Duration::from_secs(48 * 3600);
    File::open(path)
        .unwrap()
        .set_times(FileTimes::new().set_modified(old))
        .unwrap();
}

struct Running(Child);
impl Drop for Running {
    fn drop(&mut self) {
        // SAFETY: every Running child is launched into a fresh process group
        // whose id is its pid. This owns its compiler-probe descendants too.
        unsafe {
            libc::kill(-(self.0.id() as libc::pid_t), libc::SIGKILL);
        }
        let _ = self.0.wait();
    }
}

#[test]
fn build_cleanup_preview_apply_preserve_dirty_completed_checkout_and_history() {
    let fixture = Fixture::new();
    let preview = fixture.prune(&["--dry-run"]);
    assert_eq!(fixture.entry(&preview)["status"], "candidate");
    assert_eq!(preview["removed_bytes"], 0);
    assert!(fixture.checkout.join("target/debug/artifact").exists());
    let applied = fixture.prune(&[]);
    assert_eq!(fixture.entry(&applied)["status"], "removed");
    assert!(applied["removed_bytes"].as_u64().unwrap() >= 128 * 1024);
    assert!(!fixture.checkout.join("target").exists());
    fixture.preserved();
    assert_eq!(fixture.prune(&[])["removed_bytes"], 0);
}

#[test]
fn build_cleanup_requires_completion_grace_and_budget() {
    let fixture = Fixture::new();
    fixture.state(false);
    assert!(fixture.entry(&fixture.prune(&[]))["reason"]
        .as_str()
        .unwrap()
        .contains("unfinished"));
    fixture.state(true);
    fs::write(
        fixture.checkout.join("target/debug/artifact"),
        "recent edit in old directory",
    )
    .unwrap();
    assert!(fixture.entry(&fixture.prune(&[]))["reason"]
        .as_str()
        .unwrap()
        .contains("grace"));
    age(&fixture.checkout.join("target"));
    for limit in [["--maximum-roots", "0"], ["--maximum-bytes", "1"]] {
        assert!(fixture.entry(&fixture.prune(&limit))["reason"]
            .as_str()
            .unwrap()
            .contains("budget"));
        assert!(fixture.checkout.join("target").exists());
    }
    Connection::open(fixture.database())
        .unwrap()
        .execute(
            "UPDATE tasks SET abandoned_at=1 WHERE id=?1",
            [&fixture.task],
        )
        .unwrap();
    assert!(fixture.entry(&fixture.prune(&[]))["reason"]
        .as_str()
        .unwrap()
        .contains("no positive"));
}

#[test]
fn build_cleanup_retains_running_binary_from_elsewhere_and_open_output() {
    let fixture = Fixture::new();
    let binary = fixture.checkout.join("target/debug/sleep");
    let source = fixture.checkout.join("sleeper.rs");
    fs::write(
        &source,
        "fn main() { std::thread::sleep(std::time::Duration::from_secs(30)); }",
    )
    .unwrap();
    let compiled = Command::new("rustc")
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    age(&fixture.checkout.join("target"));
    let mut child = Running(
        Command::new(&binary)
            .process_group(0)
            .arg("30")
            .current_dir(fixture.repo.path())
            .stdin(Stdio::null())
            .spawn()
            .unwrap(),
    );
    assert!(child.0.try_wait().unwrap().is_none());
    let retained = fixture.entry(&fixture.prune(&[]));
    assert!(
        retained["reason"].as_str().unwrap().contains("process"),
        "{retained}"
    );
    assert!(binary.exists());
    drop(child);
    let handle = File::open(fixture.checkout.join("target/debug/artifact")).unwrap();
    assert!(fixture.entry(&fixture.prune(&[]))["reason"]
        .as_str()
        .unwrap()
        .contains("open file"));
    drop(handle);
    assert_eq!(fixture.entry(&fixture.prune(&[]))["status"], "removed");
    fixture.preserved();
}

#[test]
fn build_cleanup_retains_unknown_inspection_and_cooperating_lease() {
    let fixture = Fixture::new();
    let bin = fixture.home.path().join("bin");
    fs::create_dir(&bin).unwrap();
    fs::write(bin.join("lsof"), "#!/bin/sh\nexit 1\n").unwrap();
    fs::set_permissions(bin.join("lsof"), fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    let result = fixture
        .command()
        .env("PATH", path)
        .args(["wt", "prune", "--builds", "--json"])
        .output()
        .unwrap();
    assert!(result.status.success());
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert!(fixture.entry(&report)["reason"]
        .as_str()
        .unwrap()
        .contains("inspection failed"));
    // Existing lease protocol; an ordinary cooperating writer excludes deletion.
    fixture.prune(&["--dry-run"]);
    let lock_dir = fixture.repo.path().join(".git/loopflow/worktree-locks");
    let digest = hex::encode(Sha256::digest(
        fixture.checkout.as_os_str().as_encoded_bytes(),
    ));
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(lock_dir.join(format!("{digest}.lock")))
        .unwrap();
    fs2::FileExt::lock_exclusive(&lock).unwrap();
    assert!(fixture.entry(&fixture.prune(&[]))["reason"]
        .as_str()
        .unwrap()
        .contains("owned"));
    assert!(fixture.checkout.join("target").exists());
    drop(lock);
    assert_eq!(fixture.entry(&fixture.prune(&[]))["status"], "removed");
}

#[test]
fn build_cleanup_retains_tracked_output_symlink_escape_and_unregistered_ownership() {
    let fixture = Fixture::new();
    assert!(Command::new("git")
        .current_dir(&fixture.checkout)
        .args(["add", "-f", "target/debug/artifact"])
        .output()
        .unwrap()
        .status
        .success());
    assert!(fixture.entry(&fixture.prune(&[]))["reason"]
        .as_str()
        .unwrap()
        .contains("tracked"));
    assert!(Command::new("git")
        .current_dir(&fixture.checkout)
        .args(["reset", "HEAD", "--", "target"])
        .output()
        .unwrap()
        .status
        .success());
    let outside = fixture.home.path().join("source");
    fs::write(&outside, "retained external source").unwrap();
    std::os::unix::fs::symlink(&outside, fixture.checkout.join("target/debug/escape")).unwrap();
    assert!(fixture.entry(&fixture.prune(&[]))["status"] == "retained");
    assert_eq!(
        fs::read_to_string(&outside).unwrap(),
        "retained external source"
    );
    fs::remove_file(fixture.checkout.join("target/debug/escape")).unwrap();
    age(&fixture.checkout.join("target"));
    Connection::open(fixture.database())
        .unwrap()
        .execute(
            "UPDATE tasks SET worktree=NULL WHERE id=?1",
            [&fixture.task],
        )
        .unwrap();
    assert!(fixture.entry(&fixture.prune(&[]))["reason"]
        .as_str()
        .unwrap()
        .contains("no positive"));
}

#[test]
fn build_cleanup_normal_python_recovery_uses_same_owner_without_pressure() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.checkout.join("src")).unwrap();
    fs::write(
        fixture.checkout.join("src/main.rs"),
        "fn main() { println!(\"retained source\"); }\n",
    )
    .unwrap();
    fs::write(
        fixture.checkout.join("Cargo.toml"),
        "[package]\nname = \"cleanup-demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    let build_started = std::time::Instant::now();
    let build = Command::new("cargo")
        .args(["build", "--offline", "--jobs", "4"])
        .env("CARGO_TARGET_DIR", fixture.checkout.join("target"))
        .current_dir(&fixture.checkout)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let build_seconds = build_started.elapsed().as_secs_f64();
    let lockfile = fs::read(fixture.checkout.join("Cargo.lock")).unwrap();
    age(&fixture.checkout.join("target"));
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/resource_envelope.py")
        .canonicalize()
        .unwrap();
    let code = "import importlib.util, sys, json, shutil; from pathlib import Path; s=importlib.util.spec_from_file_location('resources',sys.argv[1]); m=importlib.util.module_from_spec(s); sys.modules[s.name]=m; s.loader.exec_module(m); p=m.load_policy(); target=Path(sys.argv[3]); before=m._allocated_bytes(target); free=shutil.disk_usage(target).free; r=m.inspect_resources(Path(sys.argv[2]),p,True); print(json.dumps({'before_bytes':before,'after_bytes':m._allocated_bytes(target),'removed_bytes':sum(a.removed_bytes for a in r.recovery),'free_delta_bytes':shutil.disk_usage(target.parent).free-free,'entries':[m.asdict(a) for a in r.recovery]}))";
    let mut command = Command::new("uv");
    command
        .args(["run", "--no-sync", "python", "-c", code])
        .arg(script)
        .arg(fixture.repo.path())
        .arg(fixture.checkout.join("target"))
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .env("LF_HOME", fixture.home.path())
        .env("HOME", fixture.home.path())
        .env("LF_BIN", lf_binary())
        .env("UV_CACHE_DIR", fixture.home.path().join("uv-cache"));
    for (key, _) in std::env::vars()
        .filter(|(key, _)| key.starts_with("LF_") && key != "LF_HOME" && key != "LF_BIN")
    {
        command.env_remove(key);
    }
    let cleanup_started = std::time::Instant::now();
    let result = command.stdin(Stdio::null()).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let mut measured: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert!(measured["before_bytes"].as_u64().unwrap() >= 128 * 1024);
    assert_eq!(measured["after_bytes"], 0);
    assert!(
        measured["removed_bytes"].as_u64().unwrap()
            >= measured["before_bytes"].as_u64().unwrap() * 9 / 10
    );
    measured["compile_seconds"] = build_seconds.into();
    measured["recovery_seconds"] = cleanup_started.elapsed().as_secs_f64().into();
    println!("disposable build reclamation: {measured}");
    assert!(!fixture.checkout.join("target").exists());
    assert_eq!(
        fs::read(fixture.checkout.join("Cargo.lock")).unwrap(),
        lockfile
    );
    assert!(fixture.checkout.join("src/main.rs").exists());
    fixture.preserved();
}

#[test]
fn build_cleanup_retains_unfinished_association_and_unknown_execution() {
    let fixture = Fixture::new();
    let conn = Connection::open(fixture.database()).unwrap();
    let pending = loopflow::durable::TaskId::new();
    conn.execute("INSERT INTO tasks(id,project_id,issue_identifier,worktree,created_at,checkout_machine_id,planning_completed) SELECT ?1,project_id,'PROOF-2',worktree || '/.',1,checkout_machine_id,0 FROM tasks WHERE id=?2",params![pending.as_str(),fixture.task]).unwrap();
    let retained = fixture.entry(&fixture.prune(&[]));
    assert!(
        retained["reason"].as_str().unwrap().contains("unfinished"),
        "{retained}"
    );
    conn.execute("DELETE FROM tasks WHERE id=?1", [pending.as_str()])
        .unwrap();
    let process = loopflow::id::LfProcessId::new();
    conn.execute(
        "INSERT INTO processes(id,trace_id,cwd,started_at) VALUES(?1,?2,?3,?4)",
        params![
            process,
            loopflow::id::TraceId::new(),
            fixture.checkout.to_str().unwrap(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64
        ],
    )
    .unwrap();
    let retained = fixture.entry(&fixture.prune(&[]));
    assert!(
        retained["reason"].as_str().unwrap().contains("unresolved"),
        "{retained}"
    );
    assert!(fixture.checkout.join("target").exists());
    conn.execute(
        "UPDATE processes SET completed_at=2,outcome='succeeded' WHERE id=?1",
        [process],
    )
    .unwrap();
    assert_eq!(fixture.entry(&fixture.prune(&[]))["status"], "removed");
}

#[test]
fn build_cleanup_retains_primary_persistent_and_caps_actual_removal() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.repo.path().join("target")).unwrap();
    fs::write(fixture.repo.path().join("target/primary"), "retained").unwrap();
    assert!(Command::new("git")
        .current_dir(fixture.repo.path())
        .args(["config", "branch.finished.loopflow-persistent", "true"])
        .output()
        .unwrap()
        .status
        .success());
    let retained = fixture.entry(&fixture.prune(&[]));
    assert!(
        retained["reason"].as_str().unwrap().contains("persistent"),
        "{retained}"
    );
    assert!(Command::new("git")
        .current_dir(fixture.repo.path())
        .args(["config", "--unset", "branch.finished.loopflow-persistent"])
        .output()
        .unwrap()
        .status
        .success());
    fs::create_dir(fixture.checkout.join(".build")).unwrap();
    fs::write(fixture.checkout.join(".build/old-output"), "old").unwrap();
    age(&fixture.checkout.join(".build"));
    let limited = fixture.prune(&["--maximum-roots", "1"]);
    assert_eq!(
        limited["entries"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["status"] == "removed")
            .count(),
        1
    );
    assert!(!fixture.checkout.join("target").exists());
    assert!(fixture.checkout.join(".build/old-output").exists());
    assert!(fixture.repo.path().join("target/primary").exists());
    fixture.prune(&[]);
    assert!(!fixture.checkout.join(".build").exists());
    fixture.preserved();
}

#[test]
fn build_cleanup_detected_direct_cargo_questions_completed_candidate() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.checkout.join("src")).unwrap();
    fs::write(fixture.checkout.join("src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(
        fixture.checkout.join("Cargo.toml"),
        "[package]\nname = \"busy-candidate\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    // Keep actual Cargo alive at its compiler probe, without using Cargo's
    // replaceable locks as cleanup exclusion. The shim lives outside output.
    let compiler = fixture.home.path().join("compiler-probe");
    fs::write(&compiler, "#!/bin/sh\nexec /bin/sleep 30\n").unwrap();
    fs::set_permissions(&compiler, fs::Permissions::from_mode(0o755)).unwrap();
    let mut cargo = Running(
        Command::new("cargo")
            .process_group(0)
            .args(["check", "--offline"])
            .env("RUSTC", compiler)
            .env("CARGO_TARGET_DIR", fixture.checkout.join("target"))
            .current_dir(&fixture.checkout)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    assert!(cargo.0.try_wait().unwrap().is_none());
    let retained = fixture.entry(&fixture.prune(&[]));
    assert!(
        retained["reason"]
            .as_str()
            .unwrap()
            .contains("verify Task selection"),
        "{retained}"
    );
    assert!(cargo.0.try_wait().unwrap().is_none());
    assert!(fixture.checkout.join("target/debug/artifact").exists());
}

#[test]
fn build_cleanup_retains_durable_state_inside_ignored_output() {
    let fixture = Fixture::new();
    let database = fixture.checkout.join("target/loopflow.db");
    fs::write(&database, "retained store").unwrap();
    age(&fixture.checkout.join("target"));
    let retained = fixture.entry(&fixture.prune(&[]));
    assert!(
        retained["reason"]
            .as_str()
            .unwrap()
            .contains("retained Loopflow state"),
        "{retained}"
    );
    assert_eq!(fs::read_to_string(&database).unwrap(), "retained store");
    fixture.preserved();
}
