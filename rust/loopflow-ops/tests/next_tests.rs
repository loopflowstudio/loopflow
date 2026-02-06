mod support;

use std::process::Command;

use loopflow_ops::{next_branch, NextOptions, NullProgress};
use loopflow_test_support::TestRepo;
use support::EnvGuard;

const MOCK_CLAUDE: &str = "#!/bin/sh\necho '{\"title\":\"test commit\",\"body\":\"\"}'\n";

fn write_gh_script(pr_number: Option<u64>, pr_state: Option<&str>) -> String {
    let number = pr_number.map(|n| n.to_string()).unwrap_or_default();
    let state = pr_state.unwrap_or("");
    format!(
        "#!/bin/sh\ncase \"$1 $2\" in\n  'pr list') echo '[]'; exit 0;;\n  'pr create') echo 'https://example.com/pr/1'; exit 0;;\n  'pr view')\n    if echo \"$@\" | grep -q 'number'; then\n      echo '{number}'; exit 0;\n    fi\n    if echo \"$@\" | grep -q 'state'; then\n      echo '{state}'; exit 0;\n    fi\n    if echo \"$@\" | grep -q 'url'; then\n      echo 'https://example.com/pr/1'; exit 0;\n    fi\n    echo ''; exit 0;;\n  'pr edit') exit 0;;\n  'pr merge') exit 0;;\nesac\nexit 0\n"
    )
}

fn branch_exists(repo: &TestRepo, name: &str) -> bool {
    Command::new("git")
        .args(["show-ref", "--verify", &format!("refs/heads/{name}")])
        .current_dir(repo.path())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[test]
fn next_creates_branch_from_current() {
    let gh_script = write_gh_script(None, None);
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);
    let repo = TestRepo::new();
    repo.create_branch("feature");

    let result = next_branch(
        repo.path(),
        &NextOptions {
            block: false,
            create_pr: false,
            rebase: false,
            wave_name: None,
        },
        &NullProgress,
    )
    .expect("next");

    assert!(branch_exists(&repo, &result.new_branch));
    assert!(branch_exists(&repo, "feature"));
}

#[test]
fn next_with_naming_schema() {
    let gh_script = write_gh_script(None, None);
    let _env = EnvGuard::new(&[("gh", gh_script.as_str()), ("claude", MOCK_CLAUDE)]);
    let repo = TestRepo::new();
    repo.create_file(
        ".lf/config.yaml",
        "branch_names:\n  schema: '{user}/{words}'\n",
    );
    repo.create_branch("feature");

    let result = next_branch(
        repo.path(),
        &NextOptions {
            block: false,
            create_pr: false,
            rebase: false,
            wave_name: Some("wave".to_string()),
        },
        &NullProgress,
    )
    .expect("next");

    assert!(result.new_branch.starts_with("jack/"));
}

#[test]
fn next_detects_merged_pr_starts_fresh() {
    let gh_script = write_gh_script(Some(12), Some("MERGED"));
    let _env = EnvGuard::new(&[("gh", gh_script.as_str()), ("claude", MOCK_CLAUDE)]);
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");

    repo.checkout("main");
    let main_head = repo.head_sha();
    repo.checkout("feature");

    let result = next_branch(
        repo.path(),
        &NextOptions {
            block: false,
            create_pr: false,
            rebase: false,
            wave_name: None,
        },
        &NullProgress,
    )
    .expect("next");

    assert!(branch_exists(&repo, &result.new_branch));
    assert_eq!(repo.head_sha(), main_head);
}

#[test]
fn next_appends_suffix_on_branch_name_collision() {
    let gh_script = write_gh_script(None, None);
    let _env = EnvGuard::new(&[("gh", gh_script.as_str()), ("claude", MOCK_CLAUDE)]);
    let repo = TestRepo::new();
    // Schema {name} produces "wave" — but that branch already exists, triggering collision
    repo.create_file(".lf/config.yaml", "branch_names:\n  schema: '{name}'\n");
    repo.create_branch("wave");

    let result = next_branch(
        repo.path(),
        &NextOptions {
            block: false,
            create_pr: false,
            rebase: false,
            wave_name: Some("wave".to_string()),
        },
        &NullProgress,
    )
    .expect("next should succeed despite name collision");

    assert!(result.new_branch.starts_with("wave."));
    assert!(result.new_branch.len() > "wave.".len());
    assert!(branch_exists(&repo, &result.new_branch));
    assert!(branch_exists(&repo, "wave"));
}
