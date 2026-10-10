mod support;

use std::fs;
use std::path::Path;
use std::sync::Arc;

use loopflow::ops::resolve_work_binding;
use loopflow::prompt::format_prompt;
use loopflow::prompt::gather_context;
use loopflow::prompt::DocumentSource;
use loopflow::prompt::GatherContextOpts;
use loopflow::prompt::PromptComponents;
use loopflow::prompt::Surface;
use loopflow::store::{open_ephemeral_store, StorageConfig};
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

fn write_skill(repo: &Path, name: &str, content: &str) {
    let path = repo.join(".lf/skills").join(format!("{name}.md"));
    fs::create_dir_all(path.parent().expect("skill path has parent")).unwrap();
    fs::write(path, content).unwrap();
}

fn import_wave(repo: &Path, name: &str) {
    loopflow::store::sqlite::SqliteStore::new(&loopflow::store::database_path_from_env().unwrap())
        .unwrap()
        .ensure_wave(repo.canonicalize().unwrap().to_str().unwrap(), name)
        .unwrap();
}

fn render_prompt(components: PromptComponents) -> String {
    format_prompt(&components)
}

// =============================================================================
// Basic context gathering
// =============================================================================

#[test]
fn gather_context_with_skill() {
    let _env = support::EnvGuard::new(&[]);
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);

    write_skill(repo, "implement", "Build the feature described above.");
    make_commit(repo, "initial");

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        skill: Some("implement".to_string()),
        message: None,
        operate: false,
        surface: Surface::Headless,
        files: vec![],
        docs: vec![],
        wave: None,
        related_repos: Vec::new(),
        ..Default::default()
    })
    .unwrap();

    assert!(components.skill.is_some());
    assert!(components.skill.as_ref().unwrap().content.is_some());
}

#[test]
fn gather_context_with_inline_prompt() {
    let _env = support::EnvGuard::new(&[]);
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);
    make_commit(repo, "initial");

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        skill: None,
        message: Some("Fix the bug in main.rs".to_string()),
        operate: false,
        surface: Surface::Cli,
        files: vec![],
        docs: vec![],
        wave: None,
        related_repos: Vec::new(),
        ..Default::default()
    })
    .unwrap();

    // No skill when using inline
    assert!(components.skill.is_none());
}

// =============================================================================
// Docs gathering
// =============================================================================

#[test]
fn gather_context_includes_explicit_readme_docs_target() {
    let _env = support::EnvGuard::new(&[]);
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);

    fs::write(repo.join("README.md"), "# Project\nThis is a test.").unwrap();
    write_skill(repo, "implement", "Do work.");
    make_commit(repo, "initial");

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        skill: Some("implement".to_string()),
        message: None,
        operate: false,
        surface: Surface::Headless,
        files: vec![],
        docs: vec!["README.md".to_string()],
        wave: None,
        related_repos: Vec::new(),
        ..Default::default()
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
    let _env = support::EnvGuard::new(&[]);
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);

    fs::create_dir_all(repo.join("scratch")).unwrap();
    fs::write(
        repo.join("scratch/design.md"),
        "# Design\nArchitecture notes.",
    )
    .unwrap();
    write_skill(repo, "implement", "Do work.");
    make_commit(repo, "initial");

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        skill: Some("implement".to_string()),
        message: None,
        operate: false,
        surface: Surface::Headless,
        files: vec![],
        docs: vec![],
        wave: None,
        related_repos: Vec::new(),
        ..Default::default()
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
    let _env = support::EnvGuard::new(&[]);
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);

    fs::create_dir_all(repo.join("wave/auth")).unwrap();
    fs::write(repo.join("wave/auth/README.md"), "# Auth Wave\nBuild auth.").unwrap();
    write_skill(repo, "implement", "Do work.");
    make_commit(repo, "initial");

    import_wave(repo, "auth");
    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        skill: Some("implement".to_string()),
        message: None,
        operate: false,
        surface: Surface::Headless,
        files: vec![],
        docs: vec![],
        wave: Some("auth".to_string()),
        related_repos: Vec::new(),
        ..Default::default()
    })
    .unwrap();

    assert_eq!(components.wave.as_deref(), Some("auth"));
}

// =============================================================================
// Surface
// =============================================================================

#[test]
fn gather_context_preserves_surface() {
    let _env = support::EnvGuard::new(&[]);
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);
    write_skill(repo, "debug", "Fix it.");
    make_commit(repo, "initial");

    let auto = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        skill: Some("debug".to_string()),
        message: None,
        operate: false,
        surface: Surface::Headless,
        files: vec![],
        docs: vec![],
        wave: None,
        related_repos: Vec::new(),
        ..Default::default()
    })
    .unwrap();

    let interactive = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        skill: Some("debug".to_string()),
        message: None,
        operate: false,
        surface: Surface::Cli,
        files: vec![],
        docs: vec![],
        wave: None,
        related_repos: Vec::new(),
        ..Default::default()
    })
    .unwrap();

    assert_eq!(auto.surface, Surface::Headless);
    assert_eq!(interactive.surface, Surface::Cli);
}

// =============================================================================
// Prompt formatting
// =============================================================================

#[test]
fn format_prompt_includes_skill_content() {
    let _env = support::EnvGuard::new(&[]);
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);

    write_skill(repo, "implement", "Build the feature now.");
    make_commit(repo, "initial");

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        skill: Some("implement".to_string()),
        message: None,
        operate: false,
        surface: Surface::Headless,
        files: vec![],
        docs: vec![],
        wave: None,
        related_repos: Vec::new(),
        ..Default::default()
    })
    .unwrap();

    let prompt = render_prompt(components);
    assert!(prompt.contains("Build the feature now."));
}

#[test]
fn format_prompt_includes_auto_mode_header() {
    let _env = support::EnvGuard::new(&[]);
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);

    write_skill(repo, "implement", "Do work.");
    make_commit(repo, "initial");

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        skill: Some("implement".to_string()),
        message: None,
        operate: false,
        surface: Surface::Headless,
        files: vec![],
        docs: vec![],
        wave: None,
        related_repos: Vec::new(),
        ..Default::default()
    })
    .unwrap();

    let prompt = render_prompt(components);
    assert!(prompt.contains("Run mode is headless"));
}

#[test]
fn format_prompt_includes_wave_context() {
    let _env = support::EnvGuard::new(&[]);
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);

    fs::create_dir_all(repo.join("wave/payments")).unwrap();
    fs::write(
        repo.join("wave/payments/README.md"),
        "# Payments\nStripe integration.",
    )
    .unwrap();
    write_skill(repo, "implement", "Do work.");
    make_commit(repo, "initial");

    import_wave(repo, "payments");
    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        skill: Some("implement".to_string()),
        message: None,
        operate: false,
        surface: Surface::Headless,
        files: vec![],
        docs: vec![],
        wave: Some("payments".to_string()),
        related_repos: Vec::new(),
        ..Default::default()
    })
    .unwrap();

    let prompt = render_prompt(components);
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
    let _env = support::EnvGuard::new(&[]);
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);
    setup_multi_wave_repo(repo);
    write_skill(repo, "implement", "Do work.");
    make_commit(repo, "initial");

    import_wave(repo, "auth");
    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        skill: Some("implement".to_string()),
        message: None,
        operate: false,
        surface: Surface::Headless,
        files: vec![],
        docs: vec![],
        wave: Some("auth".to_string()),
        related_repos: Vec::new(),
        ..Default::default()
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
    let _env = support::EnvGuard::new(&[]);
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);
    setup_multi_wave_repo(repo);
    write_skill(repo, "implement", "Do work.");
    make_commit(repo, "initial");

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        skill: Some("implement".to_string()),
        message: None,
        operate: false,
        surface: Surface::Headless,
        files: vec![],
        docs: vec![],
        wave: None, // No wave specified
        related_repos: Vec::new(),
        ..Default::default()
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
    let _env = support::EnvGuard::new(&[]);
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);
    setup_multi_wave_repo(repo);
    write_skill(repo, "implement", "Do work.");
    make_commit(repo, "initial");

    // Specifying a wave that doesn't exist should not fail
    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        skill: Some("implement".to_string()),
        message: None,
        operate: false,
        surface: Surface::Headless,
        files: vec![],
        docs: vec![],
        wave: Some("nonexistent".to_string()),
        related_repos: Vec::new(),
        ..Default::default()
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
    let _env = support::EnvGuard::new(&[]);
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

    write_skill(repo, "implement", "Do work.");
    make_commit(repo, "initial");

    import_wave(repo, "features");
    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        skill: Some("implement".to_string()),
        message: None,
        operate: false,
        surface: Surface::Headless,
        files: vec![],
        docs: vec![],
        wave: Some("features".to_string()),
        related_repos: Vec::new(),
        ..Default::default()
    })
    .unwrap();

    // Count wave docs
    let wave_docs: Vec<_> = components
        .docs
        .iter()
        .filter(|d| d.source == DocumentSource::Wave)
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
fn nested_wave_reads_stored_ancestor_markdown_in_order() {
    let _env = support::EnvGuard::new(&[]);
    // Import the definition once; context reads the stored hierarchy.
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    for (path, content) in [
        ("MEMORY.md", "Repository decisions"),
        ("wave/infrastructure/README.md", "Parent introduction"),
        ("wave/infrastructure/GOAL.md", "Parent goal"),
        ("wave/infrastructure/MEMORY.md", "Parent decisions"),
        ("wave/infrastructure/release/GOAL.md", "Release goal"),
        ("wave/infrastructure/release/MEMORY.md", "Release decisions"),
        ("wave/infrastructure/release/notes.md", "Release notes"),
        ("wave/infrastructure/auth/MEMORY.md", "Sibling secrets"),
        (
            "wave/infrastructure/release/child/MEMORY.md",
            "Child details",
        ),
        ("wave/product/MEMORY.md", "Unrelated details"),
        ("wave/infrastructure/release/data.json", "Not Markdown"),
        ("scratch/deep/design.md", "Recursive scratch"),
    ] {
        let file = repo.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, content).unwrap();
    }
    import_wave(repo, "infrastructure/release");
    let mut components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        wave: Some("infrastructure/release".into()),
        docs: vec!["wave/infrastructure/release/GOAL.md".into()],
        ..Default::default()
    })
    .unwrap();
    loopflow::prompt::drop_duplicate_docs(&mut components, repo);
    let paths: Vec<_> = components
        .docs
        .iter()
        .filter(|doc| {
            matches!(
                doc.source,
                DocumentSource::RepoMemory | DocumentSource::Wave
            )
        })
        .map(|doc| doc.path.as_str())
        .collect();
    assert_eq!(
        paths,
        [
            "MEMORY.md",
            "wave/infrastructure/GOAL.md",
            "wave/infrastructure/MEMORY.md",
            "wave/infrastructure/README.md",
            "wave/infrastructure/release/GOAL.md",
            "wave/infrastructure/release/MEMORY.md",
            "wave/infrastructure/release/notes.md",
        ]
    );
    let prompt = render_prompt(components);
    assert!(prompt.contains("Recursive scratch"));
    for content in [
        "Repository decisions",
        "Parent introduction",
        "Parent goal",
        "Parent decisions",
        "Release goal",
        "Release decisions",
        "Release notes",
    ] {
        assert_eq!(prompt.matches(content).count(), 1, "{content}");
    }
    assert!(prompt.find("Parent decisions").unwrap() < prompt.find("Release goal").unwrap());
    assert!(prompt.find("Repository decisions").unwrap() < prompt.find("Parent goal").unwrap());
    for excluded in [
        "Sibling secrets",
        "Child details",
        "Unrelated details",
        "Not Markdown",
    ] {
        assert!(!prompt.contains(excluded));
    }
}

#[test]
fn context_delivery_repository_memory_is_included_once_without_a_wave() {
    let _env = support::EnvGuard::new(&[]);
    let temp = TempDir::new().unwrap();
    fs::write(temp.path().join("MEMORY.md"), "Repository decisions.").unwrap();
    let mut components = gather_context(&GatherContextOpts {
        repo_root: temp.path().to_path_buf(),
        docs: vec!["MEMORY.md".into()],
        ..Default::default()
    })
    .unwrap();
    let decisions = loopflow::prompt::drop_duplicate_docs(&mut components, temp.path());
    assert_eq!(components.docs.len(), 1);
    assert_eq!(components.docs[0].source, DocumentSource::RepoMemory);
    assert!(decisions.iter().any(|decision| {
        decision.source_path.as_deref() == Some("MEMORY.md")
            && decision.decision == loopflow::trace::ContextDecisionKind::Deduplicated
    }));
    let prompt = render_prompt(components);
    assert_eq!(prompt.matches("Repository decisions.").count(), 1);
    assert!(!prompt.contains("<lf:wave name="));
}

#[test]
fn run_outside_any_wave_assembles_no_memory_section() {
    let _env = support::EnvGuard::new(&[]);
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);
    write_skill(repo, "implement", "Do work.");
    make_commit(repo, "initial");

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        skill: Some("implement".to_string()),
        ..Default::default()
    })
    .unwrap();

    let prompt = render_prompt(components);
    assert!(!prompt.contains("<lf:wave"));
}

#[tokio::test]
async fn worktree_reads_shared_stored_wave_memory() {
    let _env = support::EnvGuard::new(&[]);
    let temp = TempDir::new().unwrap();
    let origin = temp.path().join("repo");
    fs::create_dir_all(&origin).unwrap();
    init_repo(&origin);
    fs::create_dir_all(origin.join("wave/goals")).unwrap();
    fs::write(
        origin.join("wave/goals/MEMORY.md"),
        "- previous committed memory",
    )
    .unwrap();
    fs::write(origin.join("wave/goals/GOAL.md"), "Origin goal.").unwrap();
    make_commit(&origin, "initial");

    // A sibling worktree, as `lf wave` bootstraps: <repo>.goals.
    let worktree = temp.path().join("repo.goals");
    std::process::Command::new("git")
        .args([
            "worktree",
            "add",
            worktree.to_str().unwrap(),
            "-b",
            "goals-branch",
        ])
        .current_dir(&origin)
        .output()
        .expect("git worktree add");
    // Checkout edits do not replace the stored plan.
    fs::write(
        worktree.join("wave/goals/MEMORY.md"),
        "- checkout-local decisions",
    )
    .unwrap();

    fs::write(worktree.join("wave/goals/GOAL.md"), "Checkout goal.").unwrap();
    import_wave(&origin, "goals");
    let store = Arc::new(
        open_ephemeral_store(&StorageConfig::sqlite(
            loopflow::store::database_path_from_env().unwrap(),
        ))
        .await
        .unwrap(),
    );
    let binding = resolve_work_binding(&store, &worktree, "wave:goals")
        .await
        .unwrap();
    assert_eq!(
        binding.cwd.canonicalize().unwrap(),
        worktree.canonicalize().unwrap()
    );
    let components = gather_context(&GatherContextOpts {
        repo_root: binding.cwd,
        wave: Some(binding.wave_name),
        message: Some(binding.context),
        operate: true,
        ..Default::default()
    })
    .unwrap();

    let prompt = render_prompt(components);
    assert_eq!(prompt.matches("previous committed memory").count(), 1);
    assert_eq!(prompt.matches("Origin goal.").count(), 1);
    assert_eq!(prompt.matches("<lf:loopflow>").count(), 1);
    assert!(prompt.contains("Curate stored Wave memory with `lf wave edit goals --memory <file>`."));
    assert!(!prompt.contains("checkout-local decisions"));
    assert!(!prompt.contains("Checkout goal."));
}
