use crate::support;

use std::process::{Command, Output, Stdio};

use loopflow_test_support::TestRepo;
use support::EnvGuard;

fn lf(repo: &TestRepo, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(args)
        .current_dir(repo.path())
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

#[test]
fn teammate_handoff_updates_existing_pr_and_preserves_review_revision() {
    let repo = TestRepo::new();
    let state = tempfile::tempdir().unwrap();
    let script = format!(
        r#"#!/bin/sh
set -eu
case "$1 $2" in
  'pr edit')
    test "$3" = 42 || exit 2
    shift 3
    while [ "$#" -gt 0 ]; do
      test "$1" = --add-reviewer || exit 2
      printf '%s\n' "$2" >> '{state}/requests'
      shift 2
    done ;;
  'pr view')
    test "$3" = 42 || exit 2
    if [ -f '{state}/requests' ]; then
      requests='[{{"login":"alice"}},{{"name":"platform"}}]'
    else
      requests='[]'
    fi
    printf '{{"number":42,"url":"https://example.com/pr/42","headRefOid":"new-head","isDraft":false,"reviewDecision":"REVIEW_REQUIRED","reviewRequests":%s,"reviews":[{{"author":{{"login":"alice"}},"state":"APPROVED","commit":{{"oid":"older-head"}},"body":"Earlier revision"}}]}}\n' "$requests" ;;
  *) echo 'unexpected mutation' >&2; exit 3 ;;
esac
"#,
        state = state.path().display()
    );
    let _env = EnvGuard::new(&[("gh", &script)]);
    let head = repo.head_sha();
    let result = lf(
        &repo,
        &[
            "pr",
            "request-review",
            "42",
            "-r",
            "alice",
            "-r",
            "org/platform",
        ],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(state.path().join("requests")).unwrap(),
        "alice\norg/platform\n"
    );
    let result = lf(&repo, &["pr", "reviews", "42", "--json"]);
    assert!(result.status.success());
    let read: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(read["reviewRequests"].as_array().unwrap().len(), 2);
    assert_eq!(read["reviewDecision"], "REVIEW_REQUIRED");
    assert_eq!(read["headRefOid"], "new-head");
    assert_eq!(read["reviews"][0]["commit"]["oid"], "older-head");
    assert_eq!(repo.head_sha(), head, "reviewing never creates a commit");
}

#[test]
fn reviewer_permission_failure_is_not_reported_as_a_handoff() {
    let _env = EnvGuard::new(&[(
        "gh",
        "#!/bin/sh\necho 'reviewer cannot access this repository' >&2\nexit 4\n",
    )]);
    let repo = TestRepo::new();
    let output = lf(&repo, &["pr", "request-review", "-r", "outsider"]);
    assert_eq!(output.status.code(), Some(4));
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot access"));
}

#[test]
fn required_checks_preserve_pending_and_do_not_fetch_failure_logs() {
    let _env = EnvGuard::new(&[(
        "gh",
        r#"#!/bin/sh
case "$*" in
  'pr checks main --required') echo 'build pending'; exit 8 ;;
  *) echo 'unexpected logs or missing required filter' >&2; exit 3 ;;
esac
"#,
    )]);
    let repo = TestRepo::new();
    let output = lf(&repo, &["pr", "checks", "--required", "--logs"]);
    assert_eq!(output.status.code(), Some(8));
    assert!(String::from_utf8_lossy(&output.stdout).contains("build pending"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("pending"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("unexpected"));
}
