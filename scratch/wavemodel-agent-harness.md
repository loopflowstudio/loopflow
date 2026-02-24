# Unified Agent Harness

## Problem

Loopflow currently has two agent runtimes that duplicate responsibility and diverge behavior:

- `lf` assembles rich step context (repo docs, area docs, direction, clipboard, wave context) and launches provider CLIs directly.
- `lfd` sessions provide lifecycle/state persistence + SSE events, but only accept flat `system_prompt` strings.

Who benefits from fixing this:

- **Concerto users**: can run `design` inline instead of bouncing to a terminal.
- **CLI users**: interactive runs gain persisted history and resumable session semantics.
- **Maintainers**: one execution path instead of two drifting implementations.

Why now:

- Phase 03 shipped design-first Concerto UX, but the critical path still detours through `TerminalLauncher.launchDesign(...)`.
- Wave content now has first-class UI surfaces (Vision/Goals/Risks/Roadmap), so inline sessions need a way to trigger timely content refresh.

## Approach

Make **lfd session orchestration the single runtime primitive** and move prompt assembly into that path.

### 1) Add step-aware session input (not just flat system prompts)

Extend session config with an explicit prompt source:

- `raw` (existing behavior): caller supplies `system_prompt`
- `step` (new): caller supplies `step`, `repo_root/cwd`, `direction[]`, `area`, `wave`, and optional initial `message`

The session runtime assembles context using existing engine prompt code (`gather_context`, `trim_context_with_breakdown`, `format_context_prompt`, `format_task_prompt`) before provider harness startup.

### 2) Introduce a unified session runner above provider harnesses

Add a thin runtime layer in `lfd::sessions` that:

1. Resolves step config into a prepared prompt package (task text + system/context text + cwd)
2. Normalizes provider launch config
3. Delegates turn streaming to the existing Claude/Codex harnesses unchanged

Key implementation choice: extract `build_step_prompt` logic from `lfd::executor::helpers` into a shared helper used by both wave execution and sessions.

### 3) Emit workspace-change events from session turns

Add a new session event for UI cache invalidation:

- `workspace_changed { paths: [] }`

Emit it when file edits/diff updates include `wave/<name>/README.md` or `wave/<name>/NN-*.md`. Concerto listens and calls `loadWaveContent` for the active wave.

This avoids adding a filesystem watcher and keeps updates tied to persisted session events.

### 4) Wire Concerto start-design flow to inline sessions

Replace terminal launch with session launch:

- `StartWaveView` creates a `design` session (step-mode config)
- initial user text is sent as first turn
- `WaveChatView`/`ChatState` continues consuming session SSE stream

### 5) Move wave step execution onto sessions

Wave executor creates one session per step (`wave_run_id` attached), sends step task input, and waits for terminal turn status.

Result: wave runs, CLI interactive runs, and Concerto interactive runs share the same lifecycle/event model.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep current split (lf runtime + lfd sessions) and only improve terminal launcher UX | Lowest short-term effort | Locks in duplicated behavior and keeps inline design blocked on ad hoc glue |
| Caller assembles prompts and passes `system_prompt` into sessions | Minimal lfd changes | Prompt correctness becomes caller-dependent; drift between callers is guaranteed |
| Rebuild sessions around `engine::agent` and drop harness abstraction | One unified subprocess model | Throws away working Claude/Codex event normalization and existing SSE persistence |

## Key decisions

1. **Sessions become the single orchestration boundary.**
   - Prevents a third runtime path from emerging.
   - Aligns with wave intent: *"Waves start with `lf design`, not configuration."*

2. **Prompt assembly happens inside the harness runtime, not at callers.**
   - Enforces one canonical context assembly path.
   - Preserves wave intent consistency across Concerto, CLI, and wave runs.

3. **Use event-driven content refresh (`workspace_changed`) instead of file watching.**
   - Matches existing session event architecture and persistence.
   - Supports wave goal: *"Wave content (Vision, Goals) is visible in the Concerto UI"* with timely updates.

4. **Provider rollout is staged: Claude + Codex first; Gemini/OpenCode return explicit not-implemented errors.**
   - Ships user value now without blocking on all providers.

### Wild success signal

New users type a design prompt in Concerto, stay in one window, and see wave Vision/Roadmap update while the design conversation runs.

### Wild failure to avoid

A partial bridge that only fixes StartWaveView while leaving wave executor and CLI on legacy runtime paths. That would reintroduce divergence within one release.

## Scope

- In scope:
  - Step-aware session config + shared prompt assembly helper
  - Unified session runtime layer above provider harnesses
  - `workspace_changed` session events + Concerto refresh handling
  - `StartWaveView` migration from terminal launch to inline session
  - Wave executor step runs routed through sessions

- Out of scope:
  - New database schema for wave content
  - Mandatory README section validation
  - Full Gemini/OpenCode provider harness implementations
  - General-purpose filesystem watching for wave docs

## Done when

1. `StartWaveView` no longer calls `TerminalLauncher.launchDesign`; it starts a `design` session and streams output inline.
2. Session create API accepts step-mode config, and lfd assembles prompt context server-side.
3. Wave executor steps run through session lifecycle/events rather than direct `engine::agent::launch_agent` calls.
4. Editing `wave/<name>/README.md` during a live design session triggers `workspace_changed` and refreshes the visible Wave content panel.
5. Validation passes:
   - `cargo test --all`
   - `swift test --package-path swift`
