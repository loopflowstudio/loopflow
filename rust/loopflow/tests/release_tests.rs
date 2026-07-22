mod support;

use std::fs;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use loopflow::ops::{
    bump_version, generate_release, release_bump, release_check, release_notes, release_run,
    release_status, release_tag, NullProgress, ReleaseNotesDegradation, ReleaseNotesStatus,
    ReleaseRunOutcome,
};
use loopflow_test_support::TestRepo;
use support::EnvGuard;

fn write_gh_script(pr_list: &str) -> String {
    format!(
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo 'gh version 2.0.0'\n  exit 0\nfi\ncase \"$1 $2\" in\n  'pr list')\n    cat <<'JSON'\n{pr_list}\nJSON\n    exit 0;;\nesac\necho \"unexpected gh invocation: $@\" >&2\nexit 1\n"
    )
}

fn write_gh_status_script(run_list: &str, release_view: &str) -> String {
    format!(
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo 'gh version 2.0.0'\n  exit 0\nfi\ncase \"$1 $2\" in\n  'run list')\n    case \"$*\" in *databaseId*) ;; *) echo 'databaseId was not requested' >&2; exit 1;; esac\n    cat <<'JSON'\n{run_list}\nJSON\n    exit 0;;\n  'release view')\n    cat <<'JSON'\n{release_view}\nJSON\n    exit 0;;\n  'pr list') echo '[]'; exit 0;;\nesac\necho \"unexpected gh invocation: $@\" >&2\nexit 1\n"
    )
}

fn write_gh_incomplete_release_script() -> &'static str {
    "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo 'gh version 2.0.0'\n  exit 0\nfi\ncase \"$1 $2\" in\n  'release view') exit 1;;\n  'run list') echo '[]'; exit 0;;\nesac\necho \"unexpected gh invocation: $@\" >&2\nexit 1\n"
}

fn write_gh_failed_release_script() -> &'static str {
    "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo 'gh version 2.0.0'\n  exit 0\nfi\ncase \"$1 $2\" in\n  'release view') exit 1;;\n  'run list') cat <<'JSON'\n[{\"databaseId\":42,\"headBranch\":\"v0.9.1\",\"status\":\"completed\",\"conclusion\":\"failure\",\"url\":\"https://example.com/run/42\"}]\nJSON\n    exit 0;;\n  'pr list') echo '[]'; exit 0;;\nesac\necho \"unexpected gh invocation: $@\" >&2\nexit 1\n"
}

fn git(repo: &TestRepo, args: &[&str]) {
    let _ = git_output(repo, args);
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

fn git_output_bare(repo: &TestRepo, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("--git-dir")
        .arg(repo.bare_path())
        .args(args)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git --git-dir {:?} {:?} failed: {}",
        repo.bare_path(),
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn wait_for_path(path: &std::path::Path) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {}",
            path.display()
        );
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn release_generates_notes_and_writes_file() {
    let repo = TestRepo::new();
    git(&repo, &["tag", "v0.8.0"]);
    fs::write(repo.path().join("feature.txt"), "released\n").unwrap();
    git(&repo, &["add", "feature.txt"]);
    git(&repo, &["commit", "-m", "Add release command"]);
    let sha = git_output(&repo, &["rev-parse", "HEAD"]);
    let pr_list = format!(
        r#"[{{"number":101,"title":"Add release command","body":"Users can now run lf release.","mergeCommit":{{"oid":"{sha}"}}}}]"#
    );
    let gh_script = write_gh_script(&pr_list);
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);

    let version =
        generate_release(repo.path(), "v0.9.1", &NullProgress).expect("release should succeed");

    assert_eq!(version, "0.9.1");

    let notes = fs::read_to_string(repo.path().join("RELEASE_NOTES.md")).expect("read notes");
    assert!(notes.starts_with("# v0.9.1\n\n"));
    assert!(notes.contains("## Merged PRs"));
    assert!(notes.contains("- #101 Add release command"));
}

#[test]
fn release_keeps_existing_header() {
    let gh_script = write_gh_script("[]");
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);

    let repo = TestRepo::new();
    git(&repo, &["tag", "v0.9.1"]);

    generate_release(repo.path(), "0.9.2", &NullProgress).expect("release should succeed");

    let notes = fs::read_to_string(repo.path().join("RELEASE_NOTES.md")).expect("read notes");
    assert_eq!(notes.lines().next().expect("first line"), "# v0.9.2");
    assert_eq!(notes.matches("# v0.9.2").count(), 1);
}

#[test]
fn release_with_bump_keyword() {
    let gh_script = write_gh_script("[]");
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);

    let repo = TestRepo::new();
    git(&repo, &["tag", "v0.9.1"]);

    let version =
        generate_release(repo.path(), "patch", &NullProgress).expect("release should succeed");
    assert_eq!(version, "0.9.2");

    let notes = fs::read_to_string(repo.path().join("RELEASE_NOTES.md")).expect("read notes");
    assert!(notes.starts_with("# v0.9.2\n\n"));
}

#[test]
fn release_notes_falls_back_when_agent_cli_is_missing() {
    let repo = TestRepo::new();
    git(&repo, &["tag", "v0.9.0"]);
    fs::write(repo.path().join("weekly.txt"), "self-contained\n").unwrap();
    git(&repo, &["add", "weekly.txt"]);
    git(
        &repo,
        &["commit", "-m", "Make weekly release self-contained"],
    );
    let sha = git_output(&repo, &["rev-parse", "HEAD"]);
    let pr_list = format!(
        r#"[{{"number":101,"title":"Make weekly release self-contained","body":"Release automation no longer requires a workstation CLI.","additions":42,"deletions":7,"changedFiles":3,"mergeCommit":{{"oid":"{sha}"}}}}]"#
    );
    let gh_script = write_gh_script(&pr_list);
    let lf_script =
        "#!/bin/sh\necho \"'claude' CLI not found. Install it and rerun \\`lf init\\`.\" >&2\nexit 1\n";
    let _env = EnvGuard::new(&[("gh", gh_script.as_str()), ("lf", lf_script)]);
    // A staged unreleased artifact still gets promoted; it is no longer a
    // decisions ledger and no longer injected into the notes.
    fs::create_dir_all(repo.path().join("release/unreleased")).unwrap();
    fs::write(
        repo.path().join("release/unreleased/CHANGES.md"),
        "staged artifact\n",
    )
    .unwrap();

    let notes = release_notes(repo.path(), "0.9.1", Some("v0.9.0"), None, &NullProgress)
        .expect("release notes should fall back");

    assert!(notes.starts_with("# v0.9.1\n\n"));
    assert!(notes.contains("loopflow:release-notes=degraded;reason=missing-cli;gate=safe"));
    assert!(notes.contains("_Generated mechanically for v0.9.1._"));
    // Notes synthesize from the merged PRs, not a central ledger.
    assert!(notes.contains("Make weekly release self-contained"));
    assert!(!notes.contains("## Release decisions"));
    assert_eq!(
        fs::read_to_string(repo.path().join("release/v0.9.1/NOTES.md")).unwrap(),
        notes,
    );
    // The unreleased dir was promoted to the version dir.
    assert!(!repo.path().join("release/unreleased").exists());
    assert!(repo.path().join("release/v0.9.1/CHANGES.md").exists());
}

#[test]
fn release_notes_falls_back_for_every_provider_degradation_class() {
    let gh_script = write_gh_script("[]");
    let lf_script = "#!/bin/sh\ncat .provider-failure >&2\nexit 1\n";
    let _env = EnvGuard::new(&[("gh", gh_script.as_str()), ("lf", lf_script)]);
    let cases = [
        (
            "failed to select provider account: no eligible managed codex account: 'primary'",
            ReleaseNotesDegradation::Cooldown,
        ),
        (
            "agent stopped after account subscription limit",
            ReleaseNotesDegradation::Quota,
        ),
        (
            "agent stopped after account credential invalidated",
            ReleaseNotesDegradation::Authentication,
        ),
        (
            "agent stopped after provider rate limit",
            ReleaseNotesDegradation::RateLimit,
        ),
        (
            "agent stopped after provider unavailable",
            ReleaseNotesDegradation::ProviderUnavailable,
        ),
        (
            "agent stopped after provider capacity",
            ReleaseNotesDegradation::ProviderUnavailable,
        ),
        (
            "agent stopped after provider transport",
            ReleaseNotesDegradation::ProviderUnavailable,
        ),
    ];

    for (failure, degradation) in cases {
        let repo = TestRepo::new();
        git(&repo, &["tag", "v0.9.0"]);
        fs::write(repo.path().join(".provider-failure"), failure).unwrap();

        let notes = release_notes(repo.path(), "0.9.1", Some("v0.9.0"), None, &NullProgress)
            .expect("provider degradation should select deterministic notes");

        assert!(notes.contains(&format!(
            "loopflow:release-notes=degraded;reason={degradation};gate=safe"
        )));
        assert!(notes.contains("_Generated mechanically for v0.9.1._"));
        assert!(notes.len() < 60 * 1024);
    }
}

#[test]
fn release_notes_bounds_oversized_pr_bodies_and_queue_metadata() {
    let repo = TestRepo::new();
    git(&repo, &["tag", "v0.9.0"]);
    fs::write(
        repo.path().join("RELEASE_NOTES.md"),
        "previous voice ".repeat(4_000),
    )
    .unwrap();
    fs::create_dir_all(repo.path().join("release/unreleased")).unwrap();
    fs::write(
        repo.path().join("release/unreleased/DECISIONS.md"),
        "bounded decision ".repeat(4_000),
    )
    .unwrap();
    fs::write(repo.path().join("bounded.txt"), "bounded\n").unwrap();
    git(&repo, &["add", "bounded.txt"]);
    git(&repo, &["commit", "-m", "Bound release context"]);
    let sha = git_output(&repo, &["rev-parse", "HEAD"]);
    let pr_list = serde_json::to_string(&vec![serde_json::json!({
        "number": 202,
        "title": "Bound release context",
        "body": "x".repeat(100_000),
        "additions": 8,
        "deletions": 2,
        "changedFiles": 1,
        "mergeCommit": {"oid": sha},
    })])
    .unwrap();
    let gh_script = write_gh_script(&pr_list);
    let lf_script = "#!/bin/sh\ncp \"$LF_RELEASE_NOTES_CONTEXT\" RELEASE_CONTEXT.json\nprintf '# v0.9.1\\n\\nBounded narrative notes.\\n' > RELEASE_NOTES.md\n";
    let _env = EnvGuard::new(&[("gh", gh_script.as_str()), ("lf", lf_script)]);

    let notes = release_notes(repo.path(), "0.9.1", Some("v0.9.0"), None, &NullProgress)
        .expect("bounded narrative notes should succeed");

    let context_path = repo.path().join("RELEASE_CONTEXT.json");
    assert!(fs::metadata(&context_path).unwrap().len() <= 128 * 1024);
    let context: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(context_path).unwrap()).unwrap();
    assert!(context["merged_prs"][0]["body"].as_str().unwrap().len() <= 4 * 1024);
    assert!(context["omissions"]["text_bytes"].as_u64().unwrap() >= 95_000);
    assert!(context["decisions"].as_str().unwrap().len() <= 16 * 1024);
    assert!(context["previous_release_notes"].as_str().unwrap().len() <= 16 * 1024);
    assert!(context["omissions"]["decisions_bytes"].as_u64().unwrap() > 0);
    assert!(
        context["omissions"]["previous_release_notes_bytes"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(notes.len() < 60 * 1024);
    assert!(!notes.contains(&"x".repeat(1_000)));
}

#[test]
fn interrupted_release_notes_retry_replaces_partial_state_once() {
    let gh_script = write_gh_script("[]");
    let lf_script = r#"#!/bin/sh
attempt=1
if [ -f .notes-attempt ]; then
  attempt=2
fi
printf '%s' "$attempt" > .notes-attempt
cp "$LF_RELEASE_NOTES_CONTEXT" "RELEASE_CONTEXT.$attempt.json"
if [ "$attempt" = "1" ]; then
  printf '# v0.9.1\n\nPartial notes that must not ship.\n' > RELEASE_NOTES.md
  echo 'operator interrupted release-notes generation' >&2
  exit 130
fi
printf '# v0.9.1\n\nConcise resumed release notes.\n' > RELEASE_NOTES.md
"#;
    let _env = EnvGuard::new(&[("gh", gh_script.as_str()), ("lf", lf_script)]);
    let repo = TestRepo::new();
    git(&repo, &["tag", "v0.9.0"]);
    let previous_notes = "# v0.9.0\n\nPrevious safe notes.\n";
    fs::write(repo.path().join("RELEASE_NOTES.md"), previous_notes).unwrap();

    let error = release_notes(repo.path(), "0.9.1", Some("v0.9.0"), None, &NullProgress)
        .expect_err("an unclassified interruption must keep the gate red");
    assert!(error.to_string().contains("release gate blocked"));
    assert_eq!(
        fs::read_to_string(repo.path().join("RELEASE_NOTES.md")).unwrap(),
        previous_notes
    );

    let notes = release_notes(repo.path(), "0.9.1", Some("v0.9.0"), None, &NullProgress)
        .expect("retry should replace partial state");

    assert!(
        fs::metadata(repo.path().join("RELEASE_CONTEXT.1.json"))
            .unwrap()
            .len()
            <= 128 * 1024
    );
    assert!(
        fs::metadata(repo.path().join("RELEASE_CONTEXT.2.json"))
            .unwrap()
            .len()
            <= 128 * 1024
    );
    assert!(notes.contains("Concise resumed release notes."));
    assert!(!notes.contains("Partial notes that must not ship."));
    assert_eq!(notes.matches("loopflow:release-notes=").count(), 1);
    let archived = fs::read_to_string(repo.path().join("release/v0.9.1/NOTES.md")).unwrap();
    assert_eq!(archived, notes);
    assert_eq!(
        fs::read_dir(repo.path().join("release/v0.9.1"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name() == "NOTES.md")
            .count(),
        1
    );
}

#[test]
fn successful_agent_without_fresh_notes_keeps_release_gate_unsafe() {
    let gh_script = write_gh_script("[]");
    let lf_script = "#!/bin/sh\nexit 0\n";
    let _env = EnvGuard::new(&[("gh", gh_script.as_str()), ("lf", lf_script)]);
    let repo = TestRepo::new();
    git(&repo, &["tag", "v0.9.0"]);
    let previous_notes = "# v0.9.0\n\nPrevious safe notes.\n";
    fs::write(repo.path().join("RELEASE_NOTES.md"), previous_notes).unwrap();

    let error = release_notes(repo.path(), "0.9.1", Some("v0.9.0"), None, &NullProgress)
        .expect_err("missing fresh notes must stop the gate");

    assert!(error.to_string().contains("release gate blocked"));
    assert!(error.to_string().contains("fresh RELEASE_NOTES.md"));
    assert_eq!(
        fs::read_to_string(repo.path().join("RELEASE_NOTES.md")).unwrap(),
        previous_notes
    );
}

#[test]
fn release_notes_rejects_a_successful_agent_with_stale_version() {
    let gh_script = write_gh_script("[]");
    let lf_script =
        "#!/bin/sh\nprintf '# v0.9.0\\n\\nWrong generated release.\\n' > RELEASE_NOTES.md\n";
    let _env = EnvGuard::new(&[("gh", gh_script.as_str()), ("lf", lf_script)]);
    let repo = TestRepo::new();
    git(&repo, &["tag", "v0.9.0"]);
    let previous_notes = "# v0.9.0\n\nPrevious safe notes.\n";
    fs::write(repo.path().join("RELEASE_NOTES.md"), previous_notes).unwrap();

    let error = release_notes(repo.path(), "0.9.1", Some("v0.9.0"), None, &NullProgress)
        .expect_err("stale-version notes must stop the gate");

    assert!(error.to_string().contains("release gate blocked"));
    assert!(error.to_string().contains("must start with '# v0.9.1'"));
    assert_eq!(
        fs::read_to_string(repo.path().join("RELEASE_NOTES.md")).unwrap(),
        previous_notes
    );
}

#[test]
fn oversized_agent_notes_keep_queue_metadata_unsafe() {
    let gh_script = write_gh_script("[]");
    let lf_script = r#"#!/bin/sh
{
  printf '# v0.9.1\n\n'
  i=0
  while [ "$i" -lt 7000 ]; do
    printf '0123456789'
    i=$((i + 1))
  done
} > RELEASE_NOTES.md
"#;
    let _env = EnvGuard::new(&[("gh", gh_script.as_str()), ("lf", lf_script)]);
    let repo = TestRepo::new();
    git(&repo, &["tag", "v0.9.0"]);

    let error = release_notes(repo.path(), "0.9.1", Some("v0.9.0"), None, &NullProgress)
        .expect_err("oversized queue metadata must stop the gate");

    assert!(error.to_string().contains("release gate blocked"));
    assert!(error.to_string().contains("maximum queue metadata"));
    assert!(!repo.path().join("RELEASE_NOTES.md").exists());
}

#[test]
fn release_notes_context_fuses_decisions_with_exact_commits() {
    let gh_script = write_gh_script("[]");
    let lf_script = "#!/bin/sh\ncp \"$LF_RELEASE_NOTES_CONTEXT\" RELEASE_CONTEXT.json\nprintf '# v0.9.1\\n\\nPrepared from context.\\n' > RELEASE_NOTES.md\n";
    let _env = EnvGuard::new(&[("gh", gh_script.as_str()), ("lf", lf_script)]);
    let repo = TestRepo::new();
    git(&repo, &["tag", "v0.9.0"]);
    fs::write(repo.path().join("feature.txt"), "release behavior\n").unwrap();
    git(&repo, &["add", "feature.txt"]);
    git(&repo, &["commit", "-m", "Ship release behavior"]);
    fs::create_dir_all(repo.path().join("release/unreleased")).unwrap();
    fs::write(
        repo.path().join("release/unreleased/DECISIONS.md"),
        "Prefer explicit completion evidence.\n",
    )
    .unwrap();

    release_notes(repo.path(), "0.9.1", Some("v0.9.0"), None, &NullProgress)
        .expect("release notes should succeed");

    let context: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(repo.path().join("RELEASE_CONTEXT.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        context["decisions"],
        "Prefer explicit completion evidence.\n"
    );
    assert_eq!(context["commits"][0]["title"], "Ship release behavior");
    assert!(context["merged_prs"].as_array().unwrap().is_empty());
}

#[test]
fn bump_version_patch() {
    assert_eq!(bump_version("v1.2.3", "patch").unwrap(), "1.2.4");
}

#[test]
fn bump_version_minor() {
    assert_eq!(bump_version("1.2.3", "minor").unwrap(), "1.3.0");
}

#[test]
fn bump_version_major() {
    assert_eq!(bump_version("v0.9.1", "major").unwrap(), "1.0.0");
}

#[test]
fn bump_version_invalid_format() {
    assert!(bump_version("1.2", "patch").is_err());
}

#[test]
fn release_check_returns_merged_prs() {
    let repo = TestRepo::new();
    git(&repo, &["tag", "v0.9.0"]);
    fs::write(repo.path().join("feature.txt"), "new stuff\n").unwrap();
    git(&repo, &["add", "feature.txt"]);
    git(&repo, &["commit", "-m", "Add feature"]);
    let sha = git_output(&repo, &["rev-parse", "HEAD"]);
    let old_sha = git_output(&repo, &["rev-parse", "v0.9.0"]);
    let pr_list = format!(
        r#"[{{"number":41,"title":"Old same-day change","body":"Already tagged","mergeCommit":{{"oid":"{old_sha}"}}}},{{"number":42,"title":"Add feature","body":"New stuff","additions":100,"deletions":10,"changedFiles":3,"mergeCommit":{{"oid":"{sha}"}}}}]"#
    );
    let gh_script = write_gh_script(&pr_list);
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);

    let changes = release_check(repo.path(), None).expect("check should succeed");
    assert_eq!(changes.commits.len(), 1);
    assert_eq!(changes.commits[0].title, "Add feature");
    assert_eq!(changes.merged_prs.len(), 1);
    assert_eq!(changes.merged_prs[0].number, 42);
    assert_eq!(changes.merged_prs[0].title, "Add feature");
}

#[test]
fn release_check_uses_direct_commits_as_release_truth() {
    let gh_script = write_gh_script("[]");
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);
    let repo = TestRepo::new();
    git(&repo, &["tag", "v0.9.0"]);
    fs::write(repo.path().join("direct.txt"), "shipped without a PR\n").unwrap();
    git(&repo, &["add", "direct.txt"]);
    git(&repo, &["commit", "-m", "Ship a direct commit"]);

    let changes = release_check(repo.path(), None).expect("check should succeed");

    assert_eq!(changes.commits.len(), 1);
    assert_eq!(changes.commits[0].title, "Ship a direct commit");
    assert!(changes.merged_prs.is_empty());
}

#[test]
fn release_check_reconciles_version_tags_to_origin() {
    let gh_script = write_gh_script("[]");
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);
    let repo = TestRepo::new();
    git(&repo, &["tag", "v0.9.0"]);
    git(&repo, &["push", "origin", "v0.9.0"]);
    let stale_sha = git_output(&repo, &["rev-parse", "v0.9.0"]);

    fs::write(repo.path().join("repair.txt"), "remote release truth\n").unwrap();
    git(&repo, &["add", "repair.txt"]);
    git(&repo, &["commit", "-m", "Repair release tag"]);
    let remote_sha = git_output(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["push", "origin", "HEAD:refs/heads/tag-repair"]);
    git_output_bare(
        &repo,
        &["update-ref", "refs/tags/v0.9.0", &remote_sha, &stale_sha],
    );
    let changes = release_check(repo.path(), None).expect("check should reconcile tags");

    assert!(changes.commits.is_empty());
    assert_eq!(git_output(&repo, &["rev-parse", "v0.9.0"]), remote_sha);
}

#[test]
fn release_check_reads_evidence_without_running_repository_hooks() {
    let gh_script = write_gh_script("[]");
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);
    let repo = TestRepo::new();
    fs::create_dir_all(repo.path().join(".lf")).unwrap();
    fs::write(
        repo.path().join(".lf/config.yaml"),
        "release:\n  targets:\n    app:\n      verify:\n        - exit 19\n",
    )
    .unwrap();

    let changes = release_check(repo.path(), Some("app")).expect("check should stay read-only");

    assert!(!changes.commits.is_empty());
}

#[test]
fn release_check_returns_empty_without_tag() {
    let gh_script = write_gh_script("[]");
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);

    let repo = TestRepo::new();
    // No tags means the first-parent history is the initial release range.
    let changes = release_check(repo.path(), None).expect("check should succeed");
    assert!(!changes.commits.is_empty());
    assert!(changes.merged_prs.is_empty());
}

#[test]
fn release_run_is_a_green_noop_without_merged_changes() {
    let gh_script = write_gh_script("[]");
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);

    let repo = TestRepo::new();
    git(&repo, &["tag", "v0.9.1"]);

    let outcome = release_run(repo.path(), "patch", None, &NullProgress)
        .expect("empty release should succeed");

    assert_eq!(
        outcome,
        ReleaseRunOutcome::NoChanges {
            target: "default".to_string(),
            latest_tag: Some("v0.9.1".to_string()),
        }
    );
}

#[test]
fn release_run_checks_the_host_local_publisher_role() {
    let gh_script = write_gh_status_script("[]", r#"{"isDraft":false}"#);
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);

    let repo = TestRepo::new();
    let checked = repo.path().join("publisher-checked");
    let publisher = repo.path().join("publisher.sh");
    fs::write(
        &publisher,
        format!(
            "#!/bin/sh\n[ \"$1\" = check ] || exit 19\n: > '{}'\n",
            checked.display()
        ),
    )
    .expect("write publisher");
    fs::create_dir_all(repo.path().join(".lf")).expect("create config dir");
    fs::write(
        repo.path().join(".lf/config.yaml"),
        "release:\n  targets:\n    default:\n      publisher: [\"sh\", \"{repo}/publisher.sh\"]\n",
    )
    .expect("write config");
    git(&repo, &["add", ".lf/config.yaml", "publisher.sh"]);
    git(&repo, &["commit", "-m", "Configure publisher role"]);
    git(&repo, &["push", "origin", "HEAD"]);
    git(&repo, &["tag", "v0.9.1"]);

    let outcome = release_run(repo.path(), "patch", None, &NullProgress)
        .expect("configured publisher should pass preflight");

    assert!(checked.exists());
    assert_eq!(
        outcome,
        ReleaseRunOutcome::NoChanges {
            target: "default".to_string(),
            latest_tag: Some("v0.9.1".to_string()),
        }
    );
}

#[test]
fn release_run_fails_closed_when_the_publisher_role_is_missing() {
    let gh_script = write_gh_script("[]");
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);

    let repo = TestRepo::new();
    fs::create_dir_all(repo.path().join(".lf")).expect("create config dir");
    fs::write(
        repo.path().join(".lf/config.yaml"),
        "release:\n  targets:\n    default:\n      publisher: [\"missing-release-publisher-role\"]\n",
    )
    .expect("write config");
    git(&repo, &["add", ".lf/config.yaml"]);
    git(&repo, &["commit", "-m", "Require publisher role"]);
    git(&repo, &["push", "origin", "HEAD"]);
    git(&repo, &["tag", "v0.9.1"]);
    let head_before = git_output(&repo, &["rev-parse", "HEAD"]);
    let tags_before = git_output(&repo, &["tag", "--list"]);

    let error = release_run(repo.path(), "patch", None, &NullProgress)
        .expect_err("missing publisher authority must stop release");

    assert!(error.to_string().contains("missing-release-publisher-role"));
    assert_eq!(git_output(&repo, &["rev-parse", "HEAD"]), head_before);
    assert_eq!(git_output(&repo, &["tag", "--list"]), tags_before);
}

#[test]
fn release_run_refuses_to_skip_an_incomplete_tag() {
    let _env = EnvGuard::new(&[("gh", write_gh_incomplete_release_script())]);

    let repo = TestRepo::new();
    fs::create_dir_all(repo.path().join(".lf")).expect("create config dir");
    fs::write(
        repo.path().join(".lf/config.yaml"),
        "release:\n  targets:\n    default:\n      publisher: [\"true\"]\n",
    )
    .expect("write config");
    git(&repo, &["tag", "v0.9.1"]);

    let error = release_run(repo.path(), "patch", None, &NullProgress)
        .expect_err("incomplete tag should stop the release");

    assert!(error.to_string().contains("v0.9.1 is incomplete"));
}

#[test]
fn release_run_keeps_a_failed_tag_red_until_a_fix_merges() {
    let _env = EnvGuard::new(&[("gh", write_gh_failed_release_script())]);

    let repo = TestRepo::new();
    fs::create_dir_all(repo.path().join(".lf")).expect("create config dir");
    fs::write(
        repo.path().join(".lf/config.yaml"),
        "release:\n  targets:\n    default:\n      publisher: [\"true\"]\n",
    )
    .expect("write config");
    git(&repo, &["tag", "v0.9.1"]);

    let error = release_run(repo.path(), "patch", None, &NullProgress)
        .expect_err("failed build without a fix should stay red");

    assert!(error.to_string().contains("no merged fix is available"));
}

#[test]
fn active_tagged_publisher_blocks_concurrent_cleanup_until_exit() {
    let repo = TestRepo::new();
    git(&repo, &["tag", "v0.9.1"]);
    git(&repo, &["push", "origin", "v0.9.1"]);
    let state = tempfile::tempdir().expect("publisher state");
    let ready = state.path().join("ready");
    let publish = state.path().join("publish");
    let completed = state.path().join("completed");
    let exit = state.path().join("exit");
    let published = state.path().join("published");
    let publisher = repo.path().join("publisher.sh");
    fs::write(
        &publisher,
        format!(
            r#"#!/bin/sh
case "$1" in
  check) exit 0 ;;
  publish)
    worktree="$LF_RELEASE_SOURCE_REPO"
    [ -e "$worktree/.git" ] || {{ echo 'tagged source missing' >&2; exit 41; }}
    [ ! -f "$worktree/publisher.sh" ] || {{ echo 'publisher control came from tag' >&2; exit 42; }}
    : > '{}'
    attempts=0
    while [ ! -f '{}' ] && [ "$attempts" -lt 500 ]; do
      attempts=$((attempts + 1))
      sleep 0.02
    done
    [ -d "$worktree" ] || {{ echo 'publisher worktree disappeared' >&2; exit 43; }}
    : > '{}'
    : > '{}'
    attempts=0
    while [ ! -f '{}' ] && [ "$attempts" -lt 500 ]; do
      attempts=$((attempts + 1))
      sleep 0.02
    done
    [ -d "$worktree" ] || {{ echo 'publisher worktree disappeared before exit' >&2; exit 44; }}
    exit 0 ;;
esac
exit 2
"#,
            ready.display(),
            publish.display(),
            published.display(),
            completed.display(),
            exit.display(),
        ),
    )
    .expect("write publisher");

    let gh_script = format!(
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo 'gh version 2.0.0'
  exit 0
fi
case "$1 $2" in
  'release view')
    if [ -f '{}' ]; then
      echo '{{"isDraft":false}}'
      exit 0
    fi
    exit 1 ;;
  'run list')
    echo '[{{"databaseId":42,"headBranch":"v0.9.1","status":"completed","conclusion":"success"}}]'
    exit 0 ;;
  'run download') exit 0 ;;
esac
echo "unexpected gh invocation: $@" >&2
exit 1
"#,
        published.display(),
    );
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);

    fs::create_dir_all(repo.path().join(".lf")).expect("config dir");
    fs::write(
        repo.path().join(".lf/config.yaml"),
        "release:\n  targets:\n    default:\n      publisher: [\"sh\", \"{repo}/publisher.sh\"]\n",
    )
    .expect("release config");
    git(&repo, &["add", ".lf/config.yaml", "publisher.sh"]);
    git(&repo, &["commit", "-m", "Add current publisher controller"]);
    git(&repo, &["push", "origin", "HEAD"]);
    let tags_before = git_output(&repo, &["tag", "--list"]);
    let neighbor = repo.create_named_worktree("neighbor");
    let repo_name = repo
        .path()
        .file_name()
        .expect("repo name")
        .to_string_lossy();
    let publisher_worktree = repo
        .path()
        .parent()
        .expect("repo parent")
        .join(format!("{repo_name}.publish-default-v0-9-1"));

    let release = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["release", "run", "patch"])
        .current_dir(repo.path())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("start release re-entry");

    wait_for_path(&ready);
    assert!(publisher_worktree.exists());
    let removal = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["wt", "remove", "publish-default-v0-9-1"])
        .current_dir(repo.path())
        .output()
        .expect("attempt concurrent cleanup");

    assert!(!removal.status.success());
    assert!(
        String::from_utf8_lossy(&removal.stderr).contains("release publisher for v0.9.1"),
        "unexpected cleanup error: {}",
        String::from_utf8_lossy(&removal.stderr)
    );
    assert!(publisher_worktree.exists());
    assert!(neighbor.exists());

    fs::write(&publish, "").expect("allow publication");
    wait_for_path(&completed);
    assert!(
        publisher_worktree.exists(),
        "owner cleanup must wait for publisher exit"
    );
    fs::write(&exit, "").expect("allow publisher exit");

    let output = release
        .wait_with_output()
        .expect("wait for release re-entry");
    assert!(
        output.status.success(),
        "release re-entry failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!publisher_worktree.exists());
    assert!(neighbor.exists());
    assert_eq!(git_output(&repo, &["tag", "--list"]), tags_before);
}

#[test]
fn release_bump_updates_cargo_toml() {
    let repo = TestRepo::new();
    fs::write(
        repo.path().join("Cargo.toml"),
        "[package]\nname = \"test\"\nversion = \"0.9.0\"\n",
    )
    .unwrap();

    release_bump(repo.path(), "0.9.1", None, &NullProgress).expect("bump should succeed");

    let content = fs::read_to_string(repo.path().join("Cargo.toml")).unwrap();
    assert!(content.contains("version = \"0.9.1\""));
    assert!(!content.contains("version = \"0.9.0\""));
}

#[test]
fn release_status_reports_latest_tag_and_release() {
    let gh_script = write_gh_status_script(
        r#"[{"databaseId":7,"headBranch":"main","displayTitle":"Retry v0.9.1","status":"completed","conclusion":"failure","url":"https://example.com/run/7"},{"databaseId":42,"headBranch":"v0.9.1","displayTitle":"Release v0.9.1","status":"completed","conclusion":"success","url":"https://example.com/run/42"}]"#,
        r#"{"isDraft":false}"#,
    );
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);

    let repo = TestRepo::new();
    git(&repo, &["tag", "v0.9.1"]);

    let status = release_status(repo.path(), None).expect("status should succeed");
    assert_eq!(status.target, "default");
    assert_eq!(status.latest_tag.as_deref(), Some("v0.9.1"));
    assert_eq!(status.workflow_status.as_deref(), Some("completed"));
    assert_eq!(status.workflow_conclusion.as_deref(), Some("success"));
    assert_eq!(status.notes_status, Some(ReleaseNotesStatus::Missing));
    assert!(status.release_exists);
}

#[test]
fn release_status_reports_degraded_notes_as_gate_safe() {
    let gh_script = write_gh_status_script(
        r#"[{"databaseId":42,"headBranch":"v0.9.1","status":"completed","conclusion":"success"}]"#,
        r#"{"isDraft":false}"#,
    );
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);
    let repo = TestRepo::new();
    fs::create_dir_all(repo.path().join("release/v0.9.1")).unwrap();
    fs::write(
        repo.path().join("release/v0.9.1/NOTES.md"),
        "# v0.9.1\n\n<!-- loopflow:release-notes=degraded;reason=quota;gate=safe -->\n\nConcise fallback notes.\n",
    )
    .unwrap();
    git(&repo, &["add", "release/v0.9.1/NOTES.md"]);
    git(&repo, &["commit", "-m", "Archive degraded release notes"]);
    git(&repo, &["tag", "v0.9.1"]);

    let status = release_status(repo.path(), None).expect("status should read tagged notes");

    assert_eq!(
        status.notes_status,
        Some(ReleaseNotesStatus::Degraded(ReleaseNotesDegradation::Quota))
    );
    assert!(status.release_exists);
}

#[test]
fn release_status_reports_unmarked_notes_as_legacy() {
    let gh_script = write_gh_status_script("[]", r#"{"isDraft":false}"#);
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);
    let repo = TestRepo::new();
    fs::create_dir_all(repo.path().join("release/v0.9.1")).unwrap();
    fs::write(
        repo.path().join("release/v0.9.1/NOTES.md"),
        "# v0.9.1\n\nNotes from before status markers.\n",
    )
    .unwrap();
    git(&repo, &["add", "release/v0.9.1/NOTES.md"]);
    git(&repo, &["commit", "-m", "Archive legacy release notes"]);
    git(&repo, &["tag", "v0.9.1"]);

    let status = release_status(repo.path(), None).expect("status should read legacy notes");

    assert_eq!(status.notes_status, Some(ReleaseNotesStatus::Legacy));
    assert!(status.release_exists);
}

#[test]
fn release_status_does_not_trust_malformed_safe_markers() {
    let gh_script = write_gh_status_script("[]", r#"{"isDraft":false}"#);
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);
    let markers = [
        "narrative;gate=unsafe -->",
        "degraded;reason=quota -->",
        "degraded;reason=unknown;gate=safe -->",
    ];

    for marker in markers {
        let repo = TestRepo::new();
        fs::create_dir_all(repo.path().join("release/v0.9.1")).unwrap();
        fs::write(
            repo.path().join("release/v0.9.1/NOTES.md"),
            format!("# v0.9.1\n\n<!-- loopflow:release-notes={marker}\n\nNotes.\n"),
        )
        .unwrap();
        git(&repo, &["add", "release/v0.9.1/NOTES.md"]);
        git(&repo, &["commit", "-m", "Archive malformed release notes"]);
        git(&repo, &["tag", "v0.9.1"]);

        let status = release_status(repo.path(), None).expect("status should fail closed");

        assert_eq!(status.notes_status, Some(ReleaseNotesStatus::Legacy));
    }
}

#[test]
fn release_status_does_not_count_a_draft_as_complete() {
    let gh_script = write_gh_status_script(
        r#"[{"databaseId":42,"headBranch":"v0.9.1","status":"completed","conclusion":"success"}]"#,
        r#"{"isDraft":true}"#,
    );
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);

    let repo = TestRepo::new();
    git(&repo, &["tag", "v0.9.1"]);

    let status = release_status(repo.path(), None).expect("status should succeed");

    assert!(!status.release_exists);
}

#[test]
fn release_status_scopes_to_named_target() {
    let gh_script = write_gh_status_script(
        r#"[{"databaseId":7,"headBranch":"cli/v1.2.3","displayTitle":"Release cli/v1.2.3","status":"completed","conclusion":"success","url":"https://example.com/run/7"}]"#,
        r#"{"isDraft":false}"#,
    );
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);

    let repo = TestRepo::new();
    fs::create_dir_all(repo.path().join(".lf")).expect("create config dir");
    fs::write(
        repo.path().join(".lf/config.yaml"),
        r#"
release:
  targets:
    cli:
      tag_prefix: "cli/"
"#,
    )
    .expect("write config");
    git(&repo, &["tag", "cli/v1.2.3"]);

    let status = release_status(repo.path(), Some("cli")).expect("status should succeed");
    assert_eq!(status.target, "cli");
    assert_eq!(status.latest_tag.as_deref(), Some("cli/v1.2.3"));
    assert!(status.release_exists);
}

#[test]
fn release_tag_is_idempotent_when_remote_tag_matches_head() {
    let repo = TestRepo::new();
    let head = git_output(&repo, &["rev-parse", "HEAD"]);

    let tag = release_tag(repo.path(), "0.9.1", None).expect("first tag should succeed");
    assert_eq!(tag, "v0.9.1");

    let second = release_tag(repo.path(), "0.9.1", None).expect("second tag should be no-op");
    assert_eq!(second, "v0.9.1");

    let remote_tag = git_output_bare(&repo, &["rev-parse", "refs/tags/v0.9.1"]);
    assert_eq!(remote_tag, head);
}

#[test]
fn release_run_resumes_an_existing_explicit_tag() {
    let gh_script = write_gh_status_script("[]", r#"{"isDraft":false}"#);
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);
    let repo = TestRepo::new();
    let intended_notes = "# v0.9.1\n\n<!-- loopflow:release-notes=narrative;gate=safe -->\n\nConcise intended artifact.\n";
    fs::create_dir_all(repo.path().join("release/v0.9.1")).unwrap();
    fs::write(repo.path().join("release/v0.9.1/NOTES.md"), intended_notes).unwrap();
    git(&repo, &["add", "release/v0.9.1/NOTES.md"]);
    git(&repo, &["commit", "-m", "Prepare exact release artifact"]);
    release_tag(repo.path(), "0.9.1", None).expect("tag should succeed");

    let result = release_run(repo.path(), "0.9.1", None, &NullProgress)
        .expect("interrupted release should resume from its tag");

    let ReleaseRunOutcome::Resumed(receipt) = result else {
        panic!("expected resumed release");
    };
    assert_eq!(receipt.version, "0.9.1");
    assert_eq!(receipt.tag, "v0.9.1");
    assert!(receipt.release_exists);
    let status = release_status(repo.path(), None).expect("status should preserve narrative notes");
    assert_eq!(status.notes_status, Some(ReleaseNotesStatus::Narrative));
    assert_eq!(
        git_output_bare(&repo, &["show", "v0.9.1:release/v0.9.1/NOTES.md",],),
        intended_notes.trim()
    );
    assert_eq!(
        git_output_bare(
            &repo,
            &["for-each-ref", "--format=%(refname)", "refs/tags/v0.9.1"],
        )
        .lines()
        .count(),
        1
    );
}

#[test]
fn release_tag_fails_if_remote_tag_points_to_different_commit() {
    let repo = TestRepo::new();
    release_tag(repo.path(), "0.9.1", None).expect("initial tag should succeed");

    repo.create_file("CHANGELOG.md", "new release prep");
    repo.stage_all();
    repo.commit("change after tag");

    let err = release_tag(repo.path(), "0.9.1", None).expect_err("mismatched tag should fail");
    let message = err.to_string();
    assert!(message.contains("already exists on origin"));
    assert!(message.contains("expected"));
}
