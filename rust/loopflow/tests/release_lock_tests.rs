mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use loopflow::ops::{release_publish, release_tag, OpsError};
use loopflow_test_support::TestRepo;
use support::EnvGuard;

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
