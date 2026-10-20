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

fn prune(repo: &TestRepo, gh: &str, dry_run: bool) -> Output {
    let home = tempfile::tempdir().unwrap();
    let bin = home.path().join("bin");
    fs::create_dir(&bin).unwrap();
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
        .env("HOME", home.path())
        .env("LF_HOME", home.path().join(".lf"))
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("PATH", path)
        .current_dir(repo.path())
        .args(["wt", "prune", "--branches"]);
    if dry_run {
        command.arg("--dry-run");
    }
    command.output().expect("run lf")
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
