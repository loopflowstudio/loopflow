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

## Approach

Make **lfd session orchestration the single runtime primitive** and move prompt assembly into that path.

### 1) Step-only session config

Sessions always assemble prompts from step context. No raw `system_prompt` mode.

`SessionConfig` requires step context fields:

```rust
SessionConfig {
    step: String,
    repo_root: PathBuf,
    directions: Vec<String>,
    area: Option<String>,
    wave: Option<String>,
    message: Option<String>,
    model: Option<String>,
    cwd: Option<PathBuf>,
    max_turns: Option<u32>,
    yolo_mode: bool,
}
```

On the wire (HTTP API), these are flat JSON fields. Internally, the session manager validates `repo_root` exists and contains `.lf/` — fail loudly with a clear error if the path is wrong or stale.

### 2) Shared prompt assembly helper

Extract `build_step_prompt` logic from `lfd::executor::helpers` into a shared helper used by both wave execution and sessions.

The shared helper returns a narrower type than the current tuple:

```rust
struct PreparedPrompt {
    system_prompt: String,  // from format_context_prompt
    task_prompt: String,    // from format_task_prompt
    model: Option<String>,  // from step frontmatter
    cwd: PathBuf,
}
```

Session startup calls this helper, then passes `PreparedPrompt` to the harness.

### 3) Harness trait takes PreparedPrompt

Change `SessionHarness::start()` to take `PreparedPrompt` instead of `&SessionConfig`:

```rust
pub trait SessionHarness: Send + Sync {
    async fn start(&mut self, prompt: &PreparedPrompt) -> Result<()>;
    async fn send_input(&mut self, content: &str) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;
}
```

Both Claude and Codex harnesses update. The harness no longer needs to know about step context — it receives assembled prompts and runs them.

### 4) Wire Concerto start-design flow to inline sessions

Replace terminal launch with session launch:

- `StartWaveView` creates a `design` session (step-mode config)
- initial user text is sent as first turn
- `WaveChatView`/`ChatState` continues consuming session SSE stream

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep current split (lf runtime + lfd sessions) and only improve terminal launcher UX | Lowest short-term effort | Locks in duplicated behavior and keeps inline design blocked on ad hoc glue |
| Caller assembles prompts and passes `system_prompt` into sessions | Minimal lfd changes | Prompt correctness becomes caller-dependent; drift between callers is guaranteed |
| Rebuild sessions around `engine::agent` and drop harness abstraction | One unified subprocess model | Throws away working Claude/Codex event normalization and existing SSE persistence |
| Support both `raw` (flat system_prompt) and `step` session modes | Flexibility for ad hoc use cases | Two modes means two paths to maintain; the whole point is one canonical assembly path |

## Key decisions

1. **Sessions become the single orchestration boundary.**
   - Prevents a third runtime path from emerging.
   - Aligns with wave intent: *"Waves start with `lf design`, not configuration."*

2. **Prompt assembly happens inside the session runtime, not at callers.**
   - Enforces one canonical context assembly path.
   - Preserves wave intent consistency across Concerto, CLI, and wave runs.

3. **Step-only, no raw mode.**
   - One path. No branching. No escape hatch that drifts.

4. **Harness receives `PreparedPrompt`, not config.**
   - Clean separation: session manager owns context assembly, harness owns subprocess lifecycle.

5. **`repo_root` validated at session creation.**
   - Must exist and contain `.lf/`. Fail with a clear error, not silent empty context.

6. **Provider rollout is staged: Claude + Codex first; Gemini/OpenCode return explicit not-implemented errors.**
   - Ships user value now without blocking on all providers.

### Wild success signal

New users type a design prompt in Concerto and stay in one window while the design conversation runs.

### Wild failure to avoid

A partial bridge that only fixes StartWaveView while leaving wave executor and CLI on legacy runtime paths. That would reintroduce divergence within one release.

## Scope

- In scope:
  - Step-only session config with required step context fields
  - Shared prompt assembly helper (`PreparedPrompt`)
  - `SessionHarness::start()` trait change to take `PreparedPrompt`
  - `StartWaveView` migration from terminal launch to inline session

- Out of scope (fast follows):
  - `workspace_changed` session events + Concerto content refresh
  - Wave executor step runs routed through sessions
  - New database schema for wave content
  - Mandatory README section validation
  - Full Gemini/OpenCode provider harness implementations
  - General-purpose filesystem watching for wave docs

## Done when

1. `StartWaveView` no longer calls `TerminalLauncher.launchDesign`; it starts a `design` session and streams output inline.
2. Session create API requires step context fields, and lfd assembles prompt context server-side.
3. `SessionHarness::start()` takes `PreparedPrompt`.
4. Invalid `repo_root` at session creation returns a clear error.
5. Validation passes:
   - `cargo test --all`
   - `swift test --package-path swift`
