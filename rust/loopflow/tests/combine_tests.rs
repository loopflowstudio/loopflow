mod support;

use std::process::Command;

use loopflow::ops::{combine_prs, CombineOptions, NullProgress};
use loopflow_test_support::TestRepo;
use support::EnvGuard;

fn write_gh_combine_script(pr_list_json: &str) -> String {
    format!(
        "#!/bin/sh\ncase \"$1 $2\" in\n  'pr list')\n    cat <<'JSON'\n{pr_list_json}\nJSON\n    exit 0;;\n  'pr create')\n    echo 'https://example.com/pr/99'\n    exit 0;;\n  'pr close')\n    exit 0;;\nesac\nexit 0\n"
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
fn combine_combines_open_prs_and_returns_new_pr() {
    let gh_script = write_gh_combine_script(
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

    let result = combine_prs(
        repo.path(),
        &CombineOptions {
            wave_name: Some("wave".to_string()),
        },
        &NullProgress,
    )
    .expect("combine should succeed");

    assert_eq!(
        result.new_pr_url.as_deref(),
        Some("https://example.com/pr/99")
    );
    assert_eq!(result.closed_prs, vec![11, 12]);

    let current_branch = git_output(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert_eq!(current_branch, "wave-combined");

    let log = git_output(
        &repo,
        &["log", "--oneline", "origin/main..HEAD", "--format=%s"],
    );
    assert!(log.contains("commit a"));
    assert!(log.contains("commit b"));
}

#[test]
fn combine_requires_at_least_two_open_prs() {
    let gh_script = write_gh_combine_script(
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

    let err = combine_prs(
        repo.path(),
        &CombineOptions {
            wave_name: Some("wave".to_string()),
        },
        &NullProgress,
    )
    .expect_err("combine should fail");

    assert!(
        err.to_string().contains("need at least 2 open PRs"),
        "unexpected error: {err}"
    );
}
