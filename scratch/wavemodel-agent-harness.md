# Unified Agent Harness

Merge lf's agent running (prompt assembly, subprocess launch, I/O) with lfd's session infrastructure (lifecycle, event streaming, SSE) into one concept.

## Problem

Two parallel implementations of "run a coding agent" exist:

**lf** assembles prompts (repo docs, area docs, step prompt, direction, clipboard) and launches agents by shelling out to `claude`, `codex`, etc. It manages stdin/stdout directly. Rich context, no server infrastructure.

**lfd sessions** create provider sessions via harnesses (Claude, Codex), manage lifecycle (starting → active → ending → ended), stream structured events (turns, items, text deltas) over SSE, and persist to storage. Server infrastructure, but no prompt assembly — sessions get a raw `system_prompt` string and that's it.

The result: Concerto can't run `lf design` inline. It can show a chat session (WaveChatView + ChatState), and it can assemble prompts (lf engine), but there's no path from "run step X" to "create a session with step X's assembled prompt and stream events back." The NUX works around this by launching an external terminal.

## What unification means

One agent harness interface that:

1. Accepts a step (or assembled prompt) and provider config
2. Uses the loopflow engine for prompt assembly (repo docs, area, direction, step prompt, clipboard)
3. Launches the provider through lfd's session infrastructure (lifecycle, events, persistence)
4. Streams structured events back to clients (Concerto, CLI, API consumers)

The harness is provider-agnostic. Four providers implement it:

| Provider | Status |
|----------|--------|
| Claude (claude -p) | Implemented in lfd today |
| Codex (codex --json-rpc) | Implemented in lfd today |
| Gemini CLI | Not implemented |
| OpenCode | Not implemented |

Gemini and OpenCode return "not implemented" initially but the trait accommodates them. No provider-specific assumptions in the interface.

## What this enables

- **Concerto inline sessions**: StartWaveView creates a session with step `design`, shows WaveChatView. No terminal launch.
- **`lf -i` as a server operation**: Interactive steps run through lfd, get lifecycle management and event persistence for free.
- **Wave runs through sessions**: A wave step becomes a session. The run coordinator creates sessions instead of spawning subprocesses directly.
- **Unified provider abstraction**: One place to add a new agent provider, not two.

## Current state

lfd session harnesses (Rust):
- `HarnessFactory` trait with `create()` → `Box<dyn Harness>`
- `Harness` trait: `start(session_id, config, input_rx)` → `EventStream`
- `ClaudeHarness`: spawns `claude -p --resume`, parses NDJSON
- `CodexHarness`: spawns `codex`, JSON-RPC over stdio
- `SessionConfig`: `model`, `cwd`, `system_prompt`, `max_turns`, `yolo_mode`

lf agent running (Rust):
- Prompt assembly: `PromptAssembler` builds the full prompt from step, area, direction, repo docs, clipboard
- Agent launch: spawns provider CLI directly, pipes assembled prompt as system prompt or first message
- No lifecycle management, no event streaming, no persistence

Concerto integration surface (from Phase 03 NUX):
- `TerminalLauncher.launchDesign(prompt:repoPath:)` — current interim path, opens external terminal with `lf design -c '<prompt>'`
- `StartWaveView` collects a design prompt and calls `launchDesign` — this becomes the inline session entry point
- `WaveContentParser` reads wave README sections and roadmap from disk — the harness should trigger content refresh when a design session modifies wave files
- `WaveViewModel.content: WaveContent?` — cached on-demand, no filesystem watcher. Real-time updates during inline sessions will need the harness to push refresh signals.
- Content loading is on main-actor paths. If inline sessions produce rapid README updates, consider async parsing.

The gap: `SessionConfig.system_prompt` is a flat string. The loopflow engine's prompt assembly produces a structured prompt. The harness needs to accept either a pre-assembled prompt or a step specification and do the assembly itself.

## Design questions

- Should the harness accept a step name and assemble the prompt in-process, or should the caller assemble and pass the result? (Leaning: harness does assembly — keeps the interface simple and ensures prompt assembly is always correct.)
- How does the unified harness relate to wave run coordination? Does the run coordinator become a session orchestrator?
- What's the right Rust trait boundary? Current `Harness` trait is provider-specific (Claude, Codex). The unified harness adds a layer above that handles prompt assembly before delegating to the provider harness.
- How do we handle providers that don't support structured events? (Gemini CLI and OpenCode may have different output formats.) The event normalization that Claude and Codex harnesses do today becomes the pattern.
- How should the harness notify Concerto when a session modifies wave content files? Phase 03's content loading is pull-based (on selection/status change). Inline sessions need push-based refresh — either the harness emits file-change events, or Concerto watches the wave directory during active sessions.
