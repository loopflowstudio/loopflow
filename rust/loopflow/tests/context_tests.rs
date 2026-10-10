mod support;

use std::fs;
use std::path::Path;
use std::sync::Arc;

use loopflow::context_block::{ContextDelivery, ContextMoment};
use loopflow::ops::resolve_work_binding;
use loopflow::prompt::format_prompt;
use loopflow::prompt::gather_context;
use loopflow::prompt::DocumentSource;
use loopflow::prompt::GatherContextOpts;
use loopflow::prompt::PromptComponents;
use loopflow::prompt::Surface;
use loopflow::prompt::{gather_documents, GatherSpec};
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

fn setup_multi_wave_repo(repo: &Path) {
    for (path, text) in [
        ("auth/README.md", "Authentication system."),
        ("auth/oauth.md", "OAuth provider setup."),
        ("payments/README.md", "Payment processing."),
        ("payments/stripe.md", "Stripe integration guide."),
        ("search/README.md", "Elastic search setup."),
    ] {
        let path = repo.join("wave").join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
}

#[test]
fn wave_filtering_includes_only_specified_wave() {
    let repo = TempDir::new().unwrap();
    setup_multi_wave_repo(repo.path());
    let docs = gather_documents(&GatherSpec {
        repo_root: repo.path().to_owned(),
        wave: Some("auth".into()),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        docs.iter()
            .map(|doc| (doc.path.as_str(), doc.content.as_str(), doc.source))
            .collect::<Vec<_>>(),
        [
            (
                "wave/auth/README.md",
                "Authentication system.",
                DocumentSource::Wave
            ),
            (
                "wave/auth/oauth.md",
                "OAuth provider setup.",
                DocumentSource::Wave
            ),
        ]
    );
}

#[test]
fn wave_filtering_excludes_waves_without_a_matching_selection() {
    let repo = TempDir::new().unwrap();
    setup_multi_wave_repo(repo.path());
    for wave in [None, Some("nonexistent".into())] {
        let docs = gather_documents(&GatherSpec {
            repo_root: repo.path().to_owned(),
            wave,
            ..Default::default()
        })
        .unwrap();
        assert!(docs.is_empty());
    }
}

#[test]
fn wave_filtering_includes_all_markdown_in_path_order() {
    let repo = TempDir::new().unwrap();
    let directory = repo.path().join("wave/features");
    fs::create_dir_all(&directory).unwrap();
    let files = [
        "01-core.md",
        "02-advanced.md",
        "03-experimental.md",
        "README.md",
    ];
    for file in files {
        fs::write(directory.join(file), file).unwrap();
    }
    let docs = gather_documents(&GatherSpec {
        repo_root: repo.path().to_owned(),
        wave: Some("features".into()),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(docs.len(), files.len());
    for (doc, file) in docs.iter().zip(files) {
        assert_eq!(doc.path, format!("wave/features/{file}"));
        assert_eq!(doc.content, file);
        assert_eq!(doc.source, DocumentSource::Wave);
    }
}

#[test]
fn nested_wave_reads_checkout_ancestor_markdown_in_order() {
    let _env = support::EnvGuard::new(&[]);
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
    let prompt = components
        .docs
        .iter()
        .map(|doc| doc.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
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
    let delivery = ContextDelivery::prepare(&components).unwrap();
    assert!(
        delivery.references.is_empty(),
        "repository memory is a live source, not a second reference"
    );
    let prompt = delivery.block(ContextMoment::Start).unwrap().text;
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
async fn worktree_reads_its_checkout_wave_memory() {
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
    // Checkout edits are the next launch context, independent of main.
    fs::write(
        worktree.join("wave/goals/MEMORY.md"),
        "- checkout-local decisions",
    )
    .unwrap();

    fs::write(worktree.join("wave/goals/GOAL.md"), "Checkout goal.").unwrap();
    loopflow::store::sqlite::SqliteStore::new(&loopflow::store::database_path_from_env().unwrap())
        .unwrap()
        .ensure_wave(origin.canonicalize().unwrap().to_str().unwrap(), "goals")
        .unwrap();
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

    let prompt = components
        .docs
        .iter()
        .map(|doc| doc.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(prompt.matches("checkout-local decisions").count(), 1);
    assert_eq!(prompt.matches("Checkout goal.").count(), 1);
    assert!(!prompt.contains("previous committed memory"));
    assert!(!prompt.contains("Origin goal."));
}
