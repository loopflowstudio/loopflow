# Naming Cleanup

Tighten naming across the agent execution stack before runtime convergence. These renames reduce confusion, remove redundant concepts, and make the codebase legible before the harness extraction refactor.

## Changes

### 1. `Agent` (DB record) → `AgentRun`

The `Agent` struct in `lfd/types/agent.rs` is a persistent record of a step execution — id, step, pid, status, model. It collides with `engine/agent.rs` which is about building and launching agent processes.

`AgentRun` parallels `WaveRun`. A wave has runs; a run has agent runs.

**Blast radius:** ~20 store methods across trait + sqlite + postgres impls, row mapping, executor call sites, HTTP routes (status filtering only — no direct serialization). SQL table stays `agents`. Localized to `lfd/`.

**What moves:**
- `Agent` → `AgentRun` in `lfd/types/agent.rs`
- `Agent::new()` → `AgentRun::new()`
- Re-export in `lfd/types/mod.rs`
- Store trait methods: signatures change type, names stay (`start_agent`, `get_agent`, etc. — the "agent" in method names refers to the concept, not the struct)
- `build_agent_for_step()` return type
- `AgentStatus` stays — it's already correct

### 2. `LaunchConfig` → `AgentConfig`

`LaunchConfig` is used by both fire-once execution (`launch_agent`) and long-lived sessions (`SessionHarness.start`). "Launch" implies fire-once. The struct is really "what the agent needs to know": system prompt, task prompt, model, cwd, permissions.

`AgentConfig` is neutral about execution mode.

**Blast radius:** 17 files. Definition in `engine/agent.rs`, re-export in `engine/mod.rs`, used across CLI commands, executor helpers, prompt module, session harnesses (trait + claude + codex), ops modules (release, lint, messages, rebase), and integration tests.

**What moves:**
- `LaunchConfig` → `AgentConfig` in `engine/agent.rs`
- `PreparedLaunchPrompt.launch` field → `PreparedLaunchPrompt.agent_config` (or just `.config`)
- All imports and usage sites (mechanical find-replace)
- `LaunchResult` stays — it *is* about a launch (fire-once subprocess result)

### 3. Kill "provider" terminology

"Provider" is used for Claude, Codex, OpenCode — but these are coding agents, not providers. The actual providers (Anthropic, OpenAI, Google) are behind these agents. OpenCode itself can call multiple LLM providers. The word is misleading.

Loopflow already has the right concept: **harness**. A harness is how you talk to a specific coding agent. `ClaudeHarness` spawns `claude -p --resume`. `CodexHarness` spawns `codex --app-server`. Clean.

**Renames:**
- `HarnessProvider` enum → `HarnessKind` (or just inline — it's only used for dispatch)
- `SessionHarness` trait → `Harness`
- `Session.provider` field → `Session.harness` (DB column: `harness`)
- `CreateSessionParams.provider` → `CreateSessionParams.harness`
- `ProviderNotImplemented` error → `HarnessNotImplemented`
- `provider_session_id` stays — this is genuinely the external agent's session id

**Does NOT change:**
- `AuthProvider` — this is about auth backends, not agent harnesses
- `ProviderSessionId` event — it carries the external agent's id
- `ClaudeHarness`, `CodexHarness` — already correct

**DB migration:** Rename `provider` column to `harness` in `sessions` table (or alias — column rename in sqlite requires table rebuild).

### 4. Collapse prompt prep to two layers

Three layers today:
```
build_step_prompt()        lfd/executor/helpers.rs
  → prepare_step_prompt()  lfd/prompt.rs           (hollow middle)
    → prepare_launch_prompt()  engine/launch.rs     (real work)
```

The middle layer (`prepare_step_prompt` + `PrepareStepPromptConfig`) loads config, fetches summary, and passes through. Both callers can do this directly.

**After:**
```
build_step_prompt()           lfd/executor/helpers.rs  (wave execution)
  → prepare_launch_prompt()   engine/launch.rs

prepare_session_prompt()      lfd/sessions/mod.rs      (session creation)
  → prepare_launch_prompt()   engine/launch.rs
```

Two callers, each calling the engine directly. Config loading and summary fetching happen at the call site. `prepare_step_prompt`, `PrepareStepPromptConfig`, and `lfd/prompt.rs` are deleted.

**Callers today:**
1. `build_step_prompt()` in `lfd/executor/helpers.rs` — wave step execution
2. `prepare_session_prompt()` in `lfd/sessions/mod.rs` — interactive session creation

Both inline the config load + summary fetch + `LaunchPromptInput` construction. The parity test moves to `engine/launch.rs` tests (where it already has coverage).

## Sequencing

1. **`Agent` → `AgentRun`** — smallest blast radius, no cross-module boundaries
2. **`LaunchConfig` → `AgentConfig`** — mechanical rename, touches more files but no logic changes
3. **Kill "provider"** — touches sessions module + DB migration
4. **Collapse prompt prep** — deletes code, changes call sites

Each is independently shippable. 1 and 2 can be parallel. 3 and 4 are independent of each other but both benefit from landing after 1+2 to avoid churn.

## Validation

Standard suite — no new test coverage needed, these are renames:
```
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
uv run pytest python/tests/
swift test --package-path swift
```
