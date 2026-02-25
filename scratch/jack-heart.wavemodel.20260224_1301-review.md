# Review: wavemodel agent harness

## What was implemented

Four phases of the wavemodel wave shipped on this branch:

1. **Launch config split** — The monolithic `LaunchConfig` was decomposed into three purpose-specific types: `LaunchConfig` (prompt assembly output), `ProcessConfig` (subprocess execution behavior), and `AgentCapabilities` (provider feature flags). All 20+ call sites updated consistently.

2. **Server-side prompt assembly** — New `lfd::prompt::prepare_step_prompt()` centralizes prompt building. Sessions now receive step-level intent (`step`, `repo_root`, `directions`, `area`, `wave`) instead of raw system prompts. The session manager validates `repo_root` contains `.lf/` and that `cwd` resolves inside it via canonical path comparison.

3. **Concerto design-first onboarding** — `StartWaveView` collects a wave name and launches an inline design chat (no terminal detour). `WaveDetailPanel` surfaces Vision, Goals, Risks, and Roadmap parsed from wave READMEs via the new `WaveContentParser`. `WaveSchema.swift` and schema-first setup code deleted.

4. **Session harness improvements** — Claude and Codex harnesses now accept `LaunchConfig` for startup and seed the task prompt on first turn only. Provider session IDs are captured and applied for resume. New error variants (`ProviderNotImplemented`, `InvalidConfig`, `InvalidRepoRoot`) give clear feedback.

## Key choices

| Decision | Why | Alternative rejected |
|----------|-----|---------------------|
| Three config types instead of one | Prevents mixing prompt intent with process behavior with provider capabilities | Single struct with optional fields (what we had) — hard to reason about which fields apply where |
| `prepare_step_prompt()` in `lfd::prompt` | Shared between executor and sessions, close to where prompt assembly happens | Putting it in `engine::prompt` — that module doesn't know about `lfd` concerns (summaries, store) |
| `WaveContentParser` as a separate service | Keeps parsing logic testable without SwiftUI dependencies | Inline parsing in views — untestable, repeated across views |
| `step: "design"` default for all chat tabs | Acceptable interim until per-tab step routing is specified | Block chat until step selection — worse UX for design-first flow |

## How it fits together

Sessions are the orchestration boundary. A caller sends `CreateSessionParams` with step-level intent → `SessionManager` validates paths, calls `prepare_step_prompt()` to build a `LaunchConfig`, then hands it to the provider-specific harness for startup. The harness bridges provider events back through a broadcast channel → event bridge persists them → SSE streams them to clients.

On the Swift side, `RepoState.chatState(for:)` creates a `ChatState` with `AgentSessionConfig` containing step/repo/directions/area/wave. `ChatState` calls `createSession` on the wave service, which POST /sessions to lfd. `WaveContentParser` reads wave READMEs independently for the detail panel.

## Risks and bottlenecks

- **Runtime drift**: The wave executor still builds launch configs inline rather than through `prepare_step_prompt()`. Until convergence lands, the two paths can diverge on prompt assembly logic.
- **Stale wave content**: No filesystem watcher — content loads on wave selection or status change. Inline design sessions modify READMEs but Concerto won't reflect changes until re-navigation.
- **Markdown parsing on main thread**: `WaveContentParser` does file I/O synchronously. Fine for small READMEs, could hitch on large files. Not urgent — wave READMEs are small by convention.
- **Chat tab step routing**: All tabs default to `step: design`. Appropriate for now; needs per-tab routing when more step types become relevant in Concerto.

## What's not included

- Runtime convergence (executor → session path) — tracked in wave README as future work
- Filesystem watcher for wave content refresh — tracked as risk in wave README
- Per-tab step routing in Concerto chat — tracked as "not here" in wave README
- Concerto UI tests via xcodegen/xcodebuild (Swift package tests cover models and state; UI tests require Xcode project generation)

## Test results

| Suite | Result |
|-------|--------|
| `cargo fmt --check` | Pass |
| `cargo clippy -- -D warnings` | Pass |
| `cargo test --all` | 447 pass, 2 fail (pre-existing Docker socket tests, unrelated) |
| `uv run pytest python/tests/` | 47 pass |
| `swift test --package-path swift` | 129 pass |

## Wave alignment

This branch ships all four phases of the wavemodel wave. The wave README reflects shipped status for all Goals and Metrics. Risks are annotated with evidence from implementation. "Not here" scope boundaries are clearly stated.
