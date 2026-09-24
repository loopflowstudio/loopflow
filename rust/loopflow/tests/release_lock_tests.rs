mod support;

use std::fs;
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use loopflow::engine::git::worktree_remove;
use loopflow::ops::{release_publish, release_tag, OpsError};
use loopflow_test_support::TestRepo;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use support::EnvGuard;

#[test]
fn only_an_owned_inherited_lock_suppresses_manual_intervention() {
    for held in [false, true] {
        let _env = EnvGuard::new(&[]);
        let repo = TestRepo::new();
        let home = PathBuf::from(std::env::var_os("LF_HOME").unwrap());
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/dto/release_history.json"
        ))
        .unwrap();
        let mut obligation = fixture["obligations"][0].clone();
        obligation["repo"] = json!(repo.path().canonicalize().unwrap());
        obligation["opportunities"]
            .as_array_mut()
            .unwrap()
            .truncate(1);
        obligation["opportunities"][0]["attempts"][0]["outcome"] =
            json!({"status": "failed", "cause": "interrupted publication"});
        let attempts = obligation["opportunities"][0]["attempts"].clone();
        let records = home.join("cron/obligations");
        fs::create_dir_all(&records).unwrap();
        let record = records.join("fixture.json");
        fs::write(&record, serde_json::to_vec(&obligation).unwrap()).unwrap();

        let locks = repo.path().join(".lf/locks");
        fs::create_dir_all(&locks).unwrap();
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(locks.join(format!(
                "release-{}.lock",
                hex::encode(Sha256::digest("default"))
            )))
            .unwrap();
        if held {
            fs2::FileExt::lock_exclusive(&file).unwrap();
        }
        let fd = file.as_raw_fd();
        let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
        command
            .args(["release", "tag", "0.9.1"])
            .current_dir(repo.path())
            .env("LF_RELEASE_LOCK_FD", fd.to_string());
        // SAFETY: file stays alive until the child exits; fcntl only changes
        // this descriptor's inheritance in the child and is async-signal-safe.
        unsafe {
            command.pre_exec(move || {
                if libc::fcntl(fd, libc::F_SETFD, 0) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let saved: Value = serde_json::from_slice(&fs::read(record).unwrap()).unwrap();
        let opportunity = &saved["opportunities"][0];
        assert_eq!(opportunity["attempts"], attempts);
        assert_eq!(
            opportunity["interventions"].as_array().unwrap().len(),
            usize::from(!held),
            "an unlocked descriptor must not hide manual repair"
        );
    }
}

// Always release the fixture's child, including when an assertion fails.
struct MutationParent {
    child: Child,
    state: PathBuf,
}

impl Drop for MutationParent {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::write(self.state.join("allow"), "");
    }
}

fn wait_for(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !path.exists() {
        assert!(Instant::now() < deadline, "waiting for {}", path.display());
        thread::sleep(Duration::from_millis(10));
    }
}

fn start(repo: &TestRepo, state: &Path, args: &[&str]) -> MutationParent {
    let log = fs::File::create(state.join("controller.log")).unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(args)
        .current_dir(repo.path())
        .env_remove("LF_RELEASE_LOCK_FD")
        .stdout(Stdio::from(log.try_clone().unwrap()))
        .stderr(Stdio::from(log))
        .spawn()
        .unwrap();
    let parent = MutationParent {
        child,
        state: state.to_path_buf(),
    };
    wait_for(&state.join("ready"));
    parent
}

fn blocking_mutation(state: &Path, mutation: &str) -> String {
    format!(
        r#": > '{state}/ready'
while [ ! -f '{state}/allow' ]; do
  [ -d '{state}' ] || exit 1
  sleep 0.02
done
{mutation}
: > '{state}/completed'
exit 0
"#,
        state = state.display(),
    )
}

#[test]
fn surviving_tag_push_excludes_release_after_controller_death() {
    let repo = TestRepo::new();
    let state = tempfile::tempdir().unwrap();
    let real_git = Command::new("which").arg("git").output().unwrap();
    let real_git = String::from_utf8(real_git.stdout).unwrap();
    let mutation = format!(
        "'{}' \"$@\" > '{}/child.log' 2>&1 || exit $?",
        real_git.trim(),
        state.path().display(),
    );
    let git = format!(
        "#!/bin/sh\nif [ \"$*\" = 'push origin v0.9.1' ]; then\n{}fi\nexec '{}' \"$@\"\n",
        blocking_mutation(state.path(), &mutation),
        real_git.trim(),
    );
    let _env = EnvGuard::new(&[("git", &git)]);
    let mut parent = start(&repo, state.path(), &["release", "tag", "0.9.1"]);
    parent.child.kill().unwrap();
    assert!(!parent.child.wait().unwrap().success());

    let contender = release_tag(repo.path(), "0.9.2", None);
    let independent = TestRepo::new();
    let independent_result = release_tag(independent.path(), "0.9.2", None);
    fs::write(state.path().join("allow"), "").unwrap();
    wait_for(&state.path().join("completed"));

    assert!(
        matches!(contender, Err(OpsError::ReleaseDeferred { .. })),
        "{contender:?}"
    );
    assert!(independent_result.is_ok(), "{independent_result:?}");
    let published = Command::new(real_git.trim())
        .arg("--git-dir")
        .arg(repo.bare_path())
        .args(["tag", "--list"])
        .output()
        .unwrap();
    assert!(published.status.success());
    assert_eq!(
        String::from_utf8(published.stdout).unwrap().trim(),
        "v0.9.1"
    );
    wait_until_released(|| release_tag(repo.path(), "0.9.1", None).map(|_| ()));
}

#[test]
fn surviving_publication_excludes_release_after_controller_death() {
    let repo = TestRepo::new();
    let state = tempfile::tempdir().unwrap();
    let published = state.path().join("published");
    let gh = format!(
        "#!/bin/sh\ncase \"$1 $2\" in\n'--version ') exit 0;;\n'release view') echo '{{\"isDraft\":true}}'; exit 0;;\n'release edit')\nif [ \"$3\" = v0.9.1 ]; then\n{}fi\nprintf '%s\\n' \"$3\" >> '{}'; exit 0;;\nesac\nexit 1\n",
        blocking_mutation(state.path(), &format!("echo v0.9.1 >> '{}'", published.display())),
        published.display(),
    );
    let _env = EnvGuard::new(&[("gh", &gh)]);
    let mut parent = start(
        &repo,
        state.path(),
        &["release", "publish", "v0.9.1", "--finalize"],
    );
    parent.child.kill().unwrap();
    assert!(!parent.child.wait().unwrap().success());

    let contender = release_publish(repo.path(), "v0.9.2", None, &[], true);
    fs::write(state.path().join("allow"), "").unwrap();
    wait_for(&state.path().join("completed"));

    assert!(
        matches!(contender, Err(OpsError::ReleaseDeferred { .. })),
        "{contender:?}"
    );
    assert_eq!(fs::read_to_string(published).unwrap(), "v0.9.1\n");
    wait_until_released(|| release_tag(repo.path(), "0.9.1", None).map(|_| ()));
}

fn wait_until_released(mut operation: impl FnMut() -> Result<(), OpsError>) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match operation() {
            Ok(()) => return,
            Err(OpsError::ReleaseDeferred { .. }) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(10));
            }
            result => panic!("child exited but target remained unavailable: {result:?}"),
        }
    }
}

#[test]
fn surviving_publisher_keeps_its_checkout_after_controller_death() {
    for stage in ["prepare", "publish"] {
        let state = tempfile::tempdir().unwrap();
        let gh = "#!/bin/sh\ncase \"$1 $2\" in\n'--version ') exit 0;;\n'run list') printf '[{\"databaseId\":42,\"headBranch\":\"v0.9.1\",\"headSha\":\"%s\",\"status\":\"completed\",\"conclusion\":\"success\"}]' \"$(git rev-parse v0.9.1)\";;\n'run download') exit 0;;\n'release view') exit 1;;\n*) exit 91;;\nesac\n";
        let _env = EnvGuard::new(&[("gh", gh)]);
        let repo = TestRepo::new();
        fs::create_dir_all(repo.path().join(".lf")).unwrap();
        fs::write(
            repo.path().join(".lf/config.yaml"),
            "release:\n  targets:\n    default:\n      workflow: release.yml\n      publisher: [sh, '{repo}/publisher.sh']\n",
        ).unwrap();
        fs::write(
            repo.path().join("publisher.sh"),
            format!(
                r#"#!/bin/sh
if [ "$1" = '{stage}' ]; then
  pwd > '{state}/checkout'
  {barrier}
fi
case "$1" in
  check) exit 0;;
  prepare)
    while [ "$#" -gt 0 ]; do
      if [ "$1" = --output ]; then
        mkdir -p "$2"
        echo '{{}}' > "$2/candidate.json"
        exit 0
      fi
      shift
    done;;
  publish) exit 0;;
esac
exit 92
"#,
                state = state.path().display(),
                barrier = blocking_mutation(
                    state.path(),
                    &format!(
                        "cat publisher.sh > '{}/retained-source'",
                        state.path().display()
                    )
                ),
            ),
        )
        .unwrap();
        for args in [
            vec!["add", ".lf/config.yaml", "publisher.sh"],
            vec!["commit", "-m", "Configure publisher fixture"],
            vec!["tag", "v0.9.1"],
            vec!["push", "origin", "HEAD", "v0.9.1"],
        ] {
            let output = Command::new("git")
                .args(args)
                .current_dir(repo.path())
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let independent = state.path().join("independent-checkout");
        let output = Command::new("git")
            .args(["worktree", "add", "--detach"])
            .arg(&independent)
            .arg("HEAD")
            .current_dir(repo.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut parent = start(&repo, state.path(), &["release", "run", "0.9.1"]);
        let checkout = PathBuf::from(
            fs::read_to_string(state.path().join("checkout"))
                .unwrap()
                .trim(),
        );
        let bytes = fs::read(checkout.join("publisher.sh")).unwrap();
        parent.child.kill().unwrap();
        assert!(!parent.child.wait().unwrap().success());
        let removal = worktree_remove(repo.path(), &checkout);
        worktree_remove(repo.path(), &independent).unwrap();
        let contender = release_tag(repo.path(), "0.9.2", None);
        fs::write(state.path().join("allow"), "").unwrap();
        wait_for(&state.path().join("completed"));
        assert!(
            removal.is_err(),
            "removed surviving {stage} child's checkout"
        );
        assert!(
            matches!(contender, Err(OpsError::ReleaseDeferred { .. })),
            "{contender:?}"
        );
        assert_eq!(
            fs::read(state.path().join("retained-source")).unwrap(),
            bytes
        );
        wait_until_released(|| release_tag(repo.path(), "0.9.1", None).map(|_| ()));
        worktree_remove(repo.path(), &checkout).unwrap();
        assert!(!checkout.exists());
    }
}
