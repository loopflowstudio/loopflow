# OpenCode Context Injection

## Problem

Loopflow assembles context (area docs, directions, diffs, LOOPFLOW.md) into a temp file and injects it into each agent's session. Each agent uses a different mechanism: Claude uses `--append-system-prompt-file`, Codex uses `-c model_instructions_file=`, Gemini uses `GEMINI_SYSTEM_MD` env var. OpenCode needs the same treatment so `lf implement --agent opencode` gets full context.

## Approach

Use `OPENCODE_CONFIG_CONTENT` env var to inject both permission auto-approve and context file path in a single JSON object. This is already implemented in `launch_agent` at `agent.rs:282-303`.

The env var produces JSON like:
```json
{"permission":"allow","instructions":["/tmp/lf-context-abc123.md"]}
```

OpenCode merges this with its existing config at runtime. User-owned `opencode.json` is never modified.

### What's done

PR 01 already implements the unified `serde_json::Map` approach:
- `permission: "allow"` when `config.auto == true`
- `instructions: [path]` when `config.context_file.is_some()`
- Both keys coexist in a single JSON object
- Empty map → no env var set (interactive mode, no context)

### What's left

The env var construction happens inside `launch_agent`, which spawns a real subprocess. Current tests only cover `build_opencode_command` (CLI args). The env var logic is untested.

**Option A: Extract and test.** Pull the env var construction into a testable function like `build_opencode_env(config: &LaunchConfig) -> Option<String>`. Test it directly.

**Option B: Test via integration.** Keep the code in `launch_agent` and test via the existing golden prompt / parity test infrastructure.

Choose **Option A**. The function is pure (config in, JSON string out) and trivial to unit test. Matches how `build_opencode_command` is already extracted and tested for CLI args.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Write `AGENTS.md` to project root | OpenCode reads it automatically, no env var needed | Pollutes the user's repo with a temp file. `AGENTS.md` is user-owned. Race conditions if multiple runs overlap. |
| Modify `opencode.json` per-run | Direct config file manipulation | Invasive. Breaks if user edits config concurrently. Must restore after run. |
| Use `--instructions` CLI flag | Simpler than env var | Flag doesn't exist in opencode's CLI. `OPENCODE_CONFIG_CONTENT` is the documented runtime config mechanism. |
| Separate env vars for permission and instructions | Clearer separation of concerns | OpenCode only supports one config override mechanism: `OPENCODE_CONFIG_CONTENT`. Can't split it. |

## Key decisions

- **Single env var for all config overrides.** OpenCode's `OPENCODE_CONFIG_CONTENT` merges with base config. One insertion point, not two. Follows the wave's "minimal config surface" principle.
- **Absolute paths for temp files.** The `instructions` array accepts relative or absolute paths. Temp files get absolute paths to avoid CWD sensitivity.
- **No env var when empty.** If neither `auto` nor `context_file` is set (interactive mode), don't set the env var at all. Clean environment for the TUI.
- **Extract env builder for testability.** `build_opencode_env` returns `Option<String>` — testable without spawning a process.

## Scope

- In scope: Extract `build_opencode_env`, unit tests for all combinations (auto + context, auto only, context only, neither)
- Out of scope: Testing that OpenCode actually reads the env var (requires opencode installed), `AGENTS.md` fallback, interactive mode context

## Done when

```bash
cargo test -p loopflow -- opencode_env
cargo fmt --check
cargo clippy -- -D warnings
```

Tests cover:
- `auto=true, context_file=Some(...)` → JSON with both keys
- `auto=true, context_file=None` → JSON with permission only
- `auto=false, context_file=Some(...)` → JSON with instructions only
- `auto=false, context_file=None` → `None` (no env var)
