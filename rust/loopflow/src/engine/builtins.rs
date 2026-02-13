//! Built-in step definitions and system docs embedded in the binary.
//!
//! These are the core steps and documentation that work everywhere,
//! even without any local step files.

use std::collections::HashMap;
use std::sync::LazyLock;

/// Bundled LOOPFLOW.md system documentation.
pub const LOOPFLOW_DOC: &str = include_str!("builtins/LOOPFLOW.md");

/// Bundled RLM.md — recursive language model instructions for agents.
pub const RLM_DOC: &str = include_str!("builtins/RLM.md");

/// Returns the content of a built-in step, if it exists.
pub fn get_builtin_step(name: &str) -> Option<&'static str> {
    BUILTIN_STEPS.get(name).copied()
}

/// Returns the content of a built-in flow, if it exists.
pub fn get_builtin_flow(name: &str) -> Option<&'static str> {
    BUILTIN_FLOWS.get(name).copied()
}

/// Returns the content of a built-in direction, if it exists.
pub fn get_builtin_direction(name: &str) -> Option<&'static str> {
    BUILTIN_DIRECTIONS.get(name).copied()
}

/// Returns the content of a built-in ops prompt, if it exists.
pub fn get_builtin_ops_prompt(name: &str) -> Option<&'static str> {
    BUILTIN_OPS_PROMPTS.get(name).copied()
}

/// List of all built-in step names.
pub fn builtin_step_names() -> Vec<&'static str> {
    BUILTIN_STEPS.keys().copied().collect()
}

/// List of all built-in flow names.
pub fn builtin_flow_names() -> Vec<&'static str> {
    BUILTIN_FLOWS.keys().copied().collect()
}

/// List of all built-in direction names.
pub fn builtin_direction_names() -> Vec<&'static str> {
    BUILTIN_DIRECTIONS.keys().copied().collect()
}

/// List of all built-in ops prompt names.
pub fn builtin_ops_prompt_names() -> Vec<&'static str> {
    BUILTIN_OPS_PROMPTS.keys().copied().collect()
}

static BUILTIN_STEPS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // Code steps
    m.insert("compress", include_str!("builtins/steps/code/compress.md"));
    m.insert("debug", include_str!("builtins/steps/code/debug.md"));
    m.insert("gate", include_str!("builtins/steps/code/gate.md"));
    m.insert(
        "implement",
        include_str!("builtins/steps/code/implement.md"),
    );

    // Interactive steps
    m.insert(
        "design",
        include_str!("builtins/steps/interactive/design.md"),
    );
    m.insert(
        "explore",
        include_str!("builtins/steps/interactive/explore.md"),
    );
    m.insert(
        "refine",
        include_str!("builtins/steps/interactive/refine.md"),
    );

    // Plan steps
    m.insert("5whys", include_str!("builtins/steps/plan/5whys.md"));
    m.insert("expand", include_str!("builtins/steps/plan/expand.md"));
    m.insert("ingest", include_str!("builtins/steps/plan/ingest.md"));
    m.insert("iterate", include_str!("builtins/steps/plan/iterate.md"));
    m.insert("kickoff", include_str!("builtins/steps/plan/kickoff.md"));
    m.insert("polish", include_str!("builtins/steps/plan/polish.md"));
    m.insert("reduce", include_str!("builtins/steps/plan/reduce.md"));
    m.insert("review", include_str!("builtins/steps/plan/review.md"));
    m.insert("roadmap", include_str!("builtins/steps/plan/roadmap.md"));

    // Ops steps
    m.insert(
        "add-to-roadmap",
        include_str!("builtins/steps/ops/add-to-roadmap.md"),
    );
    m.insert("commit", include_str!("builtins/steps/ops/commit.md"));
    m.insert(
        "consolidate",
        include_str!("builtins/steps/ops/consolidate.md"),
    );
    m.insert("init", include_str!("builtins/steps/ops/init.md"));
    m.insert("lint", include_str!("builtins/steps/ops/lint.md"));
    m.insert("rebase", include_str!("builtins/steps/ops/rebase.md"));
    m.insert(
        "synthesize",
        include_str!("builtins/steps/ops/synthesize.md"),
    );
    m.insert("validate", include_str!("builtins/steps/ops/validate.md"));

    m
});

static BUILTIN_FLOWS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // Code flows
    m.insert(
        "design-and-ship",
        include_str!("builtins/flows/code/design-and-ship.yaml"),
    );
    m.insert("grind", include_str!("builtins/flows/code/grind.yaml"));
    m.insert(
        "incident",
        include_str!("builtins/flows/code/incident.yaml"),
    );
    m.insert("pair", include_str!("builtins/flows/code/pair.yaml"));
    m.insert(
        "ship-roadmap",
        include_str!("builtins/flows/code/ship-roadmap.yaml"),
    );
    m.insert("ship", include_str!("builtins/flows/code/ship.yaml"));
    m.insert("start", include_str!("builtins/flows/code/start.yaml"));

    // Plan flows
    m.insert("publish", include_str!("builtins/flows/plan/publish.yaml"));
    m.insert(
        "research",
        include_str!("builtins/flows/plan/research.yaml"),
    );
    m.insert(
        "roadmap-expand",
        include_str!("builtins/flows/plan/roadmap-expand.yaml"),
    );
    m.insert(
        "roadmap-polish",
        include_str!("builtins/flows/plan/roadmap-polish.yaml"),
    );
    m.insert(
        "roadmap-reduce",
        include_str!("builtins/flows/plan/roadmap-reduce.yaml"),
    );

    m
});

static BUILTIN_DIRECTIONS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // Roles
    m.insert("ceo", include_str!("builtins/directions/roles/ceo.md"));
    m.insert(
        "designer",
        include_str!("builtins/directions/roles/designer.md"),
    );
    m.insert(
        "infra-engineer",
        include_str!("builtins/directions/roles/infra-engineer.md"),
    );
    m.insert(
        "product-engineer",
        include_str!("builtins/directions/roles/product-engineer.md"),
    );

    // Values
    m.insert("craft", include_str!("builtins/directions/values/craft.md"));
    m.insert("flow", include_str!("builtins/directions/values/flow.md"));
    m.insert("scale", include_str!("builtins/directions/values/scale.md"));

    m
});

static BUILTIN_OPS_PROMPTS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    m.insert(
        "commit_message",
        include_str!("builtins/ops/commit_message.md"),
    );
    m.insert("pr_message", include_str!("builtins/ops/pr_message.md"));
    m.insert(
        "release_notes",
        include_str!("builtins/ops/release_notes.md"),
    );
    m.insert("docker_gen", include_str!("builtins/ops/docker_gen.md"));
    m.insert("summarize", include_str!("builtins/ops/summarize.md"));

    m
});
