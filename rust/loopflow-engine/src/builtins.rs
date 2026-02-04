//! Built-in step definitions and system docs embedded in the binary.
//!
//! These are the core steps and documentation that work everywhere,
//! even without any local step files.

use std::collections::HashMap;
use std::sync::LazyLock;

/// Bundled LOOPFLOW.md system documentation.
pub const LOOPFLOW_DOC: &str = include_str!("builtins/LOOPFLOW.md");

/// Returns the content of a built-in step, if it exists.
pub fn get_builtin_step(name: &str) -> Option<&'static str> {
    BUILTIN_STEPS.get(name).copied()
}

/// List of all built-in step names.
pub fn builtin_step_names() -> Vec<&'static str> {
    BUILTIN_STEPS.keys().copied().collect()
}

static BUILTIN_STEPS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // Code steps
    m.insert("debug", include_str!("builtins/steps/debug.md"));
    m.insert("implement", include_str!("builtins/steps/implement.md"));

    // Interactive steps
    m.insert("design", include_str!("builtins/steps/design.md"));
    m.insert("explore", include_str!("builtins/steps/explore.md"));

    // Plan steps
    m.insert("review", include_str!("builtins/steps/review.md"));
    m.insert("iterate", include_str!("builtins/steps/iterate.md"));
    m.insert("polish", include_str!("builtins/steps/polish.md"));
    m.insert("reduce", include_str!("builtins/steps/reduce.md"));
    m.insert("expand", include_str!("builtins/steps/expand.md"));

    // Ops steps
    m.insert("lint", include_str!("builtins/steps/lint.md"));

    m
});
