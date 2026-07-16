mod support;

use std::process::Command;

use loopflow_test_support::TestRepo;
use support::EnvGuard;

/// Fake `gh` that fails `pr checks`, returns two failed checks with PR 978
/// (job URL) and PR 983 (run URL) shapes, and records every `run view`
/// invocation. `run view` with a URL arg exits 404; with a numeric run id it
/// prints log lines. The record file is the proof artifact: it should contain
/// only numeric run ids, never full URLs.
fn gh_script_for_url_shapes(record: &std::path::Path) -> String {
    format!(
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo 'gh version 2.40.0'
  exit 0
fi
case "$1 $2" in
  'pr checks')
    exit 1
    ;;
  'pr view')
    printf 'build\thttps://github.com/loopflowstudio/loopflow/actions/runs/978123456/jobs/111222333\n'
    printf 'test\thttps://github.com/loopflowstudio/loopflow/actions/runs/983654321\n'
    exit 0
    ;;
  'run view')
    echo "$@" >> '{record}'
    for arg in "$@"; do
      case "$arg" in
        */*)
          echo "HTTP 404: Not Found" >&2
          exit 1
          ;;
      esac
    done
    echo "log line 1 for run $3"
    echo "log line 2 for run $3"
    exit 0
    ;;
esac
echo "unexpected gh invocation: $@" >&2
exit 1
"#,
        record = record.display()
    )
}

/// Fake `gh` whose `run view --log` always fails, simulating missing, expired,
/// or private logs.
fn gh_script_for_failing_logs() -> &'static str {
    r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo 'gh version 2.40.0'
  exit 0
fi
case "$1 $2" in
  'pr checks')
    exit 1
    ;;
  'pr view')
    printf 'build\thttps://github.com/loopflowstudio/loopflow/actions/runs/978123456/jobs/111222333\n'
    exit 0
    ;;
  'run view')
    echo "could not find workflow run" >&2
    exit 1
    ;;
esac
echo "unexpected gh invocation: $@" >&2
exit 1
"#
}

/// Fake `gh` that returns a non-Actions (external CI) `detailsUrl`. The code
/// should skip `gh run view` and print an actionable message instead.
fn gh_script_for_external_ci() -> &'static str {
    r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo 'gh version 2.40.0'
  exit 0
fi
case "$1 $2" in
  'pr checks')
    exit 1
    ;;
  'pr view')
    printf 'external-ci\thttps://ci.example.com/build/12345\n'
    exit 0
    ;;
esac
echo "unexpected gh invocation: $@" >&2
exit 1
"#
}

/// Fake `gh` that returns a bare numeric run id as the `detailsUrl`, covering
/// the numeric-id variant. Records `run view` invocations.
fn gh_script_for_numeric_id(record: &std::path::Path) -> String {
    format!(
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo 'gh version 2.40.0'
  exit 0
fi
case "$1 $2" in
  'pr checks')
    exit 1
    ;;
  'pr view')
    printf 'lint\t555666777\n'
    exit 0
    ;;
  'run view')
    echo "$@" >> '{record}'
    for arg in "$@"; do
      case "$arg" in
        */*)
          echo "HTTP 404: Not Found" >&2
          exit 1
          ;;
      esac
    done
    echo "log line 1 for run $3"
    echo "log line 2 for run $3"
    exit 0
    ;;
esac
echo "unexpected gh invocation: $@" >&2
exit 1
"#,
        record = record.display()
    )
}

fn run_wt_ci_logs(repo: &TestRepo) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["wt", "ci", "--logs"])
        .current_dir(repo.path())
        .env("RUST_LOG", "off")
        .output()
        .expect("run lf wt ci --logs")
}

/// Reproduces the PR 978 (job URL) and PR 983 (run URL) shapes that caused
/// `gh run view <full-url>` to produce malformed API paths and HTTP 404.
/// Proves the fix extracts numeric run/job ids and never passes a URL.
#[test]
fn wt_ci_logs_extracts_run_ids_from_pr_shaped_urls() {
    let record_dir = tempfile::TempDir::new().expect("record dir");
    let record = record_dir.path().join("run_view_calls.txt");
    let script = gh_script_for_url_shapes(&record);
    let _env = EnvGuard::new(&[("gh", script.as_str())]);
    let repo = TestRepo::new();

    let output = run_wt_ci_logs(&repo);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "ci checks should fail");

    // Attribution headers for both failed checks.
    assert!(
        stdout.contains("### build"),
        "stdout should attribute build: {stdout}"
    );
    assert!(
        stdout.contains("### test"),
        "stdout should attribute test: {stdout}"
    );

    // Log content — proves gh run view was called with numeric run ids, not
    // full URLs (the fake gh returns 404 for any arg containing "/").
    assert!(
        stdout.contains("log line 1 for run 978123456"),
        "should print logs for PR 978 shaped job URL: {stdout}"
    );
    assert!(
        stdout.contains("log line 2 for run 983654321"),
        "should print logs for PR 983 shaped run URL: {stdout}"
    );

    // No malformed API requests leaked to stderr.
    assert!(
        !stderr.contains("404") && !stderr.contains("Not Found"),
        "no malformed API errors in stderr: {stderr}"
    );

    // The record file is the direct proof: gh run view received numeric run
    // ids with --log and --job, never full URLs.
    let calls = std::fs::read_to_string(&record).expect("record file");
    assert!(
        calls.contains("run view 978123456 --log --job 111222333"),
        "PR 978 job URL → numeric run id + job id: {calls}"
    );
    assert!(
        calls.contains("run view 983654321 --log"),
        "PR 983 run URL → numeric run id: {calls}"
    );
    assert!(
        !calls.contains("http") && !calls.contains("/actions/runs/"),
        "no full URLs passed to gh run view: {calls}"
    );
}

/// Missing, expired, or private logs produce an actionable error with the run
/// id and a hint — not a silent `let _ =` drop.
#[test]
fn wt_ci_logs_reports_unavailable_logs_actionably() {
    let script = gh_script_for_failing_logs();
    let _env = EnvGuard::new(&[("gh", script)]);
    let repo = TestRepo::new();

    let output = run_wt_ci_logs(&repo);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "ci checks should fail");
    assert!(
        stderr.contains("Couldn't fetch logs"),
        "should report actionable error: {stderr}"
    );
    assert!(
        stderr.contains("missing, expired"),
        "should hint at possible causes: {stderr}"
    );
    assert!(
        stderr.contains("978123456"),
        "should include the run id so the user can look it up: {stderr}"
    );
}

/// Non-Actions `detailsUrl` (external CI) is skipped with an actionable
/// message and the original URL — not passed to `gh run view` as garbage.
#[test]
fn wt_ci_logs_skips_non_actions_urls() {
    let script = gh_script_for_external_ci();
    let _env = EnvGuard::new(&[("gh", script)]);
    let repo = TestRepo::new();

    let output = run_wt_ci_logs(&repo);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "ci checks should fail");
    assert!(
        stderr.contains("### external-ci"),
        "should attribute the external check: {stderr}"
    );
    assert!(
        stderr.contains("Logs not available via gh"),
        "should explain why logs aren't fetched: {stderr}"
    );
    assert!(
        stderr.contains("ci.example.com"),
        "should include the URL for manual inspection: {stderr}"
    );
}

/// A bare numeric run id in `detailsUrl` is passed through correctly — the
/// numeric-id variant.
#[test]
fn wt_ci_logs_handles_numeric_id_details_url() {
    let record_dir = tempfile::TempDir::new().expect("record dir");
    let record = record_dir.path().join("run_view_calls.txt");
    let script = gh_script_for_numeric_id(&record);
    let _env = EnvGuard::new(&[("gh", script.as_str())]);
    let repo = TestRepo::new();

    let output = run_wt_ci_logs(&repo);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(!output.status.success(), "ci checks should fail");
    assert!(
        stdout.contains("log line 1 for run 555666777"),
        "should print logs for numeric id detailsUrl: {stdout}"
    );
    let calls = std::fs::read_to_string(&record).expect("record file");
    assert!(
        calls.contains("run view 555666777 --log"),
        "numeric id should be passed through as-is: {calls}"
    );
}
