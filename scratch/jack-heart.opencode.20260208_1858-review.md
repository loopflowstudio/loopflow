# OpenCode Command Builder and Launch — Review

## What was implemented

PR 01 of the OpenCode agent integration. Adds `build_opencode_command()` alongside the existing Claude/Codex/Gemini builders, wires it into `build_model_command()` dispatch, and sets `OPENCODE_CONFIG_CONTENT` env var in `launch_agent` for permission auto-approve and context injection.

### Files changed

| File | Change |
|------|--------|
| `rust/loopflow/src/engine/agent.rs` | `build_opencode_command()`, dispatch arm, `launch_agent` env var block, 4 unit tests |
| `rust/loopflow/src/engine/mod.rs` | Re-export `build_opencode_command` |
| `rust/loopflow/src/engine/config.rs` | Doc comment update for `parse_model` (opencode -> None) |
| `rust/loopflow/src/engine/stream.rs` | Doc comment updates (mentions OpenCode in module/struct docs) |
| `roadmap/opencode/` | README + PRs 02-04 design docs |
| `scratch/` | Design doc, this review |

## Key choices

1. **Unified env var builder from PR 01.** The `OPENCODE_CONFIG_CONTENT` block builds a `serde_json::Map` that handles both `permission` and `instructions` in one code path. This avoids a refactor in PR 03 — the `context_file` branch is reachable today if callers pass it.

2. **No `context_file` in `build_opencode_command`.** OpenCode's context injection is an env var concern (like Gemini's `GEMINI_SYSTEM_MD`), not a CLI flag concern (like Claude's `--append-system-prompt-file`). The code follows the Gemini pattern.

3. **No default model variant.** `parse_model("opencode")` returns `None` via the `_ => None` fallback — same as Codex. OpenCode manages its own model config.

4. **`serde_json` for JSON construction.** Already a dependency. Avoids hand-building JSON strings.

## How it fits together

`build_opencode_command` produces the CLI args (`opencode run --model X --format json`). `launch_agent` adds the env var. The split mirrors Gemini exactly: Gemini builds CLI args in `build_gemini_command` and sets `GEMINI_SYSTEM_MD` in `launch_agent`.

The dispatch chain: `build_agent_command` → `parse_model` → `build_model_command` → `build_opencode_command`. `launch_agent` handles the subprocess spawn with env vars.

## Risks and bottlenecks

- **`OPENCODE_CONFIG_CONTENT` behavior is based on docs, not live testing.** If the env var doesn't merge correctly with user config, auto mode could hang on permission prompts. PR 04 should include a live smoke test.
- **`opencode run` exit behavior unknown.** If it exits non-zero on permission denial rather than hanging, the current error handling works fine. If it hangs, there's no timeout.

## What's not included

- Stream parsing (PR 02) — opencode's NDJSON output goes through as `Passthrough` for now
- Context injection wiring from callers (PR 03) — the code is ready but no caller passes `context_file` for opencode yet
- README/docs updates (PR 04)
- `check_cli_available("opencode")` — works already via the generic `--version` check

## Test results

```
cargo test -p loopflow -- opencode     # 4 passed
cargo test -p loopflow                 # 198 passed, 1 failed (postgres_store_suite — Docker, unrelated)
cargo fmt --check                      # clean
cargo clippy -p loopflow -- -D warnings # clean
```
