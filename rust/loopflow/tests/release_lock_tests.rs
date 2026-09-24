mod support;

use std::fs;
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
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
if [ '{phase}' = upstream ]; then
  '{real_git}' branch --unset-upstream || exit $?
fi
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
