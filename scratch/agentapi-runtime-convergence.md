# Runtime Convergence

Unify the two agent execution paths (`lf` engine and `lfd` sessions) into one runtime. Interactive `lf` commands use the session API and Concerto's chat UI instead of spawning a terminal.

## What exists after this

One execution path for all agent work:

- **Headless runs** (wave executor, `lf implement --auto`): session API in batch mode, no UI
- **Interactive runs** (`lf design`, `lf explore`, `lf review`): session API, Concerto chat UI

`lf` interactive commands create a session via lfd, open Concerto to that session, and the user works through the chat interface. No terminal agent process. The ghostty/warp terminal launch path is removed for interactive steps.

The wave executor routes step runs through the same session orchestration that interactive sessions use. `build_step_prompt()` and `prepare_step_prompt()` merge into one function. `LaunchConfig` flows through `SessionManager` for both paths.

## What to unify

### Provider layer

`engine/agent.rs` builds CLI commands and spawns subprocesses directly. `sessions/harness/*.rs` does the same thing with event streaming. Both know how to launch Claude, Codex, etc. Merge these:

- One provider module that owns process spawning, argument construction, and event translation
- `SessionHarness` becomes the shared execution primitive
- `launch_agent()` becomes "create a session, send one turn, wait for completion, return stdout/stderr"
- `build_claude_command` / `build_codex_command` move into their respective harnesses

### Prompt assembly

`build_step_prompt()` (executor/helpers.rs) and `prepare_step_prompt()` (lfd/prompt.rs) do the same work with different signatures. Merge into one function that returns `LaunchConfig`.

### Interactive `lf` → Concerto

`lf design` currently spawns `claude -p` in a terminal. Change to:

1. `lf design` creates a session via lfd (same as Concerto's StartWaveView)
2. `lf` opens Concerto and navigates to that session
3. User interacts through Concerto's chat UI
4. `lf` exits after launching (or optionally waits for session end)

This means `lf` interactive commands become thin launchers: create session + open UI.

### Wave executor

Route wave executor step runs through `SessionManager`:

1. Executor creates a session with `auto: true`
2. Session harness runs the step
3. Events are persisted (same store as interactive sessions)
4. Executor reads completion status from session, not subprocess exit code

This gives wave runs the same event history and replay capability as interactive sessions.

## What doesn't change

- `lf` headless/auto mode still runs without Concerto (sessions are the runtime, but no UI required)
- Provider harness behavior (Codex JSON-RPC, Claude NDJSON) stays the same
- Session API endpoints stay the same
- Event model stays the same

## Risks

- **Latency**: routing through lfd adds overhead vs direct subprocess spawn. For headless runs this is acceptable. For interactive, the session API is already the path.
- **lfd dependency**: `lf` interactive now requires lfd running. Currently `lf design` works standalone. Need graceful fallback or clear error.
- **Concerto dependency**: interactive sessions need Concerto installed. Terminal fallback for environments without it?

## Done when

- `build_step_prompt` and `prepare_step_prompt` are one function
- Wave executor creates sessions instead of calling `launch_agent` directly
- `lf design` opens Concerto instead of spawning a terminal agent
- `engine/agent.rs` command builders live inside harness modules
- One set of provider conformance tests covers both `lf` and session paths
