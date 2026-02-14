use std::fs;
use std::path::Path;

use loopflow::engine::{format_prompt, gather_context, GatherContextOpts};
use tempfile::TempDir;

fn init_repo(dir: &Path) {
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(dir)
        .output()
        .expect("git init");
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir)
        .output()
        .expect("git config email");
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir)
        .output()
        .expect("git config name");
}

fn make_commit(dir: &Path, message: &str) {
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(dir)
        .output()
        .expect("git add");
    std::process::Command::new("git")
        .args(["commit", "-m", message, "--allow-empty"])
        .current_dir(dir)
        .output()
        .expect("git commit");
}

fn write_step(repo: &Path, name: &str, content: &str) {
    let steps_dir = repo.join(".lf/steps");
    fs::create_dir_all(&steps_dir).unwrap();
    fs::write(steps_dir.join(format!("{name}.md")), content).unwrap();
}

fn write_direction(repo: &Path, name: &str, content: &str) {
    let dir = repo.join(".lf/directions");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(format!("{name}.md")), content).unwrap();
}

// =============================================================================
// Basic context gathering
// =============================================================================

#[test]
fn gather_context_with_step() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);

    write_step(repo, "implement", "Build the feature described above.");
    make_commit(repo, "initial");

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        step: Some("implement".to_string()),
        message: None,
        run_mode: Some("auto".to_string()),
        directions: vec![],
        files: vec![],
        lfdocs: false,
        diff_files: false,
        diff: false,
        clipboard: false,
        area: None,
        wave: None,
    })
    .unwrap();

    assert!(components.step.is_some());
    assert!(components.step.as_ref().unwrap().content.is_some());
}

#[test]
fn gather_context_with_inline_prompt() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);
    make_commit(repo, "initial");

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        step: None,
        message: Some("Fix the bug in main.rs".to_string()),
        run_mode: Some("interactive".to_string()),
        directions: vec![],
        files: vec![],
        lfdocs: false,
        diff_files: false,
        diff: false,
        clipboard: false,
        area: None,
        wave: None,
    })
    .unwrap();

    // No step when using inline
    assert!(components.step.is_none());
}

#[test]
fn gather_context_with_directions() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);

    write_step(repo, "review", "Review the code.");
    write_direction(repo, "concise", "Be brief and direct.");
    write_direction(repo, "security", "Focus on security issues.");
    make_commit(repo, "initial");

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        step: Some("review".to_string()),
        message: None,
        run_mode: Some("auto".to_string()),
        directions: vec!["concise".to_string(), "security".to_string()],
        files: vec![],
        lfdocs: false,
        diff_files: false,
        diff: false,
        clipboard: false,
        area: None,
        wave: None,
    })
    .unwrap();

    assert_eq!(components.directions.len(), 2);
}

// =============================================================================
// Docs gathering
// =============================================================================

#[test]
fn gather_context_includes_readme() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);

    fs::write(repo.join("README.md"), "# Project\nThis is a test.").unwrap();
    write_step(repo, "implement", "Do work.");
    make_commit(repo, "initial");

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        step: Some("implement".to_string()),
        message: None,
        run_mode: Some("auto".to_string()),
        directions: vec![],
        files: vec![],
        lfdocs: true,
        diff_files: false,
        diff: false,
        clipboard: false,
        area: None,
        wave: None,
    })
    .unwrap();

    let docs_content: String = components
        .docs
        .iter()
        .map(|d| d.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(docs_content.contains("# Project"));
}

#[test]
fn gather_context_includes_scratch_docs() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);

    fs::create_dir_all(repo.join("scratch")).unwrap();
    fs::write(
        repo.join("scratch/design.md"),
        "# Design\nArchitecture notes.",
    )
    .unwrap();
    write_step(repo, "implement", "Do work.");
    make_commit(repo, "initial");

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        step: Some("implement".to_string()),
        message: None,
        run_mode: Some("auto".to_string()),
        directions: vec![],
        files: vec![],
        lfdocs: true,
        diff_files: false,
        diff: false,
        clipboard: false,
        area: None,
        wave: None,
    })
    .unwrap();

    let docs_content: String = components
        .docs
        .iter()
        .map(|d| d.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(docs_content.contains("Architecture notes"));
}

// =============================================================================
// Wave context
// =============================================================================

#[test]
fn gather_context_with_wave() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);

    fs::create_dir_all(repo.join("wave/auth")).unwrap();
    fs::write(repo.join("wave/auth/README.md"), "# Auth Wave\nBuild auth.").unwrap();
    write_step(repo, "implement", "Do work.");
    make_commit(repo, "initial");

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        step: Some("implement".to_string()),
        message: None,
        run_mode: Some("auto".to_string()),
        directions: vec![],
        files: vec![],
        lfdocs: true,
        diff_files: false,
        diff: false,
        clipboard: false,
        area: None,
        wave: Some("auth".to_string()),
    })
    .unwrap();

    assert_eq!(components.wave.as_deref(), Some("auth"));
}

// =============================================================================
// Run mode
// =============================================================================

#[test]
fn gather_context_preserves_run_mode() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);
    write_step(repo, "debug", "Fix it.");
    make_commit(repo, "initial");

    let auto = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        step: Some("debug".to_string()),
        message: None,
        run_mode: Some("auto".to_string()),
        directions: vec![],
        files: vec![],
        lfdocs: false,
        diff_files: false,
        diff: false,
        clipboard: false,
        area: None,
        wave: None,
    })
    .unwrap();

    let interactive = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        step: Some("debug".to_string()),
        message: None,
        run_mode: Some("interactive".to_string()),
        directions: vec![],
        files: vec![],
        lfdocs: false,
        diff_files: false,
        diff: false,
        clipboard: false,
        area: None,
        wave: None,
    })
    .unwrap();

    assert_eq!(auto.run_mode.as_deref(), Some("auto"));
    assert_eq!(interactive.run_mode.as_deref(), Some("interactive"));
}

// =============================================================================
// Prompt formatting
// =============================================================================

#[test]
fn format_prompt_includes_step_content() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);

    write_step(repo, "implement", "Build the feature now.");
    make_commit(repo, "initial");

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        step: Some("implement".to_string()),
        message: None,
        run_mode: Some("auto".to_string()),
        directions: vec![],
        files: vec![],
        lfdocs: false,
        diff_files: false,
        diff: false,
        clipboard: false,
        area: None,
        wave: None,
    })
    .unwrap();

    let prompt = format_prompt(&components);
    assert!(prompt.contains("Build the feature now."));
}

#[test]
fn format_prompt_includes_auto_mode_header() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);

    write_step(repo, "implement", "Do work.");
    make_commit(repo, "initial");

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        step: Some("implement".to_string()),
        message: None,
        run_mode: Some("auto".to_string()),
        directions: vec![],
        files: vec![],
        lfdocs: false,
        diff_files: false,
        diff: false,
        clipboard: false,
        area: None,
        wave: None,
    })
    .unwrap();

    let prompt = format_prompt(&components);
    assert!(prompt.contains("auto"));
}

#[test]
fn format_prompt_includes_directions() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);

    write_step(repo, "review", "Review code.");
    write_direction(repo, "concise", "Be brief.");
    make_commit(repo, "initial");

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        step: Some("review".to_string()),
        message: None,
        run_mode: Some("auto".to_string()),
        directions: vec!["concise".to_string()],
        files: vec![],
        lfdocs: false,
        diff_files: false,
        diff: false,
        clipboard: false,
        area: None,
        wave: None,
    })
    .unwrap();

    let prompt = format_prompt(&components);
    assert!(prompt.contains("Be brief."));
}

#[test]
fn format_prompt_includes_wave_context() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);

    fs::create_dir_all(repo.join("wave/payments")).unwrap();
    fs::write(
        repo.join("wave/payments/README.md"),
        "# Payments\nStripe integration.",
    )
    .unwrap();
    write_step(repo, "implement", "Do work.");
    make_commit(repo, "initial");

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        step: Some("implement".to_string()),
        message: None,
        run_mode: Some("auto".to_string()),
        directions: vec![],
        files: vec![],
        lfdocs: true,
        diff_files: false,
        diff: false,
        clipboard: false,
        area: None,
        wave: Some("payments".to_string()),
    })
    .unwrap();

    let prompt = format_prompt(&components);
    assert!(prompt.contains("payments"));
}

// =============================================================================
// Wave filtering (fixture-based tests)
// =============================================================================

/// Setup multiple wave directories for isolation tests
fn setup_multi_wave_repo(repo: &Path) {
    // Create auth wave
    fs::create_dir_all(repo.join("wave/auth")).unwrap();
    fs::write(
        repo.join("wave/auth/README.md"),
        "# Auth Wave\nAuthentication system.",
    )
    .unwrap();
    fs::write(
        repo.join("wave/auth/oauth.md"),
        "# OAuth\nOAuth provider setup.",
    )
    .unwrap();

    // Create payments wave
    fs::create_dir_all(repo.join("wave/payments")).unwrap();
    fs::write(
        repo.join("wave/payments/README.md"),
        "# Payments Wave\nPayment processing.",
    )
    .unwrap();
    fs::write(
        repo.join("wave/payments/stripe.md"),
        "# Stripe\nStripe integration guide.",
    )
    .unwrap();

    // Create search wave
    fs::create_dir_all(repo.join("wave/search")).unwrap();
    fs::write(
        repo.join("wave/search/README.md"),
        "# Search Wave\nElastic search setup.",
    )
    .unwrap();
}

#[test]
fn wave_filtering_includes_only_specified_wave() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);
    setup_multi_wave_repo(repo);
    write_step(repo, "implement", "Do work.");
    make_commit(repo, "initial");

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        step: Some("implement".to_string()),
        message: None,
        run_mode: Some("auto".to_string()),
        directions: vec![],
        files: vec![],
        lfdocs: true,
        diff_files: false,
        diff: false,
        clipboard: false,
        area: None,
        wave: Some("auth".to_string()),
    })
    .unwrap();

    let docs_content: String = components
        .docs
        .iter()
        .map(|d| d.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    // Should include auth wave content
    assert!(
        docs_content.contains("Authentication system"),
        "Should include auth wave README"
    );
    assert!(
        docs_content.contains("OAuth provider setup"),
        "Should include auth wave oauth.md"
    );

    // Should NOT include other waves
    assert!(
        !docs_content.contains("Payment processing"),
        "Should NOT include payments wave"
    );
    assert!(
        !docs_content.contains("Stripe integration"),
        "Should NOT include stripe.md"
    );
    assert!(
        !docs_content.contains("Elastic search"),
        "Should NOT include search wave"
    );
}

#[test]
fn wave_filtering_excludes_all_waves_when_no_wave() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);
    setup_multi_wave_repo(repo);
    write_step(repo, "implement", "Do work.");
    make_commit(repo, "initial");

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        step: Some("implement".to_string()),
        message: None,
        run_mode: Some("auto".to_string()),
        directions: vec![],
        files: vec![],
        lfdocs: true,
        diff_files: false,
        diff: false,
        clipboard: false,
        area: None,
        wave: None, // No wave specified
    })
    .unwrap();

    let docs_content: String = components
        .docs
        .iter()
        .map(|d| d.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    // Should NOT include ANY wave content
    assert!(
        !docs_content.contains("Authentication system"),
        "Should NOT include auth wave"
    );
    assert!(
        !docs_content.contains("Payment processing"),
        "Should NOT include payments wave"
    );
    assert!(
        !docs_content.contains("Elastic search"),
        "Should NOT include search wave"
    );
}

#[test]
fn wave_filtering_handles_nonexistent_wave() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);
    setup_multi_wave_repo(repo);
    write_step(repo, "implement", "Do work.");
    make_commit(repo, "initial");

    // Specifying a wave that doesn't exist should not fail
    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        step: Some("implement".to_string()),
        message: None,
        run_mode: Some("auto".to_string()),
        directions: vec![],
        files: vec![],
        lfdocs: true,
        diff_files: false,
        diff: false,
        clipboard: false,
        area: None,
        wave: Some("nonexistent".to_string()),
    })
    .unwrap();

    let docs_content: String = components
        .docs
        .iter()
        .map(|d| d.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    // Should NOT include ANY wave content (nonexistent wave)
    assert!(
        !docs_content.contains("Authentication system"),
        "Should NOT include auth wave"
    );
    assert!(
        !docs_content.contains("Payment processing"),
        "Should NOT include payments wave"
    );
}

#[test]
fn wave_filtering_includes_all_files_in_wave_directory() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);

    // Create wave with multiple files
    fs::create_dir_all(repo.join("wave/features")).unwrap();
    fs::write(
        repo.join("wave/features/README.md"),
        "# Features Overview\nMain features doc.",
    )
    .unwrap();
    fs::write(
        repo.join("wave/features/01-core.md"),
        "# Core Features\nCore feature list.",
    )
    .unwrap();
    fs::write(
        repo.join("wave/features/02-advanced.md"),
        "# Advanced Features\nAdvanced feature list.",
    )
    .unwrap();
    fs::write(
        repo.join("wave/features/03-experimental.md"),
        "# Experimental\nExperimental features.",
    )
    .unwrap();

    write_step(repo, "implement", "Do work.");
    make_commit(repo, "initial");

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        step: Some("implement".to_string()),
        message: None,
        run_mode: Some("auto".to_string()),
        directions: vec![],
        files: vec![],
        lfdocs: true,
        diff_files: false,
        diff: false,
        clipboard: false,
        area: None,
        wave: Some("features".to_string()),
    })
    .unwrap();

    // Count wave docs
    let wave_docs: Vec<_> = components
        .docs
        .iter()
        .filter(|d| d.category == "wave")
        .collect();

    assert_eq!(
        wave_docs.len(),
        4,
        "Should include all 4 files from features wave"
    );

    let docs_content: String = wave_docs.iter().map(|d| d.content.as_str()).collect();
    assert!(docs_content.contains("Main features doc"));
    assert!(docs_content.contains("Core feature list"));
    assert!(docs_content.contains("Advanced feature list"));
    assert!(docs_content.contains("Experimental features"));
}

#[test]
fn loopflow_doc_always_included() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);
    write_step(repo, "implement", "Do work.");
    make_commit(repo, "initial");

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        step: Some("implement".to_string()),
        message: None,
        run_mode: Some("auto".to_string()),
        directions: vec![],
        files: vec![],
        lfdocs: false, // Even with lfdocs=false
        diff_files: false,
        diff: false,
        clipboard: false,
        area: None,
        wave: None,
    })
    .unwrap();

    // LOOPFLOW.md should always be present
    assert!(
        components.loopflow_doc.is_some(),
        "LOOPFLOW.md should always be included"
    );
    assert!(
        !components.loopflow_doc.as_ref().unwrap().is_empty(),
        "LOOPFLOW.md should have content"
    );
}
