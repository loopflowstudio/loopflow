use std::fs;
use std::path::Path;

use loopflow::engine::{
    format_prompt, gather_context, DocumentSource, GatherContextOpts, GatheredContext,
    PromptFormatMode, Surface,
};
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
    let skills_dir = repo.join(".lf/skills");
    fs::create_dir_all(&skills_dir).unwrap();
    fs::write(skills_dir.join(format!("{name}.md")), content).unwrap();
}

fn write_direction(repo: &Path, name: &str, content: &str) {
    let dir = repo.join(".lf/directions");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(format!("{name}.md")), content).unwrap();
}

fn write_direction_group(repo: &Path, group: &str, name: &str, content: &str) {
    let dir = repo.join(".lf/directions").join(group);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(format!("{name}.md")), content).unwrap();
}

fn render_prompt(components: GatheredContext) -> String {
    format_prompt(PromptFormatMode::Full, components.components()).into_string()
}

// =============================================================================
// Basic context gathering
// =============================================================================

#[test]
fn gather_context_with_skill() {
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
        directions: vec![],
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
        directions: vec![],
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

#[test]
fn gather_context_with_directions() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);

    write_skill(repo, "review", "Review the code.");
    write_direction(repo, "concise", "Be brief and direct.");
    write_direction(repo, "security", "Focus on security issues.");
    make_commit(repo, "initial");

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        skill: Some("review".to_string()),
        message: None,
        operate: false,
        surface: Surface::Headless,
        directions: vec!["concise".to_string(), "security".to_string()],
        files: vec![],
        docs: vec![],
        wave: None,
        related_repos: Vec::new(),
        ..Default::default()
    })
    .unwrap();

    assert_eq!(components.directions.len(), 2);
}

#[test]
fn gather_context_expands_user_direction_group() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);
    write_skill(repo, "review", "Review the code.");
    write_direction_group(repo, "mygroup", "alpha", "Alpha direction");
    write_direction_group(repo, "mygroup", "beta", "Beta direction");
    make_commit(repo, "initial");

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        skill: Some("review".to_string()),
        message: None,
        operate: false,
        surface: Surface::Headless,
        directions: vec!["mygroup".to_string()],
        files: vec![],
        docs: vec![],
        wave: None,
        related_repos: Vec::new(),
        ..Default::default()
    })
    .unwrap();

    let direction_names: Vec<String> = components
        .directions
        .iter()
        .map(|direction| direction.name.clone())
        .collect();
    assert_eq!(
        direction_names,
        vec!["alpha".to_string(), "beta".to_string()]
    );
}

// =============================================================================
// Docs gathering
// =============================================================================

#[test]
fn gather_context_includes_explicit_readme_docs_target() {
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
        directions: vec![],
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
        directions: vec![],
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
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);

    fs::create_dir_all(repo.join("wave/auth")).unwrap();
    fs::write(repo.join("wave/auth/README.md"), "# Auth Wave\nBuild auth.").unwrap();
    write_skill(repo, "implement", "Do work.");
    make_commit(repo, "initial");

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        skill: Some("implement".to_string()),
        message: None,
        operate: false,
        surface: Surface::Headless,
        directions: vec![],
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
        directions: vec![],
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
        directions: vec![],
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
        directions: vec![],
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
        directions: vec![],
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
fn format_prompt_includes_directions() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);

    write_skill(repo, "review", "Review code.");
    write_direction(repo, "concise", "Be brief.");
    make_commit(repo, "initial");

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        skill: Some("review".to_string()),
        message: None,
        operate: false,
        surface: Surface::Headless,
        directions: vec!["concise".to_string()],
        files: vec![],
        docs: vec![],
        wave: None,
        related_repos: Vec::new(),
        ..Default::default()
    })
    .unwrap();

    let prompt = render_prompt(components);
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
    write_skill(repo, "implement", "Do work.");
    make_commit(repo, "initial");

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        skill: Some("implement".to_string()),
        message: None,
        operate: false,
        surface: Surface::Headless,
        directions: vec![],
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
        directions: vec![],
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
        directions: vec![],
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
        directions: vec![],
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

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        skill: Some("implement".to_string()),
        message: None,
        operate: false,
        surface: Surface::Headless,
        directions: vec![],
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
fn wave_memory_is_loaded_separately_from_wave_docs() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);

    fs::create_dir_all(repo.join("wave/living")).unwrap();
    fs::write(repo.join("wave/living/README.md"), "# Living").unwrap();
    fs::write(repo.join("wave/living/plan.md"), "# Plan").unwrap();
    fs::write(
        repo.join("wave/living/MEMORY.md"),
        "- keep tests focused on behavior",
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
        directions: vec![],
        files: vec![],
        docs: vec![],
        wave: Some("living".to_string()),
        related_repos: Vec::new(),
        ..Default::default()
    })
    .unwrap();

    let wave_docs: Vec<_> = components
        .docs
        .iter()
        .filter(|d| d.source == DocumentSource::Wave)
        .collect();
    assert_eq!(
        wave_docs.len(),
        2,
        "README.md and plan.md should be wave docs"
    );
    assert!(wave_docs.iter().all(|d| !d.path.ends_with("MEMORY.md")));

    assert!(components.wave_memory.is_some());
    assert_eq!(
        components.wave_memory.as_ref().map(|doc| doc.path.as_str()),
        Some("wave/living/MEMORY.md")
    );
    assert!(components
        .wave_memory
        .as_ref()
        .expect("wave memory should be loaded")
        .content
        .contains("keep tests focused on behavior"));

    let prompt = render_prompt(components);
    assert!(prompt.contains("<lf:wave-memory>"));
    assert!(prompt.contains("keep tests focused on behavior"));
}

// =============================================================================
// Ambient wave context
// =============================================================================

#[test]
fn run_in_wave_context_assembles_chat_and_memory_sections() {
    use loopflow::wave::journal::{journal_path, EventKind, Journal, MessageId, MessageOp};

    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);
    fs::create_dir_all(repo.join("wave/goals")).unwrap();
    fs::write(
        repo.join("wave/goals/MEMORY.md"),
        "- child progress arrives as typed Work observations",
    )
    .unwrap();
    make_commit(repo, "initial");

    // The wave's journal, as its server would have written it.
    let (mut journal, _) = Journal::open(&journal_path(repo, "goals")).unwrap();
    journal.append(|seq| EventKind::UserMessage {
        id: MessageId(format!("msg-{seq}")),
        op: MessageOp::Message,
        text: "how goes the build?".to_string(),
    });

    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        wave: Some("goals".to_string()),
        ..Default::default()
    })
    .unwrap();

    let prompt = render_prompt(components);
    assert!(prompt.contains("<lf:wave-chat-recent>"));
    assert!(prompt.contains("user: how goes the build?"));
    assert!(prompt.contains("<lf:wave-memory>"));
    assert!(prompt.contains("child progress arrives as typed Work observations"));
}

#[test]
fn run_outside_any_wave_assembles_neither_section() {
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
    assert!(!prompt.contains("<lf:wave-chat-recent>"));
    assert!(!prompt.contains("<lf:wave-memory>"));
}

#[test]
fn worktree_reads_the_origin_repos_wave_memory() {
    let temp = TempDir::new().unwrap();
    let origin = temp.path().join("repo");
    fs::create_dir_all(&origin).unwrap();
    init_repo(&origin);
    fs::create_dir_all(origin.join("wave/goals")).unwrap();
    fs::write(
        origin.join("wave/goals/MEMORY.md"),
        "- origin memory is the truth",
    )
    .unwrap();
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
    // The worktree's committed copy lags the origin: the origin must win.
    fs::write(
        worktree.join("wave/goals/MEMORY.md"),
        "- stale worktree copy",
    )
    .unwrap();

    let components = gather_context(&GatherContextOpts {
        repo_root: worktree.clone(),
        wave: Some("goals".to_string()),
        ..Default::default()
    })
    .unwrap();

    let memory = components.wave_memory.as_ref().expect("memory resolved");
    assert!(memory.content.contains("origin memory is the truth"));
    assert!(!memory.content.contains("stale worktree copy"));
}
