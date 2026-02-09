mod support;

use std::process::Command;

use loopflow::ops::{absorb_into_pr, collapse_prs, AbsorbOptions, CollapseOptions, NullProgress};
use loopflow_test_support::TestRepo;
use support::EnvGuard;

fn write_gh_collapse_script(pr_list_json: &str) -> String {
    format!(
        "#!/bin/sh\ncase \"$1 $2\" in\n  'pr list')\n    cat <<'JSON'\n{pr_list_json}\nJSON\n    exit 0;;\n  'pr create')\n    echo 'https://example.com/pr/99'\n    exit 0;;\n  'pr close')\n    exit 0;;\nesac\nexit 0\n"
    )
}

fn write_gh_absorb_script(target_branch: &str) -> String {
    format!(
        "#!/bin/sh\ncase \"$1 $2\" in\n  'pr view')\n    echo '{target_branch}'\n    exit 0;;\nesac\nexit 0\n"
    )
}

fn git_output(repo: &TestRepo, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo.path())
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn collapse_combines_open_prs_and_returns_new_pr() {
    let gh_script = write_gh_collapse_script(
        r#"[
  {"number":11,"headRefName":"wave-a","url":"https://example.com/pr/11"},
  {"number":12,"headRefName":"wave-b","url":"https://example.com/pr/12"}
]"#,
    );
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);

    let repo = TestRepo::new();

    repo.create_branch("wave-a");
    repo.create_file("a.txt", "A\n");
    repo.stage_all();
    repo.commit("commit a");
    repo.push_new_branch("wave-a");

    repo.create_branch("wave-b");
    repo.create_file("b.txt", "B\n");
    repo.stage_all();
    repo.commit("commit b");
    repo.push_new_branch("wave-b");

    let result = collapse_prs(
        repo.path(),
        &CollapseOptions {
            wave_name: Some("wave".to_string()),
        },
        &NullProgress,
    )
    .expect("collapse should succeed");

    assert_eq!(
        result.new_pr_url.as_deref(),
        Some("https://example.com/pr/99")
    );
    assert_eq!(result.closed_prs, vec![11, 12]);

    let current_branch = git_output(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert_eq!(current_branch, "wave-collapsed");

    let log = git_output(
        &repo,
        &["log", "--oneline", "origin/main..HEAD", "--format=%s"],
    );
    assert!(log.contains("commit a"));
    assert!(log.contains("commit b"));
}

#[test]
fn collapse_requires_at_least_two_open_prs() {
    let gh_script = write_gh_collapse_script(
        r#"[
  {"number":11,"headRefName":"wave-a","url":"https://example.com/pr/11"}
]"#,
    );
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);

    let repo = TestRepo::new();
    repo.create_branch("wave-a");
    repo.create_file("a.txt", "A\n");
    repo.stage_all();
    repo.commit("commit a");
    repo.push_new_branch("wave-a");

    let err = collapse_prs(
        repo.path(),
        &CollapseOptions {
            wave_name: Some("wave".to_string()),
        },
        &NullProgress,
    )
    .expect_err("collapse should fail");

    assert!(
        err.to_string().contains("need at least 2 open PRs"),
        "unexpected error: {err}"
    );
}

#[test]
fn absorb_cherry_picks_current_commits_into_target_pr_branch() {
    let gh_script = write_gh_absorb_script("target-pr-branch");
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);

    let repo = TestRepo::new();

    repo.create_branch("target-pr-branch");
    repo.create_file("target.txt", "target\n");
    repo.stage_all();
    repo.commit("target base commit");
    repo.push_new_branch("target-pr-branch");

    repo.create_branch("current-work");
    repo.create_file("current.txt", "current\n");
    repo.stage_all();
    repo.commit("current work commit");

    let result = absorb_into_pr(
        repo.path(),
        &AbsorbOptions {
            target_pr_number: 42,
        },
        &NullProgress,
    )
    .expect("absorb should succeed");

    assert_eq!(result.target_branch, "target-pr-branch");
    assert_eq!(result.commits_absorbed, 1);

    let current_branch = git_output(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert_eq!(current_branch, "target-pr-branch");

    let last_commit = git_output(&repo, &["log", "-1", "--format=%s"]);
    assert_eq!(last_commit, "current work commit");
}
