use std::ffi::OsString;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::sync::{LazyLock, Mutex};

use loopflow::engine::{load_config, load_config_or_default};
use tempfile::TempDir;

static HOME_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

struct HomeOverride {
    original: Option<OsString>,
}

impl HomeOverride {
    fn new(path: &Path) -> Self {
        let original = std::env::var_os("HOME");
        std::env::set_var("HOME", path);
        Self { original }
    }
}

impl Drop for HomeOverride {
    fn drop(&mut self) {
        match self.original.as_ref() {
            Some(home) => std::env::set_var("HOME", home),
            None => std::env::remove_var("HOME"),
        }
    }
}

fn with_clean_home<T>(f: impl FnOnce() -> T) -> T {
    let _lock = HOME_LOCK.lock().unwrap();
    let home = TempDir::new().unwrap();
    let _override = HomeOverride::new(home.path());
    f()
}

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
    let config = with_clean_home(|| load_config_or_default(Some(temp.path())));
    assert!(config.agent.is_none());
}

#[test]
fn load_config_or_default_handles_empty_file() {
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), "");
    let config = with_clean_home(|| load_config_or_default(Some(temp.path())));
    assert!(config.agent.is_none());
}

#[test]
fn load_config_or_default_handles_whitespace_only() {
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), "   \n\n  ");
    let config = with_clean_home(|| load_config_or_default(Some(temp.path())));
    assert!(config.agent.is_none());
}

#[test]
fn load_config_parses_basic_yaml() {
    let temp = TempDir::new().unwrap();
    write_config(
        temp.path(),
        r#"
agent: claude:sonnet
yolo: true
"#,
    );

    let config = with_clean_home(|| load_config(Some(temp.path())))
        .expect("config should load")
        .expect("config should exist");
    assert_eq!(config.agent.as_deref(), Some("claude:sonnet"));
    assert!(config.yolo);
}

#[test]
fn load_config_or_default_returns_defaults() {
    let temp = TempDir::new().unwrap();
    let config = with_clean_home(|| load_config_or_default(Some(temp.path())));

    assert!(config.agent.is_none());
    assert!(!config.yolo);
    assert!(!config.chrome);
}

// =============================================================================
// Model configuration
// =============================================================================

#[test]
fn config_model_with_variant() {
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), "agent: gemini:2.5-pro");

    let config = with_clean_home(|| load_config(Some(temp.path())))
        .unwrap()
        .unwrap();
    assert_eq!(config.agent.as_deref(), Some("gemini:2.5-pro"));
}

#[test]
fn config_model_without_variant() {
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), "agent: codex");

    let config = with_clean_home(|| load_config(Some(temp.path())))
        .unwrap()
        .unwrap();
    assert_eq!(config.agent.as_deref(), Some("codex"));
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
pr: true
"#,
    );

    let config = with_clean_home(|| load_config(Some(temp.path())))
        .unwrap()
        .unwrap();
    assert!(config.yolo);
    assert!(config.chrome);
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

    let config = with_clean_home(|| load_config(Some(temp.path())))
        .unwrap()
        .unwrap();
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

    let config = with_clean_home(|| load_config(Some(temp.path())))
        .unwrap()
        .unwrap();
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

    let config = with_clean_home(|| load_config(Some(temp.path())))
        .unwrap()
        .unwrap();
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

    let config = with_clean_home(|| load_config(Some(temp.path())))
        .unwrap()
        .unwrap();
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

    let config = with_clean_home(|| load_config(Some(temp.path())))
        .unwrap()
        .unwrap();
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

    let config = with_clean_home(|| load_config(Some(temp.path())))
        .unwrap()
        .unwrap();
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

    let config = with_clean_home(|| load_config(Some(temp.path())))
        .unwrap()
        .unwrap();
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

    let config = with_clean_home(|| load_config(Some(temp.path())))
        .unwrap()
        .unwrap();
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

    let config = with_clean_home(|| load_config(Some(temp.path())))
        .unwrap()
        .unwrap();
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

    let config = with_clean_home(|| load_config(Some(temp.path())))
        .unwrap()
        .unwrap();
    assert!(config.branch_names.is_some());
}

// =============================================================================
// Land strategy
// =============================================================================

#[test]
fn config_land_strategy() {
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), "land: local");

    let config = with_clean_home(|| load_config(Some(temp.path())))
        .unwrap()
        .unwrap();
    assert_eq!(config.land, "local");
}

#[test]
fn config_land_defaults_to_gh() {
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), "yolo: false");

    let config = with_clean_home(|| load_config(Some(temp.path())))
        .unwrap()
        .unwrap();
    assert_eq!(config.land, "gh");
}

// =============================================================================
// Autoprune
// =============================================================================

#[test]
fn config_autoprune_bool() {
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), "autoprune: true");

    let config = with_clean_home(|| load_config(Some(temp.path())))
        .unwrap()
        .unwrap();
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

    let config = with_clean_home(|| load_config(Some(temp.path())))
        .unwrap()
        .unwrap();
    assert!(config.autoprune.enabled);
    assert_eq!(config.autoprune.poll_interval_seconds, 120);
}
