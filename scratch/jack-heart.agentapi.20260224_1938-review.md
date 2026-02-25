# Branch Review: agentapi naming cleanup + prompt convergence

## What was implemented

Four naming cleanups and one structural simplification, all from the `naming-cleanup` design doc:

1. **`Agent` → `AgentRun`** — the DB record type in `lfd/types/agent.rs` no longer collides with `engine/agent.rs`. Parallels `WaveRun`. Store trait methods, row mapping, executor call sites, and HTTP routes all updated.

2. **`LaunchConfig` → `AgentConfig`** — neutral about execution mode (fire-once vs session). Renamed across engine, CLI, executor, harness, and ops modules (17 files).

3. **`provider` → `harness`** — sessions now use "harness" consistently. `Session.provider` → `Session.harness`, `CreateSessionParams.provider` → `.harness`, `HarnessProvider` → `HarnessKind`, `SessionHarness` trait → `Harness`. DB migration renames the column. `AuthProvider` and `provider_session_id` intentionally kept (different concepts).

4. **Prompt prep collapsed to two layers** — deleted `lfd/prompt.rs` (hollow middle layer). Both callers (`build_step_prompt` in executor/helpers and `prepare_session_prompt` in sessions/mod) now call `engine::launch::prepare_launch_prompt` directly.

5. **`ClaudeArgs` extraction** — shared command-building struct between `engine::agent::build_claude_command` (one-shot) and `lfd::sessions::harness::claude::build_args` (session). Eliminates duplicated flag logic for model, system prompt, max turns, permissions, and cwd.

Cross-language updates: Python client (`models.py`, `client.py`, `api.py`), Swift models (`AgentSession.swift`, `LocalWaveService.swift`, `WaveServiceProtocol.swift`), and all test files updated to match.

## Key choices

- **DB migration for column rename** rather than keeping `provider` in SQL with a comment. SQLite supports `ALTER TABLE RENAME COLUMN` since 3.25.0. Clean rename, no compatibility shim.
- **`ClaudeArgs` as a struct, not a builder** — it's simple enough that direct field assignment is clearer than a builder pattern. `to_args()` converts to `Vec<String>`.
- **`Harness` trait (not `AgentHarness`)** — the module path (`sessions::harness::Harness`) provides sufficient context. Shorter name, less stutter.
- **`HarnessKind` enum kept** — rather than inlining dispatch, it serves as the registry of known harnesses and provides `is_known()` / `is_implemented()` for error classification.

## How it fits together

```
engine/agent.rs     AgentConfig, ClaudeArgs, launch_agent()
engine/launch.rs    prepare_launch_prompt() — shared prompt assembly
                         ↑                        ↑
lf/commands/run.rs      lfd/executor/helpers.rs   lfd/sessions/mod.rs
(CLI one-shot)          (wave step execution)     (interactive sessions)
                                                       ↓
                                              harness/{claude,codex}/
                                              Harness trait → ClaudeArgs
```

Both the engine one-shot path and the session harness path share `ClaudeArgs` for Claude flag construction and `AgentConfig` for prompt/model/cwd configuration.

## Risks and bottlenecks

- **Column rename on large SQLite DBs** — `ALTER TABLE RENAME COLUMN` is metadata-only in modern SQLite, not a table rebuild. No data risk, but verify on production DBs if any are >1GB.
- **`provider_session_id` naming** — kept as-is because it refers to the external agent's session ID. Could cause brief confusion given the "kill provider" rename, but the design doc is explicit about this exception.

## What's not included

- **Engine harness extraction** — the session harness (`Harness` trait) still lives in `lfd/sessions/harness/`. Extracting it into `engine/` for shared use by CLI and daemon is the next runtime convergence step.
- **Interactive wave routing** — `interactive == true` steps still don't auto-launch through the session path during wave execution.
- **Codex/Gemini/OpenCode harness implementations** — only Claude has a working session harness. Others return `HarnessNotImplemented`.

## Validation

All test suites pass:
- `cargo fmt --all -- --check` ✓
- `cargo clippy --all-targets -- -D warnings` ✓
- `cargo test --all` ✓ (skipping Docker socket-dependent tests)
- `uv run pytest python/tests/` — 47 passed ✓
- `swift test --package-path swift` — 143 passed ✓
