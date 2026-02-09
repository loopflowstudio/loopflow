---
status: todo
phase: 1
---
# OpenCode Command Builder and Launch

## Problem

Loopflow dispatches to three coding agents (Claude, Codex, Gemini) but not OpenCode. Users who prefer opencode — or want to compare agents — must run it manually outside loopflow's prompt assembly and streaming pipeline. This is the foundation PR: command building, dispatch, and launch integration.

## Approach

Follow the existing builder pattern exactly. Add `build_opencode_command()` alongside the three existing builders, wire it into `build_model_command()` dispatch, and handle opencode's permission env var in `launch_agent`.

The key insight: opencode's CLI is closer to Codex than Claude. Like Codex, it has a separate subcommand for non-interactive mode (`opencode run` vs `codex exec`). Like Gemini, its runtime config is injected via env var rather than CLI flags. Unlike all three, permissions are controlled through `OPENCODE_CONFIG_CONTENT` — a JSON env var that merges with the user's config at runtime.

### Command builder (`agent.rs`)

```rust
pub fn build_opencode_command(config: &LaunchConfig) -> Vec<String> {
    let mut cmd = if config.auto {
        vec!["opencode".to_string(), "run".to_string()]
    } else {
        vec!["opencode".to_string()]
    };

    if let Some(ref variant) = config.model_variant {
        cmd.push("--model".to_string());
        cmd.push(variant.clone());
    }

    if config.auto && config.stream {
        cmd.push("--format".to_string());
        cmd.push("json".to_string());
    }

    cmd
}
```

### Dispatch (`agent.rs`)

Add `"opencode"` arm to `build_model_command`:

```rust
match model {
    "claude" => build_claude_command(config),
    "codex" => build_codex_command(config),
    "gemini" => build_gemini_command(config),
    "opencode" => build_opencode_command(config),
    _ => build_claude_command(config),
}
```

### Launch integration (`agent.rs`)

In `launch_agent`, after the Gemini env var block, add opencode's `OPENCODE_CONFIG_CONTENT`. Build it as a `serde_json::Map` from the start — PR 03 will add `instructions` to the same env var, and starting with structured JSON avoids a refactor later:

```rust
if backend == "opencode" {
    let mut oc_config = serde_json::Map::new();
    if config.auto {
        oc_config.insert(
            "permission".into(),
            serde_json::Value::String("allow".into()),
        );
    }
    if let Some(ref context_file) = config.context_file {
        oc_config.insert(
            "instructions".into(),
            serde_json::json!([context_file.to_string_lossy()]),
        );
    }
    if !oc_config.is_empty() {
        cmd.env(
            "OPENCODE_CONFIG_CONTENT",
            serde_json::Value::Object(oc_config).to_string(),
        );
    }
}
```

This collapses PRs 01 and 03's env var logic into a single block. The `context_file` path won't fire until callers pass it (PR 03 enables that), but the code is ready. No dead code — the branch is reachable today if someone passes `context_file` manually.

### Re-exports (`mod.rs`)

Add `build_opencode_command` to the `pub use agent::{...}` re-export.

### Unit tests

Four tests matching the pattern of existing agent tests:

1. **Auto mode**: `LaunchConfig { auto: true }` → `["opencode", "run"]`
2. **Model variant**: `model_variant: Some("anthropic/claude-sonnet-4-5")` → includes `--model`
3. **Interactive**: default config → `["opencode"]`
4. **Streaming**: `auto + stream` → includes `--format json`

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Hardcoded `OPENCODE_CONFIG_CONTENT` string in PR 01, refactor in PR 03 | Simpler PR 01, but creates refactor churn | The structured JSON approach is 3 more lines and eliminates the PR 03 refactor entirely |
| Use `skip_permissions` field + CLI flag | Would match Claude/Gemini pattern | OpenCode has no permission CLI flag — it's env-var-only. Fighting the tool's design |
| Skip `context_file` in launch_agent until PR 03 | Smaller diff | The env var builder handles both concerns naturally. Splitting it is artificial |

## Key decisions

- **Unified env var builder from PR 01.** The roadmap says "PR 01 sets permission, PR 03 refactors to add instructions." Building it right the first time is simpler. Per the wave's "Minimal config surface" principle — one code path for one env var.
- **No `context_file` in `build_opencode_command`.** Unlike Claude (CLI flag) and Codex (config override), opencode's context injection is an env var concern, not a command-line concern. It belongs in `launch_agent`, not the command builder. This follows the Gemini pattern exactly.
- **`serde_json` for env var construction.** Hand-building JSON strings is fragile. The crate is already a dependency.
- **No default model variant.** Per the wave design: "like codex, let opencode use its own config." `parse_model("opencode")` already returns `None` for the variant via the `_ => None` fallback in `config.rs`.

## Scope

- In scope: `build_opencode_command`, dispatch, `launch_agent` env var, re-export, unit tests
- Out of scope: stream parsing (PR 02), standalone context injection wiring (PR 03), docs (PR 04)

## Done when

```bash
cargo test -p loopflow -- opencode
cargo fmt --check
cargo clippy -- -D warnings
```

All pass. `build_opencode_command` produces correct CLI args for auto, interactive, model variant, and streaming modes. `launch_agent` sets `OPENCODE_CONFIG_CONTENT` with permission and context_file when applicable.
