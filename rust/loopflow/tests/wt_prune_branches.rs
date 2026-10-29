use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};

use loopflow_test_support::TestRepo;

const BUDGET: &str =
    r#"{"resources":{"graphql":{"limit":5000,"remaining":4921,"reset":1792530300}}}"#;

fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// A repository whose origin reads as GitHub but resolves to the fixture remote.
fn github_fixture() -> TestRepo {
    let repo = TestRepo::new();
    let bare = repo.bare_path().to_str().unwrap().to_string();
    let github = "https://github.com/acme/widgets.git";
    git(repo.path(), &["remote", "set-url", "origin", github]);
    git(
        repo.path(),
        &["config", &format!("url.{bare}.insteadOf"), github],
    );
    repo
}

fn commit_branch(repo: &TestRepo, branch: &str) -> String {
    git(repo.path(), &["checkout", "-q", "-b", branch, "main"]);
    repo.create_file(&format!("{branch}.txt"), branch);
    repo.stage_all();
    repo.commit(branch);
    git(repo.path(), &["push", "-q", "origin", branch]);
    let head = repo.head_sha();
    git(repo.path(), &["checkout", "-q", "main"]);
    head
}

fn origin_heads(repo: &TestRepo) -> Vec<String> {
    git(
        repo.bare_path(),
        &["for-each-ref", "--format=%(refname:short)", "refs/heads"],
    )
    .lines()
    .map(str::to_string)
    .collect()
}

fn local_heads(repo: &TestRepo) -> Vec<String> {
    git(
        repo.path(),
        &["for-each-ref", "--format=%(refname:short)", "refs/heads"],
    )
    .lines()
    .map(str::to_string)
    .collect()
}

/// A `gh` that answers the budget and each aliased branch from `pulls`.
fn gh_stub(pulls: &HashMap<&str, serde_json::Value>) -> String {
    format!(
        r#"#!/usr/bin/env python3
import json, re, sys
pulls = json.loads({pulls:?})
if sys.argv[1:3] == ["api", "rate_limit"]:
    print({BUDGET:?})
    sys.exit(0)
query = next(arg[6:] for arg in sys.argv if arg.startswith("query="))
repository = {{}}
for index, branch in re.findall(r'b(\d+): pullRequests\(first: 100, headRefName: "([^"]+)"', query):
    repository["b" + index] = {{"nodes": pulls.get(branch, [])}}
    repository["r" + index] = {{"id": "ref"}}
print(json.dumps({{"data": {{"repository": repository}}}}))
"#,
        pulls = serde_json::to_string(pulls).unwrap(),
    )
}

fn run_lf(repo: &TestRepo, gh: &str, args: &[&str]) -> Output {
    let home = tempfile::tempdir().unwrap();
    run_lf_at_home(repo, gh, args, home.path())
}

fn run_lf_at_home(repo: &TestRepo, gh: &str, args: &[&str], home: &Path) -> Output {
    let bin = home.join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("gh"), gh).unwrap();
    fs::set_permissions(bin.join("gh"), fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    command
        .env_clear()
        .env("HOME", home)
        .env("LF_HOME", home.join(".lf"))
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("PATH", path)
        .current_dir(repo.path())
        .args(args);
    command.output().expect("run lf")
}

fn prune(repo: &TestRepo, gh: &str, dry_run: bool) -> Output {
    let mut args = vec!["wt", "prune", "--branches"];
    if dry_run {
        args.push("--dry-run");
    }
    run_lf(repo, gh, &args)
}

fn pull(number: u64, head: &str, state: &str) -> serde_json::Value {
    serde_json::json!({"number": number, "headRefOid": head, "state": state})
}

#[test]
fn deletes_finished_branches_and_retains_open_and_moved_work() {
    let repo = github_fixture();
    let closed = commit_branch(&repo, "closed-feature");
    let open = commit_branch(&repo, "open-feature");
    commit_branch(&repo, "moved-feature");
    let main = repo.head_sha();
    let candidate = format!("release-candidate/default/v1-0-0/{main}");
    git(
        repo.path(),
        &[
            "push",
            "-q",
            "origin",
            &format!("{main}:refs/heads/{candidate}"),
        ],
    );
    git(repo.path(), &["tag", "v1.0.0"]);
    git(repo.path(), &["push", "-q", "origin", "v1.0.0"]);
    let pulls = HashMap::from([
        (
            "closed-feature",
            serde_json::json!([pull(11, &closed, "CLOSED")]),
        ),
        (
            "open-feature",
            serde_json::json!([pull(12, &open, "CLOSED"), pull(14, "elsewhere", "OPEN")]),
        ),
        (
            "moved-feature",
            serde_json::json!([pull(13, "older", "MERGED")]),
        ),
    ]);
    let gh = gh_stub(&pulls);

    let dry = prune(&repo, &gh, true);
    let stdout = String::from_utf8_lossy(&dry.stdout);
    assert!(
        dry.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&dry.stderr)
    );
    assert!(
        stdout.contains("would delete origin closed-feature  closed PR #11"),
        "{stdout}"
    );
    assert!(
        stdout.contains("would delete local  closed-feature"),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("would delete origin {candidate}  released v1.0.0")),
        "{stdout}"
    );
    assert!(
        stdout.contains("retained origin open-feature  open PR #14"),
        "{stdout}"
    );
    assert!(
        stdout.contains("retained origin moved-feature  head moved past merged PR #13"),
        "{stdout}"
    );
    assert!(
        stdout.contains("would delete 2 remote, 1 local"),
        "{stdout}"
    );
    assert!(origin_heads(&repo).contains(&"closed-feature".to_string()));

    let run = prune(&repo, &gh, false);
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(stdout.contains("deleted 2 remote, 1 local"), "{stdout}");
    assert_eq!(
        origin_heads(&repo),
        vec!["main", "moved-feature", "open-feature"]
    );
    assert_eq!(
        local_heads(&repo),
        vec!["main", "moved-feature", "open-feature"]
    );

    let again = prune(&repo, &gh, false);
    assert!(String::from_utf8_lossy(&again.stdout).contains("deleted 0 remote, 0 local"));
}

#[test]
fn unauthenticated_github_deletes_nothing_and_fails() {
    let repo = github_fixture();
    commit_branch(&repo, "closed-feature");
    let before = origin_heads(&repo);
    let gh = "#!/bin/sh\necho 'To get started with GitHub CLI, please run:  gh auth login' >&2\nexit 4\n";

    let run = prune(&repo, gh, false);

    assert_eq!(run.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        stderr.contains("GitHub authentication failed: "),
        "{stderr}"
    );
    assert!(stderr.contains("nothing deleted"), "{stderr}");
    assert_eq!(origin_heads(&repo), before);
}

#[test]
fn rate_limited_github_deletes_nothing_and_fails() {
    let repo = github_fixture();
    commit_branch(&repo, "closed-feature");
    let before = origin_heads(&repo);
    let gh = format!(
        r#"#!/bin/sh
if [ "$2" = "rate_limit" ]; then echo '{BUDGET}'; exit 0; fi
echo '{{"errors":[{{"type":"RATE_LIMITED","message":"API rate limit exceeded"}}]}}'
echo 'gh: API rate limit exceeded for user ID 1.' >&2
exit 1
"#
    );

    let run = prune(&repo, &gh, false);

    assert_eq!(run.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(stderr.contains("GitHub rate limited"), "{stderr}");
    assert_eq!(origin_heads(&repo), before);
}

#[test]
fn a_low_graphql_budget_skips_the_run() {
    let repo = github_fixture();
    commit_branch(&repo, "closed-feature");
    let before = origin_heads(&repo);
    let gh = r#"#!/bin/sh
echo '{"resources":{"graphql":{"limit":5000,"remaining":12,"reset":1792530300}}}'
"#;

    let run = prune(&repo, gh, false);

    assert_eq!(run.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        stderr.contains("GitHub GraphQL budget 12 left until"),
        "{stderr}"
    );
    assert_eq!(origin_heads(&repo), before);
}

#[test]
fn worktree_prune_requires_terminal_evidence_at_the_current_head() {
    let repo = github_fixture();
    let mut pulls = HashMap::new();
    let mut retained = Vec::new();
    let mut finished = Vec::new();
    for (index, (branch, state)) in [
        ("never-pushed", None),
        ("remote-deleted", None),
        ("merged", Some("MERGED")),
        ("squashed", Some("MERGED")),
        ("closed", Some("CLOSED")),
        ("moved", Some("MERGED")),
        ("open", Some("OPEN")),
        ("dirty", Some("MERGED")),
        ("untracked", Some("CLOSED")),
        ("persistent", Some("MERGED")),
    ]
    .into_iter()
    .enumerate()
    {
        let path = repo.create_named_worktree(branch);
        fs::write(path.join("work.txt"), branch).unwrap();
        git(&path, &["add", "."]);
        git(&path, &["commit", "-qm", branch]);
        let head = git(&path, &["rev-parse", "HEAD"]);
        if let Some(state) = state {
            pulls.insert(
                branch,
                serde_json::json!([pull(index as u64 + 1, &head, state)]),
            );
        }
        match branch {
            "merged" => {
                git(repo.path(), &["merge", "--no-edit", branch]);
            }
            "squashed" => {
                // Separate file avoids a conflict with the normal merge fixture.
                git(&path, &["mv", "work.txt", "squashed.txt"]);
                git(&path, &["commit", "-qm", "rename"]);
                let head = git(&path, &["rev-parse", "HEAD"]);
                pulls.insert(branch, serde_json::json!([pull(2, &head, "MERGED")]));
                git(repo.path(), &["merge", "--squash", branch]);
                git(repo.path(), &["commit", "-qm", "squash"]);
            }
            "closed" => {
                git(&path, &["push", "origin", branch]);
            }
            "remote-deleted" => {
                git(&path, &["push", "origin", branch]);
                git(&path, &["push", "origin", "--delete", branch]);
            }
            "moved" => {
                git(
                    &path,
                    &["commit", "--allow-empty", "-qm", "unpublished follow-up"],
                );
            }
            "dirty" => {
                fs::write(path.join("work.txt"), "unfinished edits").unwrap();
            }
            "untracked" => {
                fs::write(path.join("notes.txt"), "unfinished notes").unwrap();
            }
            "persistent" => {
                git(
                    &path,
                    &["config", "branch.persistent.loopflow-persistent", "true"],
                );
            }
            _ => {}
        }
        let head = git(&path, &["rev-parse", "HEAD"]);
        if matches!(branch, "merged" | "squashed" | "closed") {
            finished.push((branch, path));
        } else {
            let content = fs::read(path.join("work.txt")).unwrap();
            retained.push((branch, path, head, content));
        }
    }
    git(repo.path(), &["push", "origin", "main"]);
    let fresh = repo.create_named_worktree("fresh");
    let gh = gh_stub(&pulls);
    let dry = run_lf(&repo, &gh, &["wt", "prune", "--dry-run"]);
    assert!(
        dry.status.success(),
        "{}",
        String::from_utf8_lossy(&dry.stderr)
    );
    for (_, path) in &finished {
        assert!(path.exists());
    }

    let run = run_lf(&repo, &gh, &["wt", "prune"]);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    for (branch, path) in &finished {
        assert!(
            !path.exists(),
            "{branch}: {}",
            String::from_utf8_lossy(&run.stdout)
        );
        assert!(!local_heads(&repo).contains(&branch.to_string()));
    }
    assert!(origin_heads(&repo).contains(&"closed".to_string()));
    for (branch, path, head, content) in &retained {
        assert!(path.exists(), "{branch}");
        assert_eq!(&git(path, &["rev-parse", "HEAD"]), head);
        assert_eq!(&git(repo.path(), &["rev-parse", branch]), head);
        assert_eq!(&fs::read(path.join("work.txt")).unwrap(), content);
        if *branch == "untracked" {
            assert_eq!(
                fs::read_to_string(path.join("notes.txt")).unwrap(),
                "unfinished notes"
            );
        }
    }
    assert!(fresh.exists());
    let again = run_lf(&repo, &gh, &["wt", "prune"]);
    assert!(again.status.success());
    assert!(String::from_utf8_lossy(&again.stdout).contains("0 removed,"));
}

#[test]
fn worktree_prune_retains_work_when_remote_evidence_is_unavailable() {
    let repo = github_fixture();
    let path = repo.create_named_worktree("unpublished");
    fs::write(path.join("work.txt"), "only local copy").unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-qm", "unpublished"]);
    let head = git(&path, &["rev-parse", "HEAD"]);
    for gh in [
        "#!/bin/sh\nexit 1\n",
        "#!/bin/sh\necho malformed\n",
        &gh_stub(&HashMap::new()),
    ] {
        let run = run_lf(&repo, gh, &["wt", "prune"]);
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(git(&path, &["rev-parse", "HEAD"]), head);
        assert_eq!(
            fs::read_to_string(path.join("work.txt")).unwrap(),
            "only local copy"
        );
    }
}

#[test]
fn worktree_prune_preserves_unfinished_tasks_and_unknown_inspection() {
    let repo = github_fixture();
    let home = tempfile::tempdir().unwrap();
    let path = repo.create_named_worktree("unfinished");
    crate::support::register_task_without_pr(&home.path().join(".lf"), &path, "unfinished", "HEAD");
    let gh = gh_stub(&HashMap::from([(
        "unfinished",
        serde_json::json!([pull(1, &repo.head_sha(), "MERGED")]),
    )]));
    let run = run_lf_at_home(&repo, &gh, &["wt", "prune"], home.path());
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(path.exists());

    let database = home.path().join(".lf/loopflow.db");
    fs::write(&database, "unreadable registry").unwrap();
    let run = run_lf_at_home(&repo, &gh, &["wt", "prune"], home.path());
    assert!(!run.status.success());
    assert!(path.exists());
    assert!(local_heads(&repo).contains(&"unfinished".to_string()));

    let home = tempfile::tempdir().unwrap();
    fs::create_dir(home.path().join("bin")).unwrap();
    let lsof = home.path().join("bin/lsof");
    fs::write(&lsof, "#!/bin/sh\nexit 1\n").unwrap();
    fs::set_permissions(lsof, fs::Permissions::from_mode(0o755)).unwrap();
    let run = run_lf_at_home(&repo, &gh, &["wt", "prune"], home.path());
    assert!(!run.status.success());
    assert!(path.exists());
}

#[test]
fn worktree_prune_preserves_a_live_process_checkout() {
    let repo = github_fixture();
    let path = repo.create_named_worktree("live");
    let gh = gh_stub(&HashMap::from([(
        "live",
        serde_json::json!([pull(1, &repo.head_sha(), "MERGED")]),
    )]));
    let mut process = Command::new("sleep")
        .arg("60")
        .current_dir(&path)
        .spawn()
        .unwrap();
    let run = run_lf(&repo, &gh, &["wt", "prune"]);
    let retained = path.exists();
    process.kill().unwrap();
    process.wait().unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(retained);
}
