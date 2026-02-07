use std::fs;
use std::path::Path;

use loopflow::engine::{load_config, load_config_or_default};
use tempfile::TempDir;

fn write_config(dir: &Path, content: &str) {
    let lf_dir = dir.join(".lf");
    fs::create_dir_all(&lf_dir).unwrap();
    fs::write(lf_dir.join("config.yaml"), content).unwrap();
}

// =============================================================================
// Config file loading
// =============================================================================

#[test]
fn load_config_or_default_handles_missing_file() {
    let temp = TempDir::new().unwrap();
    // Should return defaults without error
    let config = load_config_or_default(Some(temp.path()));
    assert_eq!(config.agent_model, "claude:opus");
}

#[test]
fn load_config_or_default_handles_empty_file() {
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), "");
    // Should return defaults without error
    let config = load_config_or_default(Some(temp.path()));
    assert_eq!(config.agent_model, "claude:opus");
}

#[test]
fn load_config_or_default_handles_whitespace_only() {
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), "   \n\n  ");
    // Should return defaults without error
    let config = load_config_or_default(Some(temp.path()));
    assert_eq!(config.agent_model, "claude:opus");
}

#[test]
fn load_config_parses_basic_yaml() {
    let temp = TempDir::new().unwrap();
    write_config(
        temp.path(),
        r#"
agent_model: claude:sonnet
yolo: true
"#,
    );

    let config = load_config(Some(temp.path()))
        .expect("config should load")
        .expect("config should exist");
    assert_eq!(config.agent_model, "claude:sonnet");
    assert!(config.yolo);
}

#[test]
fn load_config_or_default_returns_defaults() {
    let temp = TempDir::new().unwrap();
    let config = load_config_or_default(Some(temp.path()));

    assert_eq!(config.agent_model, "claude:opus");
    assert!(!config.yolo);
    assert!(!config.chrome);
}

// =============================================================================
// Model configuration
// =============================================================================

#[test]
fn config_model_with_variant() {
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), "agent_model: gemini:2.5-pro");

    let config = load_config(Some(temp.path())).unwrap().unwrap();
    assert_eq!(config.agent_model, "gemini:2.5-pro");
}

#[test]
fn config_model_without_variant() {
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), "agent_model: codex");

    let config = load_config(Some(temp.path())).unwrap().unwrap();
    assert_eq!(config.agent_model, "codex");
}

// =============================================================================
// Feature flags
// =============================================================================

#[test]
fn config_feature_flags() {
    let temp = TempDir::new().unwrap();
    write_config(
        temp.path(),
        r#"
yolo: true
chrome: true
push: true
pr: true
"#,
    );

    let config = load_config(Some(temp.path())).unwrap().unwrap();
    assert!(config.yolo);
    assert!(config.chrome);
    assert!(config.push);
    assert!(config.pr);
}

// =============================================================================
// Context and exclude patterns
// =============================================================================

#[test]
fn config_context_as_list() {
    let temp = TempDir::new().unwrap();
    write_config(
        temp.path(),
        r#"
context:
  - src/
  - lib/
"#,
    );

    let config = load_config(Some(temp.path())).unwrap().unwrap();
    assert_eq!(config.context, vec!["src/", "lib/"]);
}

#[test]
fn config_exclude_as_list() {
    let temp = TempDir::new().unwrap();
    write_config(
        temp.path(),
        r#"
exclude:
  - "*.log"
  - node_modules/
"#,
    );

    let config = load_config(Some(temp.path())).unwrap().unwrap();
    assert_eq!(config.exclude, vec!["*.log", "node_modules/"]);
}

// =============================================================================
// Directions
// =============================================================================

#[test]
fn config_direction_as_list() {
    let temp = TempDir::new().unwrap();
    write_config(
        temp.path(),
        r#"
direction:
  - concise
  - security
"#,
    );

    let config = load_config(Some(temp.path())).unwrap().unwrap();
    assert_eq!(
        config.direction,
        Some(vec!["concise".to_string(), "security".to_string()])
    );
}

#[test]
fn config_direction_single_item_list() {
    let temp = TempDir::new().unwrap();
    write_config(
        temp.path(),
        r#"
direction:
  - architect
"#,
    );

    let config = load_config(Some(temp.path())).unwrap().unwrap();
    assert_eq!(config.direction, Some(vec!["architect".to_string()]));
}

// =============================================================================
// Interactive steps
// =============================================================================

#[test]
fn config_interactive_step_list() {
    let temp = TempDir::new().unwrap();
    write_config(
        temp.path(),
        r#"
interactive:
  - design
  - review
"#,
    );

    let config = load_config(Some(temp.path())).unwrap().unwrap();
    assert!(config.interactive.contains(&"design".to_string()));
    assert!(config.interactive.contains(&"review".to_string()));
}

// =============================================================================
// IDE settings
// =============================================================================

#[test]
fn config_ide_cursor_enabled() {
    let temp = TempDir::new().unwrap();
    write_config(
        temp.path(),
        r#"
ide:
  cursor: true
  warp: false
"#,
    );

    let config = load_config(Some(temp.path())).unwrap().unwrap();
    assert!(config.ide.cursor);
    assert!(!config.ide.warp);
}

#[test]
fn config_ide_workspace() {
    let temp = TempDir::new().unwrap();
    write_config(
        temp.path(),
        r#"
ide:
  workspace: "project.code-workspace"
"#,
    );

    let config = load_config(Some(temp.path())).unwrap().unwrap();
    assert_eq!(
        config.ide.workspace,
        Some("project.code-workspace".to_string())
    );
}

// =============================================================================
// Budget configuration
// =============================================================================

#[test]
fn config_budgets() {
    let temp = TempDir::new().unwrap();
    write_config(
        temp.path(),
        r#"
budgets:
  area: 50000
  docs: 20000
  diff: 30000
"#,
    );

    let config = load_config(Some(temp.path())).unwrap().unwrap();
    assert_eq!(config.budgets.area, 50000);
    assert_eq!(config.budgets.docs, 20000);
    assert_eq!(config.budgets.diff, 30000);
}

// =============================================================================
// Summaries configuration
// =============================================================================

#[test]
fn config_summaries() {
    let temp = TempDir::new().unwrap();
    write_config(
        temp.path(),
        r#"
summaries:
  - path: src/
    tokens: 500
  - path: lib/
    tokens: 300
"#,
    );

    let config = load_config(Some(temp.path())).unwrap().unwrap();
    assert_eq!(config.summaries.len(), 2);
}

// =============================================================================
// Branch naming
// =============================================================================

#[test]
fn config_branch_names() {
    let temp = TempDir::new().unwrap();
    write_config(
        temp.path(),
        r#"
branch_names:
  schema: "{user}.{words}.{date}"
"#,
    );

    let config = load_config(Some(temp.path())).unwrap().unwrap();
    assert!(config.branch_names.is_some());
}

// =============================================================================
// Lint check
// =============================================================================

#[test]
fn config_lint_check() {
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), r#"lint_check: "cargo clippy -- -D warnings""#);

    let config = load_config(Some(temp.path())).unwrap().unwrap();
    assert_eq!(
        config.lint_check,
        Some("cargo clippy -- -D warnings".to_string())
    );
}

// =============================================================================
// Land strategy
// =============================================================================

#[test]
fn config_land_strategy() {
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), "land: local");

    let config = load_config(Some(temp.path())).unwrap().unwrap();
    assert_eq!(config.land, "local");
}

#[test]
fn config_land_defaults_to_gh() {
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), "yolo: false");

    let config = load_config(Some(temp.path())).unwrap().unwrap();
    assert_eq!(config.land, "gh");
}

// =============================================================================
// Autoprune
// =============================================================================

#[test]
fn config_autoprune_bool() {
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), "autoprune: true");

    let config = load_config(Some(temp.path())).unwrap().unwrap();
    assert!(config.autoprune.enabled);
}

#[test]
fn config_autoprune_object() {
    let temp = TempDir::new().unwrap();
    write_config(
        temp.path(),
        r#"
autoprune:
  enabled: true
  poll_interval_seconds: 120
"#,
    );

    let config = load_config(Some(temp.path())).unwrap().unwrap();
    assert!(config.autoprune.enabled);
    assert_eq!(config.autoprune.poll_interval_seconds, 120);
}
