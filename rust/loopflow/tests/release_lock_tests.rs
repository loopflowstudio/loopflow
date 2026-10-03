mod support;

use std::fs;
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use loopflow::engine::git::{current_branch, worktree_remove};
use loopflow::ops::{
    commit_workflow, release_publish, release_tag, CommitOptions, NullProgress, OpsError,
};
use loopflow::work::task::{
    AfterMerge, GithubPr, PrMergeMode, PrMergeRequest, PrPresentation, PrPublication,
};
use loopflow_test_support::TestRepo;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use support::{codex_app_server_script, EnvGuard};

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
fn surviving_source_inspector_excludes_replacement_after_controller_death() {
    let state = tempfile::tempdir().unwrap();
    let _env = EnvGuard::new(&[("gh", "#!/bin/sh\ncase \"$1 $2\" in '--version ') exit 0;; 'release view') exit 1;; esac\nexit 91\n")]);
    let repo = TestRepo::new();
    repo.create_file(
        ".lf/config.yaml",
        "release:\n  targets:\n    default:\n      publisher: [sh, '{repo}/publisher.sh']\n",
    );
    repo.create_file(
        "publisher.sh",
        &format!(
            "#!/bin/sh\n[ \"$1\" != check ] || exit 0\n[ \"$1\" = inspect ] || exit 91\n{}",
            blocking_mutation(state.path(), ":")
        ),
    );
    repo.stage_all();
    repo.commit("Inspector fixture");
    repo.push();
    release_tag(repo.path(), "0.9.1", None).unwrap();
    repo.create_file("README.md", "caller bytes\n");
    let index = fs::read(repo.path().join(".git/index")).unwrap();
    let head = repo.head_sha();
    let mut parent = start(&repo, state.path(), &["release", "run", "patch"]);
    parent.child.kill().unwrap();
    parent.child.wait().unwrap();
    assert!(matches!(
        release_tag(repo.path(), "0.9.2", None),
        Err(OpsError::ReleaseDeferred { .. })
    ));
    assert_eq!(repo.head_sha(), head);
    assert_eq!(fs::read(repo.path().join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read_to_string(repo.path().join("README.md")).unwrap(),
        "caller bytes\n"
    );
    fs::write(state.path().join("allow"), "").unwrap();
    wait_for(&state.path().join("completed"));
    wait_until_released(|| release_tag(repo.path(), "0.9.1", None).map(|_| ()));
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
fn surviving_publisher_keeps_its_checkout_after_controller_exit() {
    for (stage, kill_controller) in [
        ("prepare", true),
        ("publish", true),
        ("prepare", false),
        ("publish", false),
    ] {
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
  {child_start}
  pwd > '{state}/checkout'
  {barrier}
  {child_end}
fi
case "$1" in
  check) exit 0;;
  inspect) echo '{{"preparation_required":[],"publications":null}}';;
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
                child_start = if kill_controller { "" } else { "(" },
                child_end = if kill_controller {
                    String::new()
                } else {
                    format!(
                        ") > '{}/descendant.log' 2>&1 &\n  exit 1",
                        state.path().display()
                    )
                },
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
        let bytes = fs::read(repo.path().join("publisher.sh")).unwrap();
        if kill_controller {
            parent.child.kill().unwrap();
        } else {
            let deadline = Instant::now() + Duration::from_secs(20);
            while parent.child.try_wait().unwrap().is_none() {
                assert!(
                    Instant::now() < deadline,
                    "controller did not finish cleanup"
                );
                thread::sleep(Duration::from_millis(10));
            }
        }
        assert!(!parent.child.wait().unwrap().success());
        assert!(
            checkout.exists(),
            "controller cleanup removed surviving {stage} descendant's checkout"
        );
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

#[test]
fn surviving_release_auto_merge_child_excludes_another_release() {
    for (phase, kill_controller, preparing) in [
        ("enable", true, false),
        ("disable", true, false),
        ("enable", false, false),
        ("disable", false, false),
        ("enable", true, true),
        ("disable", true, true),
        ("enable", false, true),
        ("disable", false, true),
    ] {
        let state = tempfile::tempdir().unwrap();
        fs::write(state.path().join("armed-head"), "prior remote arm").unwrap();
        let mutation = blocking_mutation(
            state.path(),
            &format!(
                r#"[ "$3" = 1176 ] || exit 95
case " $* " in
  *' --disable-auto '*) rm '{state}/armed-head';;
  *)
    while [ "$#" -gt 0 ]; do
      if [ "$1" = --match-head-commit ]; then
        printf '%s' "$2" > '{state}/armed-head'
        break
      fi
      shift
    done;;
esac"#,
                state = state.path().display()
            ),
        );
        let blocked = if kill_controller {
            mutation
        } else {
            format!(
                "(\n{mutation}) > '{}/descendant.log' 2>&1 &\nexit 1\n",
                state.path().display()
            )
        };
        let gh = format!(
            r#"#!/bin/sh
case "$1 $2" in
  '--version ') exit 0;;
  'run list') echo '[]'; exit 0;;
  'release view') exit 1;;
  'pr create') : > '{state}/created'; echo 'https://example.com/pr/1176'; exit 0;;
  'pr edit'|'pr ready') exit 0;;
  'pr list')
    if [ '{preparing}' = true ] && [ ! -f '{state}/created' ]; then echo '[]'; exit 0; fi
    case " $* " in
      *' --head '*)
        head="$(git rev-parse HEAD)"
        printf '[{{"number":1176,"state":"OPEN","mergeCommit":null,"url":"https://example.com/pr/1176","headRefOid":"%s"}}]\n' "$head";;
      *) echo '[]';;
    esac
    exit 0;;
  'pr view')
    echo '{{"state":"OPEN","mergeStateStatus":"CLEAN","mergeCommit":null}}'
    exit 0;;
  'api graphql')
    case "$*" in *LoopflowPrMerge*) echo '{{"data":{{"repository":{{"pullRequest":{{"number":1176,"url":"https://example.com/pr/1176","state":"OPEN","isDraft":false,"headRefName":"release","headRefOid":"observed","mergedAt":null,"mergeCommit":null,"mergeStateStatus":"CLEAN","isMergeQueueEnabled":false,"autoMergeRequest":null,"mergeQueueEntry":null}}}}}}}}'; exit 0;; esac
    if [ '{phase}' = disable ] && [ -f '{state}/queried' ]; then
      echo true
    else
      echo false
    fi
    : > '{state}/queried'
    exit 0;;
  'pr merge')
    case " $* " in
      *' --disable-auto '*) [ '{phase}' = disable ] || exit 92;;
      *) [ '{phase}' = enable ] || exit 93;;
    esac
    pwd > '{state}/checkout'
    {blocked}
    ;;
esac
exit 94
"#,
            state = state.path().display(),
        );
        let notes = "#!/bin/sh\ncat > RELEASE_NOTES.md <<'EOF'\n# v0.9.1\n\n<!-- loopflow:release-notes=narrative;gate=safe -->\n\nFixture release.\nEOF\n";
        // Replacement needs a remote PR before finalization now that release
        // commit/push no longer creates a draft as an intermediate side effect.
        let notes = format!(
            "{notes}\nif [ '{phase}' = disable ]; then : > '{}/created'; fi\n",
            state.path().display()
        );
        let _env = EnvGuard::new(&[("gh", &gh), ("lf", &notes)]);
        let repo = TestRepo::new();
        let mut parent = start(&repo, state.path(), &["release", "run", "0.9.1"]);
        let checkout = PathBuf::from(
            fs::read_to_string(state.path().join("checkout"))
                .unwrap()
                .trim(),
        );
        let head = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&checkout)
            .output()
            .unwrap();
        assert!(head.status.success());
        let head = String::from_utf8(head.stdout).unwrap();
        if kill_controller {
            parent.child.kill().unwrap();
        } else {
            let deadline = Instant::now() + Duration::from_secs(20);
            while parent.child.try_wait().unwrap().is_none() {
                assert!(Instant::now() < deadline, "re-arm controller did not exit");
                thread::sleep(Duration::from_millis(10));
            }
        }
        assert!(!parent.child.wait().unwrap().success());
        if preparing {
            assert!(
                checkout.exists(),
                "controller removed auto-merge child's checkout"
            );
            assert!(worktree_remove(repo.path(), &checkout).is_err());
            assert!(fs::read_to_string(checkout.join("RELEASE_NOTES.md"))
                .unwrap()
                .contains("Fixture release."));
        }
        let contender = release_tag(repo.path(), "0.9.2", None);
        fs::write(state.path().join("allow"), "").unwrap();
        wait_for(&state.path().join("completed"));
        assert!(
            matches!(contender, Err(OpsError::ReleaseDeferred { .. })),
            "{phase}: {contender:?}"
        );
        if phase == "enable" {
            assert_eq!(
                fs::read_to_string(state.path().join("armed-head")).unwrap(),
                head.trim()
            );
        } else {
            assert!(!state.path().join("armed-head").exists());
        }
        wait_until_released(|| release_tag(repo.path(), "0.9.2", None).map(|_| ()));
        if preparing {
            worktree_remove(repo.path(), &checkout).unwrap();
        }
    }
}

#[test]
fn surviving_release_pr_mutation_retains_target_and_checkout() {
    for phase in ["create", "retarget", "edit", "ready"] {
        for kill_controller in [true, false] {
            let state = tempfile::tempdir().unwrap();
            let mutation = blocking_mutation(
                state.path(),
                &format!(
                    "printf '%s' \"$value\" > '{}/remote-state'",
                    state.path().display()
                ),
            );
            let blocked = if kill_controller {
                mutation
            } else {
                format!(
                    "(\n{mutation}) > '{}/descendant.log' 2>&1 &\nexit 1\n",
                    state.path().display()
                )
            };
            let gh = format!(
                r#"#!/bin/sh
case "$1 $2" in
  '--version ') exit 0;;
  'run list') echo '[]'; exit 0;;
  'release view') exit 1;;
  'pr list')
    if [ ! -f '{state}/created' ]; then echo '[]'; exit 0; fi
    case " $* " in
      *' --head '*) printf '[{{"number":1176,"state":"OPEN","mergeCommit":null,"url":"https://example.com/pr/1176","headRefOid":"%s"}}]\n' "$(git rev-parse HEAD)";;
      *) echo '[]';;
    esac
    exit 0;;
  'api graphql') echo false; exit 0;;
  'pr create')
    operation=create
    value="$(git rev-parse HEAD)"
    : > '{state}/created';;
  'pr edit')
    operation=retarget
    value=''
    while [ "$#" -gt 0 ]; do
      case "$1" in
        --title) operation=edit; value="$2"; break;;
        --base) value="$2"; break;;
      esac
      shift
    done;;
  'pr ready') operation=ready; value=ready;;
  *) exit 91;;
esac
if [ "$operation" = '{phase}' ]; then
  pwd > '{state}/checkout'
  {blocked}
fi
[ "$operation" != create ] || echo 'https://example.com/pr/1176'
exit 0
"#,
                state = state.path().display()
            );
            let notes = format!("#!/bin/sh\ncat > RELEASE_NOTES.md <<'EOF'\n# v0.9.1\n\n<!-- loopflow:release-notes=narrative;gate=safe -->\n\nFixture release.\nEOF\nif [ '{phase}' = retarget ]; then : > '{}/created'; fi\n", state.path().display());
            let _env = EnvGuard::new(&[("gh", &gh), ("lf", &notes)]);
            let repo = TestRepo::new();
            let mut parent = start(&repo, state.path(), &["release", "run", "0.9.1"]);
            let checkout = PathBuf::from(
                fs::read_to_string(state.path().join("checkout"))
                    .unwrap()
                    .trim(),
            );
            let head = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(&checkout)
                .output()
                .unwrap();
            assert!(head.status.success());
            if kill_controller {
                parent.child.kill().unwrap();
            } else {
                let deadline = Instant::now() + Duration::from_secs(20);
                while parent.child.try_wait().unwrap().is_none() {
                    assert!(Instant::now() < deadline, "{phase} controller did not exit");
                    thread::sleep(Duration::from_millis(10));
                }
            }
            assert!(!parent.child.wait().unwrap().success());
            let retained = fs::read_to_string(checkout.join("RELEASE_NOTES.md"));
            let removal = worktree_remove(repo.path(), &checkout);
            let contender = release_tag(repo.path(), "0.9.2", None);
            fs::write(state.path().join("allow"), "").unwrap();
            wait_for(&state.path().join("completed"));
            assert!(
                matches!(contender, Err(OpsError::ReleaseDeferred { .. })),
                "{phase}: {contender:?}"
            );
            assert!(
                removal.is_err(),
                "removed surviving {phase} child's checkout"
            );
            assert!(retained.unwrap().contains("Fixture release."));
            let remote = fs::read_to_string(state.path().join("remote-state")).unwrap();
            match phase {
                "create" => assert_eq!(remote, String::from_utf8(head.stdout).unwrap().trim()),
                "retarget" => assert_eq!(remote, "main"),
                "edit" => assert_eq!(remote, "release: v0.9.1"),
                "ready" => assert_eq!(remote, "ready"),
                _ => unreachable!(),
            }
            wait_until_released(|| release_tag(repo.path(), "0.9.2", None).map(|_| ()));
            worktree_remove(repo.path(), &checkout).unwrap();
        }
    }
}

#[test]
fn surviving_release_git_mutation_retains_target_and_checkout() {
    let real_git = Command::new("which").arg("git").output().unwrap();
    let real_git = String::from_utf8(real_git.stdout).unwrap();
    let real_git = real_git.trim();
    for phase in ["add", "commit", "upstream", "push", "force"] {
        for kill_controller in [true, false] {
            eprintln!("Git mutation {phase}, killed controller: {kill_controller}");
            let state = tempfile::tempdir().unwrap();
            let mutation = blocking_mutation(
                state.path(),
                &format!(
                    "'{real_git}' \"$@\" > '{}/git.log' 2>&1 || exit $?",
                    state.path().display()
                ),
            );
            let blocked = if kill_controller {
                mutation
            } else {
                format!(
                    "(\n{mutation}) > '{}/descendant.log' 2>&1 &\nexit 1\n",
                    state.path().display()
                )
            };
            // Commit runs real Git and a real pre-commit hook while its caller exits.
            let hooks = state.path().join("hooks");
            fs::create_dir(&hooks).unwrap();
            let barrier = blocking_mutation(state.path(), "")
                .replace(&format!(": > '{}/completed'\n", state.path().display()), "");
            for (name, body) in [
                ("pre-commit", format!("#!/bin/sh\n{barrier}")),
                (
                    "post-commit",
                    format!("#!/bin/sh\n: > '{}/completed'\n", state.path().display()),
                ),
            ] {
                let path = hooks.join(name);
                fs::write(&path, body).unwrap();
                fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            }
            let commit_command = format!(
                "exec '{real_git}' -c core.hooksPath='{}' \"$@\"",
                hooks.display()
            );
            let commit_child = if kill_controller {
                commit_command
            } else {
                format!(
                    "({commit_command}) > '{}/descendant.log' 2>&1 &\nexit 1",
                    state.path().display()
                )
            };
            let git = format!(
                r#"#!/bin/sh
if [ "$1" = -C ] && [ -f '{state}/notes-written' ]; then
  operation=''
  case " $* " in
    *' add -A '*) operation=add;;
    *' commit -m '*) operation=commit;;
    *' push -u '*) operation=upstream;;
    *' push --force-with-lease '*) operation=force;;
    *' push '*) operation=push;;
  esac
  if [ "$operation" = push ] && [ '{phase}' = force ]; then exit 1; fi
  if [ "$operation" = '{phase}' ]; then
    [ ! -f '{state}/launched' ] || exit 1
    : > '{state}/launched'
    '{real_git}' -C "$2" rev-parse --show-toplevel > '{state}/checkout'
    if [ "$operation" = commit ]; then
      {commit_child}
    fi
    {blocked}
  fi
fi
exec '{real_git}' "$@"
"#,
                state = state.path().display()
            );
            let gh = "#!/bin/sh\ncase \"$1 $2\" in\n'--version ') exit 0;;\n'run list'|'pr list') echo '[]';;\n'release view') exit 1;;\n*) exit 91;;\nesac\n";
            let notes = format!(
                r#"#!/bin/sh
cat > RELEASE_NOTES.md <<'EOF'
# v0.9.1

<!-- loopflow:release-notes=narrative;gate=safe -->

Fixture release.
EOF
if [ '{phase}' = push ] || [ '{phase}' = force ]; then
  '{real_git}' push -u origin HEAD || exit $?
fi
: > '{state}/notes-written'
"#,
                state = state.path().display()
            );
            let _env = EnvGuard::new(&[("git", &git), ("gh", gh), ("lf", &notes)]);
            let repo = TestRepo::new();
            let mut parent = start(&repo, state.path(), &["release", "run", "0.9.1"]);
            let checkout = PathBuf::from(
                fs::read_to_string(state.path().join("checkout"))
                    .unwrap()
                    .trim(),
            );
            if kill_controller {
                parent.child.kill().unwrap();
            } else {
                let deadline = Instant::now() + Duration::from_secs(20);
                while parent.child.try_wait().unwrap().is_none() {
                    assert!(Instant::now() < deadline, "{phase} controller did not exit");
                    thread::sleep(Duration::from_millis(10));
                }
            }
            assert!(!parent.child.wait().unwrap().success());
            let retained = fs::read_to_string(checkout.join("RELEASE_NOTES.md"));
            let removal = worktree_remove(repo.path(), &checkout);
            let contender = release_tag(repo.path(), "0.9.2", None);
            fs::write(state.path().join("allow"), "").unwrap();
            assert!(
                matches!(contender, Err(OpsError::ReleaseDeferred { .. })),
                "{phase}: {contender:?}"
            );
            assert!(
                removal.is_err(),
                "removed surviving {phase} child's checkout"
            );
            assert!(retained.unwrap().contains("Fixture release."));
            wait_for(&state.path().join("completed"));
            let git_read = |args: &[&str]| {
                let output = Command::new(real_git)
                    .arg("-C")
                    .arg(&checkout)
                    .args(args)
                    .output()
                    .unwrap();
                assert!(output.status.success(), "{output:?}");
                String::from_utf8(output.stdout).unwrap()
            };
            let subject = if phase == "add" {
                ":RELEASE_NOTES.md"
            } else {
                "HEAD:RELEASE_NOTES.md"
            };
            assert!(git_read(&["show", subject]).contains("Fixture release."));
            if matches!(phase, "upstream" | "push" | "force") {
                let branch = git_read(&["branch", "--show-current"]);
                let remote = Command::new(real_git)
                    .arg("--git-dir")
                    .arg(repo.bare_path())
                    .args(["rev-parse", branch.trim()])
                    .output()
                    .unwrap();
                assert!(remote.status.success());
                assert_eq!(
                    String::from_utf8(remote.stdout).unwrap(),
                    git_read(&["rev-parse", "HEAD"])
                );
            }
            wait_until_released(|| release_tag(repo.path(), "0.9.2", None).map(|_| ()));
            worktree_remove(repo.path(), &checkout).unwrap();
        }
    }
}

#[test]
fn surviving_release_notes_provider_retains_target_checkout_and_context() {
    for kill_controller in [true, false] {
        eprintln!("Notes provider, killed controller: {kill_controller}");
        let state = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let provider = codex_app_server_script(
            "Notes written",
            "case \"$1\" in --version) echo 'codex fixture'; exit 0;; esac",
        );
        let barrier = format!(
            "pwd > '{state}/checkout'\n: > '{state}/ready'\nwhile [ ! -f '{state}/allow' ]; do [ -d '{state}' ] || exit 1; sleep 0.02; done\ncp \"$LF_RELEASE_NOTES_CONTEXT\" '{state}/context.json' || exit 93\nprintf '# v0.9.1\\n\\nNotes from the surviving provider.\\n' > RELEASE_NOTES.md\n: > '{state}/completed'\n",
            state = state.path().display()
        );
        let provider = provider.replace(
            "read -r turn_start\n",
            &format!(
                "read -r turn_start\nprintf '%s' \"$turn_start\" > '{}/turn.json'\n{barrier}",
                state.path().display()
            ),
        );
        let launch = format!("'{}' \"$@\"", env!("CARGO_BIN_EXE_lf"));
        let lf = if kill_controller {
            format!(
                "#!/bin/sh\necho $$ > '{}/notes.pid'\nexec {launch}\n",
                state.path().display()
            )
        } else {
            format!("#!/bin/sh\n{launch} > '{state}/notes.log' 2>&1 &\necho $! > '{state}/notes.pid'\nwhile [ ! -f '{state}/ready' ]; do [ -d '{state}' ] || exit 1; sleep 0.02; done\nexit 1\n", state=state.path().display())
        };
        let gh = "#!/bin/sh\ncase \"$1 $2\" in\n'--version ') exit 0;;\n'run list'|'pr list') echo '[]';;\n'release view') exit 1;;\n*) exit 91;;\nesac\n";
        let _env = EnvGuard::with_home(
            &[("lf", &lf), ("codex", &provider), ("gh", gh)],
            Some(home.path()),
        );
        let repo = TestRepo::new();
        fs::create_dir_all(repo.path().join(".lf")).unwrap();
        fs::write(repo.path().join(".lf/config.yaml"), "agent: codex\n").unwrap();
        fs::write(
            repo.path().join("RELEASE_NOTES.md"),
            "# v0.9.0\n\nPrevious notes.\n",
        )
        .unwrap();
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .args(args)
                .current_dir(repo.path())
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
            output.stdout
        };
        git(&["add", "."]);
        git(&["commit", "-m", "Configure notes provider"]);
        git(&["push", "origin", "HEAD"]);
        fs::write(repo.path().join("local.txt"), "unpublished commit\n").unwrap();
        git(&["add", "local.txt"]);
        git(&["commit", "-m", "Keep caller work local"]);
        fs::write(repo.path().join("local.txt"), "staged caller work\n").unwrap();
        git(&["add", "local.txt"]);
        fs::write(repo.path().join("local.txt"), "unstaged caller work\n").unwrap();
        fs::write(repo.path().join("untracked.txt"), "untracked caller work\n").unwrap();
        let head = git(&["rev-parse", "HEAD"]);
        let branch = git(&["branch", "--show-current"]);
        let index = fs::read(repo.path().join(".git/index")).unwrap();
        let mut parent = start(&repo, state.path(), &["release", "run", "0.9.1"]);
        let checkout = PathBuf::from(
            fs::read_to_string(state.path().join("checkout"))
                .unwrap()
                .trim(),
        );
        // The provider received a real turn through the built CLI and Codex harness.
        let turn: Value =
            serde_json::from_slice(&fs::read(state.path().join("turn.json")).unwrap()).unwrap();
        assert_eq!(turn["method"], "turn/start");
        if kill_controller {
            parent.child.kill().unwrap();
            let pid: i32 = fs::read_to_string(state.path().join("notes.pid"))
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            // SAFETY: this PID names the fixture's nested CLI, held alive at the provider barrier.
            assert_eq!(unsafe { libc::kill(pid, libc::SIGKILL) }, 0);
        } else {
            let deadline = Instant::now() + Duration::from_secs(20);
            while parent.child.try_wait().unwrap().is_none() {
                assert!(Instant::now() < deadline, "notes controller did not exit");
                thread::sleep(Duration::from_millis(10));
            }
        }
        assert!(!parent.child.wait().unwrap().success());
        let contender = release_tag(repo.path(), "0.9.2", None);
        let removal = worktree_remove(repo.path(), &checkout);
        fs::write(state.path().join("allow"), "").unwrap();
        assert!(
            matches!(contender, Err(OpsError::ReleaseDeferred { .. })),
            "{contender:?}"
        );
        assert!(
            removal.is_err(),
            "removed surviving notes provider checkout"
        );
        wait_for(&state.path().join("completed"));
        let context: Value =
            serde_json::from_slice(&fs::read(state.path().join("context.json")).unwrap()).unwrap();
        assert_eq!(context["version"], "0.9.1");
        wait_until_released(|| release_tag(repo.path(), "0.9.2", None).map(|_| ()));
        assert!(fs::read_to_string(checkout.join("RELEASE_NOTES.md"))
            .unwrap()
            .contains("Notes from the surviving provider."));
        assert_eq!(git(&["rev-parse", "HEAD"]), head);
        assert_eq!(git(&["branch", "--show-current"]), branch);
        assert_eq!(fs::read(repo.path().join(".git/index")).unwrap(), index);
        assert_eq!(
            fs::read_to_string(repo.path().join("local.txt")).unwrap(),
            "unstaged caller work\n"
        );
        assert_eq!(
            fs::read_to_string(repo.path().join("untracked.txt")).unwrap(),
            "untracked caller work\n"
        );
        assert_eq!(
            fs::read_to_string(repo.path().join("RELEASE_NOTES.md")).unwrap(),
            "# v0.9.0\n\nPrevious notes.\n"
        );
        worktree_remove(repo.path(), &checkout).unwrap();
    }
}

#[test]
fn surviving_release_lockfile_tool_retains_target_and_checkout() {
    for tool in ["cargo", "uv"] {
        let path = Command::new("which").arg(tool).output().unwrap();
        assert!(
            path.status.success(),
            "{tool} is required for the lockfile proof"
        );
        let real_tool = String::from_utf8(path.stdout).unwrap();
        let real_tool = real_tool.trim();
        for kill_controller in [true, false] {
            eprintln!("Lockfile tool {tool}, killed controller: {kill_controller}");
            let state = tempfile::tempdir().unwrap();
            let barrier = blocking_mutation(
                state.path(),
                &format!(
                    "'{real_tool}' \"$@\" > '{}/tool.log' 2>&1 || exit $?",
                    state.path().display()
                ),
            );
            let script = format!("pwd > '{}/checkout'\n{barrier}", state.path().display());
            let script = if kill_controller {
                format!("#!/bin/sh\n{script}")
            } else {
                format!(
                    "#!/bin/sh\n(\n{script}) > '{}/descendant.log' 2>&1 &\nexit 1\n",
                    state.path().display()
                )
            };
            let gh = "#!/bin/sh\ncase \"$1 $2\" in\n'--version ') exit 0;;\n'run list'|'pr list') echo '[]';;\n'release view') exit 1;;\n*) exit 91;;\nesac\n";
            let _env = EnvGuard::new(&[(tool, &script), ("gh", gh)]);
            let repo = TestRepo::new();
            let (manifest, lockfile, content, args) = if tool == "cargo" {
                fs::create_dir(repo.path().join("src")).unwrap();
                fs::write(repo.path().join("src/lib.rs"), "pub fn version() {}\n").unwrap();
                (
                    "Cargo.toml",
                    "Cargo.lock",
                    "[package]\nname = \"lock-proof\"\nversion = \"0.9.0\"\nedition = \"2021\"\n",
                    vec!["update", "--workspace"],
                )
            } else {
                ("pyproject.toml", "uv.lock", "[project]\nname = \"lock-proof\"\nversion = \"0.9.0\"\nrequires-python = \">=3.11\"\n", vec!["lock"])
            };
            fs::write(repo.path().join(manifest), content).unwrap();
            let initial = Command::new(real_tool)
                .args(&args)
                .env("CARGO_NET_OFFLINE", "true")
                .env("UV_OFFLINE", "true")
                .current_dir(repo.path())
                .output()
                .unwrap();
            assert!(initial.status.success(), "{initial:?}");
            let git = |args: &[&str]| {
                let output = Command::new("git")
                    .args(args)
                    .current_dir(repo.path())
                    .output()
                    .unwrap();
                assert!(output.status.success(), "{output:?}");
                output.stdout
            };
            git(&["add", "."]);
            git(&["commit", "-m", "Configure dependency-free manifest"]);
            git(&["push", "origin", "HEAD"]);
            fs::write(repo.path().join("local.txt"), "unpublished commit\n").unwrap();
            git(&["add", "local.txt"]);
            git(&["commit", "-m", "Keep caller work local"]);
            fs::write(repo.path().join("local.txt"), "staged caller work\n").unwrap();
            git(&["add", "local.txt"]);
            fs::write(repo.path().join("local.txt"), "unstaged caller work\n").unwrap();
            fs::write(repo.path().join("untracked.txt"), "untracked caller work\n").unwrap();
            let head = git(&["rev-parse", "HEAD"]);
            let branch = git(&["branch", "--show-current"]);
            let index = fs::read(repo.path().join(".git/index")).unwrap();
            // The wrapper inherits these settings; no external package resolution occurs.
            let log = fs::File::create(state.path().join("controller.log")).unwrap();
            let child = Command::new(env!("CARGO_BIN_EXE_lf"))
                .args(["release", "run", "0.9.1"])
                .current_dir(repo.path())
                .env_remove("LF_RELEASE_LOCK_FD")
                .env("CARGO_NET_OFFLINE", "true")
                .env("UV_OFFLINE", "true")
                .stdout(Stdio::from(log.try_clone().unwrap()))
                .stderr(Stdio::from(log))
                .spawn()
                .unwrap();
            let mut parent = MutationParent {
                child,
                state: state.path().to_path_buf(),
            };
            wait_for(&state.path().join("ready"));
            let checkout = PathBuf::from(
                fs::read_to_string(state.path().join("checkout"))
                    .unwrap()
                    .trim(),
            );
            if kill_controller {
                parent.child.kill().unwrap();
            } else {
                let deadline = Instant::now() + Duration::from_secs(20);
                while parent.child.try_wait().unwrap().is_none() {
                    assert!(Instant::now() < deadline, "{tool} controller did not exit");
                    thread::sleep(Duration::from_millis(10));
                }
            }
            assert!(!parent.child.wait().unwrap().success());
            let contender = release_tag(repo.path(), "0.9.2", None);
            let removal = worktree_remove(repo.path(), &checkout);
            fs::write(state.path().join("allow"), "").unwrap();
            assert!(
                matches!(contender, Err(OpsError::ReleaseDeferred { .. })),
                "{tool}: {contender:?}"
            );
            assert!(removal.is_err(), "removed surviving {tool} checkout");
            wait_for(&state.path().join("completed"));
            assert!(fs::read_to_string(checkout.join(lockfile))
                .unwrap()
                .contains("version = \"0.9.1\""));
            assert_eq!(git(&["rev-parse", "HEAD"]), head);
            assert_eq!(git(&["branch", "--show-current"]), branch);
            assert_eq!(fs::read(repo.path().join(".git/index")).unwrap(), index);
            assert_eq!(
                fs::read_to_string(repo.path().join("local.txt")).unwrap(),
                "unstaged caller work\n"
            );
            assert_eq!(
                fs::read_to_string(repo.path().join("untracked.txt")).unwrap(),
                "untracked caller work\n"
            );
            assert!(fs::read_to_string(repo.path().join(lockfile))
                .unwrap()
                .contains("version = \"0.9.0\""));
            wait_until_released(|| release_tag(repo.path(), "0.9.2", None).map(|_| ()));
            worktree_remove(repo.path(), &checkout).unwrap();
        }
    }
}

#[test]
fn surviving_release_hook_retains_target_and_checkout_ownership() {
    for (phase, kill_controller) in [
        ("verify", true),
        ("prepare", true),
        ("verify", false),
        ("prepare", false),
    ] {
        let state = tempfile::tempdir().unwrap();
        let gh = "#!/bin/sh\ncase \"$1 $2\" in\n'--version ') exit 0;;\n'run list'|'pr list') echo '[]';;\n'release view') exit 1;;\n*) exit 91;;\nesac\n";
        let _env = EnvGuard::new(&[("gh", gh)]);
        let repo = TestRepo::new();
        fs::create_dir_all(repo.path().join(".lf")).unwrap();
        fs::write(
            repo.path().join(".lf/config.yaml"),
            format!("release:\n  targets:\n    default:\n      completion: tag\n      {phase}: [sh hook.sh]\n"),
        ).unwrap();
        let barrier = blocking_mutation(
            state.path(),
            &format!("cat hook.sh > '{}/retained-source'", state.path().display()),
        );
        let script = format!("pwd > '{}/checkout'\n{barrier}", state.path().display());
        fs::write(
            repo.path().join("hook.sh"),
            if kill_controller {
                script
            } else {
                format!(
                    "(\n{script}\n) > '{}/descendant.log' 2>&1 &\nexit 1\n",
                    state.path().display()
                )
            },
        )
        .unwrap();
        for args in [
            vec!["add", ".lf/config.yaml", "hook.sh"],
            vec!["commit", "-m", "Configure release hook fixture"],
            vec!["push", "origin", "HEAD"],
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
        let bytes = fs::read(repo.path().join("hook.sh")).unwrap();
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .args(args)
                .current_dir(repo.path())
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
            output.stdout
        };
        fs::write(repo.path().join("local.txt"), "unpublished commit\n").unwrap();
        git(&["add", "local.txt"]);
        git(&["commit", "-m", "Keep caller work local"]);
        fs::write(repo.path().join("local.txt"), "staged caller work\n").unwrap();
        git(&["add", "local.txt"]);
        fs::write(repo.path().join("local.txt"), "unstaged caller work\n").unwrap();
        fs::write(repo.path().join("untracked.txt"), "untracked caller work\n").unwrap();
        let head = git(&["rev-parse", "HEAD"]);
        let branch = git(&["branch", "--show-current"]);
        let index = fs::read(repo.path().join(".git/index")).unwrap();
        let mut parent = start(&repo, state.path(), &["release", "run", "0.9.1"]);
        let checkout = PathBuf::from(
            fs::read_to_string(state.path().join("checkout"))
                .unwrap()
                .trim(),
        );
        if kill_controller {
            parent.child.kill().unwrap();
        } else {
            let deadline = Instant::now() + Duration::from_secs(20);
            while parent.child.try_wait().unwrap().is_none() {
                assert!(Instant::now() < deadline, "hook controller did not exit");
                thread::sleep(Duration::from_millis(10));
            }
        }
        assert!(!parent.child.wait().unwrap().success());
        assert_eq!(git(&["rev-parse", "HEAD"]), head);
        assert_eq!(git(&["branch", "--show-current"]), branch);
        assert_eq!(fs::read(repo.path().join(".git/index")).unwrap(), index);
        assert_eq!(
            fs::read_to_string(repo.path().join("local.txt")).unwrap(),
            "unstaged caller work\n"
        );
        assert_eq!(
            fs::read_to_string(repo.path().join("untracked.txt")).unwrap(),
            "untracked caller work\n"
        );
        assert!(
            checkout.exists(),
            "cleanup removed surviving {phase} hook's checkout"
        );
        let contender = release_tag(repo.path(), "0.9.2", None);
        let removal = worktree_remove(repo.path(), &checkout);
        fs::write(state.path().join("allow"), "").unwrap();
        wait_for(&state.path().join("completed"));
        assert!(
            matches!(contender, Err(OpsError::ReleaseDeferred { .. })),
            "{phase}: {contender:?}"
        );
        assert!(
            removal.is_err(),
            "removed surviving {phase} hook's checkout"
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

#[test]
fn surviving_release_source_mutation_retains_ownership_and_caller_state() {
    let real_git = Command::new("which").arg("git").output().unwrap();
    let real_git = String::from_utf8(real_git.stdout).unwrap();
    let real_git = real_git.trim();
    for stage in ["fetch", "add", "remove", "delete", "reset", "branch-fetch"] {
        for kill_controller in [true, false] {
            eprintln!("Source {stage}, killed controller: {kill_controller}");
            let state = tempfile::tempdir().unwrap();
            let rebuilding = matches!(stage, "reset" | "branch-fetch");
            let mutation = blocking_mutation(
                state.path(),
                &format!(
                    "'{real_git}' \"$@\" > '{}/git.log' 2>&1 || exit $?",
                    state.path().display()
                ),
            );
            let blocked = if kill_controller {
                mutation
            } else {
                format!(
                    "(\n{mutation}) > '{}/child.log' 2>&1 &\nexit 1\n",
                    state.path().display()
                )
            };
            let git_script = format!(
                r#"#!/bin/sh
operation=''
case "$*" in
  *'worktree add '*) operation=add;;
  *'worktree remove '*) operation=remove;;
  *'branch -D jack/verify-'*) operation=delete;;
  *'reset --hard origin/'*) operation=reset;;
  *'fetch origin +refs/heads/jack/release-'*) operation=branch-fetch;;
  *'fetch origin +refs/heads/main:'*) operation=fetch;;
esac
if [ -f '{state}/armed' ] && [ "$operation" = '{stage}' ]; then
  {blocked}
fi
exec '{real_git}' "$@"
"#,
                state = state.path().display()
            );
            let gh = format!(
                r#"#!/bin/sh
case "$1 $2" in
  '--version ') exit 0;;
  'run list') echo '[]';;
  'release view') exit 1;;
  'pr list')
    case " $* " in
      *' --head '*)
        if [ '{rebuilding}' = true ]; then
          printf '[{{"number":1176,"state":"OPEN","mergeCommit":null,"headRefOid":"%s"}}]\n' "$(cat '{state}/pr-head')"
        else echo '[]'; fi;;
      *) echo '[]';;
    esac;;
  'api graphql') echo '{{"data":{{"repository":{{"pullRequest":{{"number":1176,"url":"https://example.com/pr/1176","state":"OPEN","isDraft":false,"headRefName":"release","headRefOid":"observed","mergedAt":null,"mergeCommit":null,"mergeStateStatus":"DIRTY","isMergeQueueEnabled":false,"autoMergeRequest":null,"mergeQueueEntry":null}}}}}}}}';;
  *) exit 91;;
esac
"#,
                state = state.path().display()
            );
            let _env = EnvGuard::new(&[("git", &git_script), ("gh", &gh)]);
            let repo = TestRepo::new();
            let git = |args: &[&str]| {
                let out = Command::new(real_git)
                    .args(args)
                    .current_dir(repo.path())
                    .output()
                    .unwrap();
                assert!(out.status.success(), "{out:?}");
                out.stdout
            };
            fs::create_dir_all(repo.path().join(".lf")).unwrap();
            fs::write(
                repo.path().join(".lf/config.yaml"),
                if rebuilding {
                    "release:\n  targets:\n    default:\n      verify: []\n"
                } else {
                    "release:\n  targets:\n    default:\n      verify:\n      - sh -c 'exit 71'\n"
                },
            )
            .unwrap();
            git(&["add", "."]);
            git(&["commit", "-m", "Configure source proof"]);
            git(&["push", "origin", "HEAD"]);
            let source = String::from_utf8(git(&["rev-parse", "HEAD"]))
                .unwrap()
                .trim()
                .to_string();
            let branch = if rebuilding {
                "jack/release-default-v0-9-1".to_string()
            } else {
                format!("jack/verify-default-{source}")
            };
            if rebuilding {
                git(&["checkout", "-b", &branch]);
                fs::write(repo.path().join("prepared.txt"), "old preparation\n").unwrap();
                git(&["add", "prepared.txt"]);
                git(&["commit", "-m", "Prior preparation"]);
                fs::write(state.path().join("pr-head"), git(&["rev-parse", "HEAD"])).unwrap();
                git(&["push", "origin", &branch]);
                git(&["checkout", "main"]);
                git(&["branch", "-D", &branch]);
            }
            let checkout = loopflow::engine::worktrees::worktree_path(
                repo.path(),
                branch.strip_prefix("jack/").unwrap(),
            );
            fs::write(repo.path().join("local.txt"), "unpublished\n").unwrap();
            git(&["add", "local.txt"]);
            git(&["commit", "-m", "Keep caller local"]);
            fs::write(repo.path().join("local.txt"), "staged\n").unwrap();
            git(&["add", "local.txt"]);
            fs::write(repo.path().join("local.txt"), "unstaged\n").unwrap();
            fs::write(repo.path().join("untracked.txt"), "untracked\n").unwrap();
            let head = git(&["rev-parse", "HEAD"]);
            let caller_branch = git(&["branch", "--show-current"]);
            let index = fs::read(repo.path().join(".git/index")).unwrap();
            let remote_heads = git(&["ls-remote", "--heads", "origin"]);
            // Prove each fetch advances an existing stale observation.
            if stage == "fetch" {
                git(&[
                    "update-ref",
                    "refs/remotes/origin/main",
                    &format!("{source}^"),
                ]);
            } else if stage == "branch-fetch" {
                git(&[
                    "update-ref",
                    &format!("refs/remotes/origin/{branch}"),
                    &source,
                ]);
            }
            fs::write(state.path().join("armed"), "").unwrap();
            let mut parent = start(&repo, state.path(), &["release", "run", "0.9.1"]);
            if kill_controller {
                parent.child.kill().unwrap();
            } else {
                let deadline = Instant::now() + Duration::from_secs(20);
                while parent.child.try_wait().unwrap().is_none() {
                    assert!(Instant::now() < deadline, "controller did not exit");
                    thread::sleep(Duration::from_millis(10));
                }
            }
            assert!(!parent.child.wait().unwrap().success());
            let contender = release_tag(repo.path(), "0.9.2", None);
            let protected_checkout = matches!(stage, "add" | "remove" | "reset");
            let removal = protected_checkout.then(|| worktree_remove(repo.path(), &checkout));
            fs::write(state.path().join("allow"), "").unwrap();
            assert!(
                matches!(contender, Err(OpsError::ReleaseDeferred { .. })),
                "{contender:?}"
            );
            if let Some(removal) = removal {
                assert!(removal.is_err(), "removed active {stage} checkout");
            }
            wait_for(&state.path().join("completed"));
            wait_until_released(|| release_tag(repo.path(), "0.9.2", None).map(|_| ()));
            match stage {
                "add" | "reset" => {
                    let out = Command::new(real_git)
                        .args(["rev-parse", "HEAD"])
                        .current_dir(&checkout)
                        .output()
                        .unwrap();
                    assert!(out.status.success(), "{out:?}");
                    assert_eq!(String::from_utf8(out.stdout).unwrap().trim(), source);
                    assert!(!checkout.join("prepared.txt").exists());
                    worktree_remove(repo.path(), &checkout).unwrap();
                }
                "remove" => assert!(!checkout.exists()),
                "delete" => assert!(!Command::new(real_git)
                    .args(["show-ref", "--verify", &format!("refs/heads/{branch}")])
                    .current_dir(repo.path())
                    .output()
                    .unwrap()
                    .status
                    .success()),
                "fetch" => assert_eq!(
                    String::from_utf8(git(&["rev-parse", "origin/main"]))
                        .unwrap()
                        .trim(),
                    source
                ),
                "branch-fetch" => assert_eq!(
                    git(&["rev-parse", &format!("origin/{branch}")]),
                    fs::read(state.path().join("pr-head")).unwrap()
                ),
                _ => unreachable!(),
            }
            assert_eq!(
                git(&["ls-remote", "--heads", "origin"]),
                remote_heads,
                "source checkout operations must not publish a branch"
            );
            assert_eq!(git(&["rev-parse", "HEAD"]), head);
            assert_eq!(git(&["branch", "--show-current"]), caller_branch);
            assert_eq!(fs::read(repo.path().join(".git/index")).unwrap(), index);
            assert_eq!(
                fs::read_to_string(repo.path().join("local.txt")).unwrap(),
                "unstaged\n"
            );
            assert_eq!(
                fs::read_to_string(repo.path().join("untracked.txt")).unwrap(),
                "untracked\n"
            );
        }
    }
}

#[test]
fn source_creation_mismatch_preserves_surviving_hook_and_its_work() {
    let state = tempfile::tempdir().unwrap();
    let _env = EnvGuard::new(&[(
        "gh",
        "#!/bin/sh\ncase \"$1 $2\" in\n'--version ') exit 0;;\n'run list'|'pr list') echo '[]';;\n'release view') exit 1;;\n*) exit 91;;\nesac\n",
    )]);
    let repo = TestRepo::new();
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo.path())
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    };
    fs::create_dir_all(repo.path().join(".lf")).unwrap();
    fs::write(
        repo.path().join(".lf/config.yaml"),
        "release:\n  targets:\n    default:\n      verify:\n      - sh -c 'exit 71'\n",
    )
    .unwrap();
    git(&["add", "."]);
    git(&["commit", "-m", "Configure source proof"]);
    git(&["push", "origin", "HEAD"]);
    let source = git(&["rev-parse", "HEAD"]);
    let caller_branch = git(&["branch", "--show-current"]);
    let index = fs::read(repo.path().join(".git/index")).unwrap();
    let name = format!("verify-default-{source}");
    let checkout = loopflow::engine::worktrees::worktree_path(repo.path(), &name);
    let branch = format!("jack/{name}");
    let mutation = blocking_mutation(
        state.path(),
        &format!(
            "cat repair.txt > '{}/retained-work' || exit $?",
            state.path().display()
        ),
    );
    let hook = repo.path().join(".git/hooks/post-checkout");
    fs::write(
        &hook,
        format!(
            r#"#!/bin/sh
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE
printf 'hook-owned work\n' > repair.txt
git add repair.txt || exit $?
git -c core.hooksPath=/dev/null commit -m 'Retain hook work' || exit $?
git rev-parse HEAD > '{state}/hook-head'
(
{mutation}
) > '{state}/hook.log' 2>&1 &
exit 1
"#,
            state = state.path().display()
        ),
    )
    .unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    let mut parent = start(&repo, state.path(), &["release", "run", "0.9.1"]);
    let deadline = Instant::now() + Duration::from_secs(20);
    while parent.child.try_wait().unwrap().is_none() {
        assert!(Instant::now() < deadline, "controller did not exit");
        thread::sleep(Duration::from_millis(10));
    }
    assert!(!parent.child.wait().unwrap().success());
    let log = fs::read_to_string(state.path().join("controller.log")).unwrap();
    assert!(log.contains("expected"), "{log}");
    assert!(
        checkout.exists(),
        "mismatch cleanup removed live hook checkout: {log}"
    );
    let hook_head = fs::read_to_string(state.path().join("hook-head")).unwrap();
    assert_eq!(git(&["rev-parse", &branch]), hook_head.trim());
    assert!(matches!(
        release_tag(repo.path(), "0.9.2", None),
        Err(OpsError::ReleaseDeferred { .. })
    ));
    assert!(worktree_remove(repo.path(), &checkout).is_err());
    fs::write(state.path().join("allow"), "").unwrap();
    wait_for(&state.path().join("completed"));
    assert_eq!(
        fs::read_to_string(state.path().join("retained-work")).unwrap(),
        "hook-owned work\n"
    );
    wait_until_released(|| release_tag(repo.path(), "0.9.2", None).map(|_| ()));
    assert_eq!(git(&["rev-parse", "HEAD"]), source);
    assert_eq!(git(&["branch", "--show-current"]), caller_branch);
    assert_eq!(fs::read(repo.path().join(".git/index")).unwrap(), index);
    worktree_remove(repo.path(), &checkout).unwrap();
    assert_eq!(git(&["rev-parse", &branch]), hook_head.trim());
}

#[test]
fn surviving_task_revocation_retains_release_ownership_and_settlement_intent() {
    for kill_controller in [true, false] {
        let state = tempfile::tempdir().unwrap();
        let mutation = blocking_mutation(
            state.path(),
            &format!("printf disabled > '{}/remote-auto'", state.path().display()),
        );
        let blocked = if kill_controller {
            mutation
        } else {
            format!(
                "(\n{mutation}) > '{}/descendant.log' 2>&1 &\nexit 1",
                state.path().display()
            )
        };
        let gh = format!(
            r#"#!/bin/sh
case "$1 $2" in
'--version ') exit 0;;
'run list'|'pr list') echo '[]'; exit 0;;
'release view') exit 1;;
'api graphql')
  if [ "$(cat '{state}/remote-auto')" = armed ]; then echo true; else echo false; fi
  exit 0;;
'pr merge')
  [ "$3 $4" = '912 --disable-auto' ] || exit 92
  {blocked}
  ;;
esac
exit 91
"#,
            state = state.path().display()
        );
        let notes = format!(
            r#"#!/bin/sh
pwd > '{state}/checkout'
: > '{state}/ready'
while [ ! -f '{state}/registered' ]; do
  [ -d '{state}' ] || exit 1
  sleep 0.02
done
cat > RELEASE_NOTES.md <<'NOTES'
# v0.9.1

<!-- loopflow:release-notes=narrative;gate=safe -->

Fixture release.
NOTES
"#,
            state = state.path().display()
        );
        let _env = EnvGuard::new(&[("gh", &gh), ("lf", &notes)]);
        let repo = TestRepo::new();
        repo.create_file("local.txt", "unpublished\n");
        repo.stage_all();
        repo.commit("caller local work");
        repo.create_file("staged.txt", "staged\n");
        repo.stage_all();
        repo.create_file("staged.txt", "working\n");
        repo.create_file("untracked.txt", "untracked\n");
        let caller_head = repo.head_sha();
        let caller_branch = current_branch(repo.path()).unwrap();
        let caller_index = fs::read(repo.path().join(".git/index")).unwrap();
        let mut parent = start(&repo, state.path(), &["release", "run", "0.9.1"]);
        let checkout = PathBuf::from(
            fs::read_to_string(state.path().join("checkout"))
                .unwrap()
                .trim(),
        );
        let git_read = |args: &[&str]| {
            let output = Command::new("git")
                .arg("-C")
                .arg(&checkout)
                .args(args)
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
            String::from_utf8(output.stdout).unwrap().trim().to_string()
        };
        let head = git_read(&["rev-parse", "HEAD"]);
        let branch = git_read(&["branch", "--show-current"]);
        // Register the actual generated branch, not the caller's ambient Task.
        let home = PathBuf::from(std::env::var_os("LF_HOME").unwrap());
        let task = support::register_task(&home, &checkout, &branch, &head);
        let now = time::OffsetDateTime::now_utc();
        let mut pr = task.pr.clone();
        pr.publication = Some(PrPublication {
            requested_at: now,
            presentation: Some(PrPresentation {
                title: "Task proof".to_string(),
                body: "Retain settlement intent".to_string(),
                head_sha: head.clone(),
            }),
            github: Some(GithubPr {
                number: 912,
                url: "https://example.com/pr/912".to_string(),
                head_sha: Some(head.clone()),
            }),
            merge: Some(PrMergeRequest {
                mode: PrMergeMode::Auto,
                requested_at: now,
                head_sha: head.clone(),
                after_merge: AfterMerge::CompleteTask,
                next_slug: None,
            }),
        });
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(task.store.update_task_pr(&pr)).unwrap();
        let pr = runtime
            .block_on(task.store.active_task_pr(&task.task.id))
            .unwrap()
            .unwrap();
        fs::write(state.path().join("remote-auto"), "armed").unwrap();
        fs::remove_file(state.path().join("ready")).unwrap();
        fs::write(state.path().join("registered"), "").unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        while !state.path().join("ready").exists() {
            assert!(
                parent.child.try_wait().unwrap().is_none() && Instant::now() < deadline,
                "revocation not reached: {}",
                fs::read_to_string(state.path().join("controller.log")).unwrap()
            );
            thread::sleep(Duration::from_millis(10));
        }
        if kill_controller {
            parent.child.kill().unwrap();
        } else {
            let deadline = Instant::now() + Duration::from_secs(20);
            while parent.child.try_wait().unwrap().is_none() {
                assert!(Instant::now() < deadline, "controller did not exit");
                thread::sleep(Duration::from_millis(10));
            }
        }
        assert!(!parent.child.wait().unwrap().success());
        let contender = release_tag(repo.path(), "0.9.2", None);
        let removal = worktree_remove(repo.path(), &checkout);
        let persisted = runtime
            .block_on(task.store.active_task_pr(&task.task.id))
            .unwrap()
            .unwrap();
        assert_eq!(persisted.merge_request(), pr.merge_request());
        assert_eq!(
            fs::read_to_string(state.path().join("remote-auto")).unwrap(),
            "armed"
        );
        let remote = Command::new("git")
            .arg("--git-dir")
            .arg(repo.bare_path())
            .args(["show-ref", "--verify", &format!("refs/heads/{branch}")])
            .output()
            .unwrap();
        assert!(
            !remote.status.success(),
            "pushed before revocation completed"
        );
        fs::write(state.path().join("allow"), "").unwrap();
        assert!(
            matches!(contender, Err(OpsError::ReleaseDeferred { .. })),
            "{contender:?}"
        );
        assert!(removal.is_err(), "removed surviving revocation checkout");
        assert!(checkout.join("RELEASE_NOTES.md").exists());
        wait_for(&state.path().join("completed"));
        assert_eq!(
            fs::read_to_string(state.path().join("remote-auto")).unwrap(),
            "disabled"
        );
        assert_eq!(repo.head_sha(), caller_head);
        assert_eq!(current_branch(repo.path()).unwrap(), caller_branch);
        assert_eq!(
            fs::read(repo.path().join(".git/index")).unwrap(),
            caller_index
        );
        assert_eq!(
            fs::read_to_string(repo.path().join("staged.txt")).unwrap(),
            "working\n"
        );
        assert_eq!(
            fs::read_to_string(repo.path().join("untracked.txt")).unwrap(),
            "untracked\n"
        );
        wait_until_released(|| release_tag(repo.path(), "0.9.2", None).map(|_| ()));
        // Replay observes remote revocation before clearing durable intent and pushing.
        let persisted = runtime
            .block_on(task.store.active_task_pr(&task.task.id))
            .unwrap()
            .unwrap();
        assert_eq!(persisted.merge_request(), pr.merge_request());
        commit_workflow(
            &checkout,
            &CommitOptions {
                add: false,
                push: true,
                create_draft_pr: false,
                task: "commit".to_string(),
                sources: Vec::new(),
                message: Some("retry prepared Task head".to_string()),
                agent: None,
            },
            &NullProgress,
            &|_| {},
        )
        .unwrap();
        let persisted = runtime
            .block_on(task.store.active_task_pr(&task.task.id))
            .unwrap()
            .unwrap();
        assert!(persisted.merge_request().is_none());
        let remote = Command::new("git")
            .arg("--git-dir")
            .arg(repo.bare_path())
            .args(["rev-parse", &branch])
            .output()
            .unwrap();
        assert!(remote.status.success());
        assert_eq!(
            String::from_utf8(remote.stdout).unwrap().trim(),
            git_read(&["rev-parse", "HEAD"])
        );
        worktree_remove(repo.path(), &checkout).unwrap();
    }
}

#[test]
fn surviving_ci_repair_keeps_release_and_checkout_exclusive_after_controller_death() {
    let merge_state = "BLOCKED";
    let awaiting_queue = false;
    let state = tempfile::tempdir().unwrap();
    let repaired = state.path().join("repaired");
    let checks = support::github_checks_page(
        "$head",
        &[
            ("tests-result", "FAILURE", true),
            ("swift-test", "FAILURE", false),
        ],
    );
    let open = support::github_merge_response(
        1309,
        "$head",
        "OPEN",
        merge_state,
        Some(if awaiting_queue {
            "awaiting_queue"
        } else {
            "auto"
        }),
    );
    let merged = support::github_merge_response(1309, "$head", "MERGED", "UNKNOWN", None);
    // Keep the release poll and both repair observations stale. The error-path
    // refresh is the first authoritative merge observation.
    let stale_views = 0;
    let gh = format!(
        r#"#!/bin/sh
repaired='{}'
head=$(git rev-parse HEAD)
case "$1 $2" in
  '--version ') echo 'gh fixture';;
  'release view') echo 'release not found' >&2; exit 1;;
  'run list') echo '[]';;
  'pr list')
    case " $* " in
      *' --head '*) printf '[{{"number":1309,"state":"OPEN","mergeCommit":null,"url":"https://github.com/loopflowstudio/release-fixture/pull/1309","headRefOid":"%s"}}]\n' "$head";;
      *) echo '[]';;
    esac;;
  'api graphql')
    case "$*" in
      *LoopflowPrChecks*)
        cat <<JSON
{checks}
JSON
        ;;
      *)
        if [ -f "$repaired" ]; then
          count=0
          [ ! -f "$repaired.views" ] || count=$(cat "$repaired.views")
          count=$((count + 1))
          printf '%s' "$count" > "$repaired.views"
          if [ "$count" -le {stale_views} ]; then
            cat <<JSON
{open}
JSON
            exit 0
          fi
          cat <<JSON
{merged}
JSON
        else
          cat <<JSON
{open}
JSON
        fi ;;

    esac;;
  *) echo "unexpected gh: $*" >&2; exit 1;;
esac
"#,
        repaired.display()
    );

    let barrier = format!("pwd > '{0}/checkout'\n: > '{0}/ready'\nwhile [ ! -f '{0}/allow' ]; do [ -d '{0}' ] || exit 1; sleep 0.02; done\nprintf 'surviving repair\n' > repair.txt\n: > '{0}/completed'\n", state.path().display());
    let codex = codex_app_server_script(
        r#"{"status":"published","summary":"Fixture repair finished."}"#,
        "",
    )
    .replace(
        "read -r turn_start\n",
        &format!("read -r turn_start\n{barrier}"),
    );
    let _env = EnvGuard::new(&[
        ("gh", &gh),
        ("codex", &codex),
        ("tmux", "#!/bin/sh\nexit 92\n"),
    ]);
    let git_output = |repo: &TestRepo, args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(repo.path())
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    };
    let git = |repo: &TestRepo, args: &[&str]| {
        git_output(repo, args);
    };
    let repo = TestRepo::new();
    git(&repo, &["tag", "v0.9.1"]);
    git(&repo, &["push", "origin", "v0.9.1"]);
    fs::create_dir_all(repo.path().join(".lf")).unwrap();
    fs::write(repo.path().join(".lf/config.yaml"), "agent: codex\n").unwrap();
    fs::write(repo.path().join("feature.txt"), "release me").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "Release fixture"]);
    git(&repo, &["push", "origin", "HEAD"]);
    let head = git_output(&repo, &["rev-parse", "HEAD"]);
    let branch = format!(
        "{}/release-default-v0-9-2",
        loopflow::engine::naming::git_user(repo.path()).unwrap()
    );
    git(
        &repo,
        &["push", "origin", &format!("HEAD:refs/heads/{branch}")],
    );
    let local_remote = git_output(&repo, &["remote", "get-url", "origin"]);
    let github_remote = "https://github.com/loopflowstudio/release-fixture.git";
    git(
        &repo,
        &[
            "config",
            &format!("url.{local_remote}.insteadOf"),
            github_remote,
        ],
    );
    git(&repo, &["remote", "set-url", "origin", github_remote]);

    fs::write(repo.path().join("caller.txt"), "local commit\n").unwrap();
    git(&repo, &["add", "caller.txt"]);
    git(&repo, &["commit", "-m", "Keep local work"]);
    fs::write(repo.path().join("caller.txt"), "staged\n").unwrap();
    git(&repo, &["add", "caller.txt"]);
    fs::write(repo.path().join("caller.txt"), "unstaged\n").unwrap();
    fs::write(repo.path().join("untracked.txt"), "untouched\n").unwrap();
    let caller_head = repo.head_sha();
    let caller_branch = current_branch(repo.path()).unwrap();
    let caller_index = fs::read(repo.path().join(".git/index")).unwrap();
    // Provider facts name the published release branch, not the caller's local commit.
    let gh = gh.replace("head=$(git rev-parse HEAD)", &format!("head={head}"));
    let executable = std::env::var_os("PATH").unwrap();
    let gh_path = std::env::split_paths(&executable)
        .map(|p| p.join("gh"))
        .find(|p| p.is_file())
        .unwrap();
    fs::write(gh_path, gh).unwrap();
    let mut parent = start(&repo, state.path(), &["repo", "release", "run", "patch"]);
    let checkout = PathBuf::from(
        fs::read_to_string(state.path().join("checkout"))
            .unwrap()
            .trim(),
    );
    parent.child.kill().unwrap();
    assert!(!parent.child.wait().unwrap().success());
    assert!(matches!(
        release_tag(repo.path(), "0.9.3", None),
        Err(OpsError::ReleaseDeferred { .. })
    ));
    assert!(worktree_remove(repo.path(), &checkout).is_err());
    assert_eq!(repo.head_sha(), caller_head);
    assert_eq!(current_branch(repo.path()).unwrap(), caller_branch);
    assert_eq!(
        fs::read(repo.path().join(".git/index")).unwrap(),
        caller_index
    );
    assert_eq!(
        fs::read_to_string(repo.path().join("caller.txt")).unwrap(),
        "unstaged\n"
    );
    assert_eq!(
        fs::read_to_string(repo.path().join("untracked.txt")).unwrap(),
        "untouched\n"
    );
    fs::write(state.path().join("allow"), "").unwrap();
    wait_for(&state.path().join("completed"));
    assert_eq!(
        fs::read_to_string(checkout.join("repair.txt")).unwrap(),
        "surviving repair\n"
    );
    wait_until_released(|| release_tag(repo.path(), "0.9.3", None).map(|_| ()));
}
