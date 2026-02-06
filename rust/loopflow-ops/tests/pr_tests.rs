mod support;

use std::process::Command;

use loopflow_ops::{create_or_update_pr, NullProgress, PrOptions};
use loopflow_test_support::TestRepo;
use support::EnvGuard;

fn write_gh_script(pr_list: &str, pr_diff: Option<&str>) -> String {
    let diff = pr_diff.unwrap_or("");
    format!(
        "#!/bin/sh\ncase \"$1 $2\" in\n  'pr list')\n    cat <<'JSON'\n{pr_list}\nJSON\n    exit 0;;\n  'pr diff') echo '{diff}'; exit 0;;\n  'pr create') echo 'https://example.com/pr/1'; exit 0;;\n  'pr edit') exit 0;;\n  'pr ready') exit 0;;\n  'pr view') echo 'OPEN'; exit 0;;\nesac\nexit 0\n"
    )
}

fn write_claude_script() -> String {
    r#"#!/bin/sh
echo '{"title":"test title","body":"test body"}'
exit 0
"#
    .to_string()
}

fn push_branch(repo: &TestRepo, name: &str) {
    let _ = Command::new("git")
        .args(["push", "-u", "origin", name])
        .current_dir(repo.path())
        .status();
}

#[test]
fn pr_create_calls_gh() {
    let gh_script = write_gh_script("[]", None);
    let claude_script = write_claude_script();
    let _env = EnvGuard::new(&[
        ("gh", gh_script.as_str()),
        ("claude", claude_script.as_str()),
    ]);
    let repo = TestRepo::new();
    repo.create_branch("feature");
    push_branch(&repo, "feature");

    let result = create_or_update_pr(
        repo.path(),
        &PrOptions {
            refresh: false,
            lint: false,
        },
        &NullProgress,
    )
    .expect("pr");

    assert!(result.created);
    assert_eq!(result.url, "https://example.com/pr/1");
}

#[test]
fn pr_update_refreshes_body() {
    let gh_script = write_gh_script(
        r#"[{"url":"https://example.com/pr/1","state":"OPEN","isDraft":false,"number":1}]"#,
        Some("diff"),
    );
    let claude_script = write_claude_script();
    let _env = EnvGuard::new(&[
        ("gh", gh_script.as_str()),
        ("claude", claude_script.as_str()),
    ]);
    let repo = TestRepo::new();
    repo.create_branch("feature");
    push_branch(&repo, "feature");

    let result = create_or_update_pr(
        repo.path(),
        &PrOptions {
            refresh: true,
            lint: false,
        },
        &NullProgress,
    )
    .expect("pr");

    assert!(result.updated);
    assert!(!result.created);
}

#[test]
fn pr_skips_when_no_diff() {
    let gh_script = write_gh_script(
        r#"[{"url":"https://example.com/pr/1","state":"OPEN","isDraft":false,"number":1}]"#,
        None,
    );
    let claude_script = write_claude_script();
    let _env = EnvGuard::new(&[
        ("gh", gh_script.as_str()),
        ("claude", claude_script.as_str()),
    ]);
    let repo = TestRepo::new();
    repo.create_branch("feature");
    push_branch(&repo, "feature");

    let result = create_or_update_pr(
        repo.path(),
        &PrOptions {
            refresh: false,
            lint: false,
        },
        &NullProgress,
    )
    .expect("pr");

    assert!(!result.created);
    assert!(!result.updated);
    assert_eq!(result.url, "https://example.com/pr/1");
}
