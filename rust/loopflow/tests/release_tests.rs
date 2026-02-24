mod support;

use std::fs;
use std::process::Command;

use loopflow::ops::{release, NullProgress};
use loopflow_test_support::TestRepo;
use support::EnvGuard;

fn write_gh_script(pr_list: &str) -> String {
    format!(
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo 'gh version 2.0.0'\n  exit 0\nfi\ncase \"$1 $2\" in\n  'pr list')\n    cat <<'JSON'\n{pr_list}\nJSON\n    exit 0;;\n  'pr create')\n    echo 'https://example.com/pr/42'\n    exit 0;;\n  'pr merge')\n    exit 0;;\nesac\necho \"unexpected gh invocation: $@\" >&2\nexit 1\n"
    )
}

fn write_claude_script(output: &str) -> String {
    format!("#!/bin/sh\ncat <<'EOF'\n{output}\nEOF\n")
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
fn release_generates_notes_and_opens_pr() {
    let gh_script = write_gh_script(
        r#"[{"number":101,"title":"Add release command","body":"Users can now run lf ops release."}]"#,
    );
    let claude_script =
        write_claude_script("## Highlights\n\n- Added automated release notes generation.");
    let _env = EnvGuard::new(&[
        ("gh", gh_script.as_str()),
        ("claude", claude_script.as_str()),
    ]);

    let repo = TestRepo::new();
    let _ = git_output(&repo, &["tag", "v0.8.0"]);

    let url = release(repo.path(), "v0.9.1", &NullProgress).expect("release should succeed");

    assert_eq!(url, "https://example.com/pr/42");

    let notes = fs::read_to_string(repo.path().join("RELEASE_NOTES.md")).expect("read notes");
    assert!(notes.starts_with("# v0.9.1\n\n"));
    assert!(notes.contains("## Highlights"));

    assert_eq!(
        git_output(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "release/v0.9.1"
    );
    assert!(
        !git_output(&repo, &["ls-remote", "--heads", "origin", "release/v0.9.1"]).is_empty(),
        "release branch should be pushed to origin"
    );
}

#[test]
fn release_keeps_existing_header() {
    let gh_script = write_gh_script("[]");
    let claude_script = write_claude_script("# v0.9.2\n\nAlready has header.");
    let _env = EnvGuard::new(&[
        ("gh", gh_script.as_str()),
        ("claude", claude_script.as_str()),
    ]);

    let repo = TestRepo::new();
    let _ = git_output(&repo, &["tag", "v0.9.1"]);

    release(repo.path(), "0.9.2", &NullProgress).expect("release should succeed");

    let notes = fs::read_to_string(repo.path().join("RELEASE_NOTES.md")).expect("read notes");
    assert_eq!(notes.lines().next().expect("first line"), "# v0.9.2");
    assert_eq!(notes.matches("# v0.9.2").count(), 1);
}

#[test]
fn release_branch_starts_from_default_branch() {
    let gh_script = write_gh_script("[]");
    let claude_script = write_claude_script("## Highlights\n\n- Release notes.");
    let _env = EnvGuard::new(&[
        ("gh", gh_script.as_str()),
        ("claude", claude_script.as_str()),
    ]);

    let repo = TestRepo::new();
    let _ = git_output(&repo, &["tag", "v0.9.1"]);
    repo.create_branch("feature-branch");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");

    release(repo.path(), "0.9.2", &NullProgress).expect("release should succeed");

    let commits = git_output(&repo, &["log", "--format=%s", "main..release/v0.9.2"]);
    assert_eq!(commits.lines().collect::<Vec<_>>(), vec!["release: v0.9.2"]);
}

#[test]
fn release_requires_clean_working_tree() {
    let gh_script = write_gh_script("[]");
    let claude_script = write_claude_script("## Highlights\n\n- Release notes.");
    let _env = EnvGuard::new(&[
        ("gh", gh_script.as_str()),
        ("claude", claude_script.as_str()),
    ]);

    let repo = TestRepo::new();
    repo.create_file("dirty.txt", "dirty");

    let error = release(repo.path(), "0.9.2", &NullProgress).expect_err("release should fail");
    assert!(error.to_string().contains("working tree dirty"));
}
