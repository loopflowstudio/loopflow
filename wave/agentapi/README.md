# Agent API

Unified session API for interactive coding agents. lfd spawns and manages harness processes (Codex, Claude, OpenCode), translates events into a canonical model, persists everything for replay. Concerto and future clients connect via HTTP/SSE.

`lf` headless runs are unchanged. Interactive `lf` commands (`design`, `explore`, `review`, `refine`) will move to the session API + Concerto UI (runtime convergence — active in scratch/).

## Vision

lfd exposes a harness-agnostic session API. Clients create sessions, send input, subscribe to events, and end sessions. lfd owns the agent lifecycle — spawning processes, translating events, persisting state. Clients connect and disconnect freely; sessions survive reconnect.

### Not here

- Approval routing / permission management (container safety instead)
- Advanced Concerto UI (tool visualization, diff views, multi-panel layouts)
- Multi-agent per step (parallel agents within one interactive step)
- Cross-wave session sharing

## Goals

- Harness-agnostic: same client code works regardless of which agent runs the session
- Harness-first: build working harnesses end-to-end before abstracting (protocol emerged from real harness behavior)
- lfd owns the session lifecycle — Concerto is a thin client, agent processes survive close/reopen
- Turn+item event model with typed items (`Command`, `File`, `Message`, `Thought`, `Tool`) and explicit lifecycles
- Container-first safety — harnesses run in yolo/bypass mode, safety comes from the container
- Harness-owned auth — Codex and Claude handle their own OAuth/login, lfd never collects raw credentials
- At most one active session per wave run at a time
- Session end is idempotent; UI disconnect does not affect session state
- Reconnect replays persisted events then follows live stream

## Risks

- **Runtime drift (reduced).** `lf` CLI and `/v0/sessions` HTTP use separate execution paths through the same harnesses. Prompt drift is eliminated (shared `engine/launch.rs`). Arg-building drift is mostly eliminated — `ClaudeArgs`, `build_claude_session_turn_args()`, and `build_codex_thread_start_params()` now live in `engine/agent` and are shared by both paths. Remaining gap: the session harness trait (`Harness`) still lives in `lfd/sessions/harness/`; extracting it into `engine/` is the next runtime convergence step. Conformance replay tests (Claude normal/crash/multi-tool, Codex normal/error) validate harness behavior against recorded traces.
- **Claude `--resume` fragility.** Each turn spawns a new process with `--resume`. If the resume format changes or session state corrupts, the entire session breaks with no partial recovery. Mitigate: session events are persisted, so replay from a new session is possible even if resume fails.
- **Container-only safety model.** No tool-level permission routing means local (non-container) sessions run with full agent permissions. Acceptable for v1 but becomes a gap if local interactive sessions grow in usage.
- **SSE replay scalability (mitigated).** Long sessions accumulate events; full replay on reconnect grows linearly. Store-backed SSE lag recovery (backfill on `Lagged`) handles mid-stream reconnects without data loss, but total event volume still grows unbounded. Not a problem at current scale.
- **Unbounded harness→bridge channel.** Unbounded `mpsc` chosen for correctness (no dropped events). Memory grows with unconsumed burst volume. Acceptable at current session lengths but worth monitoring.
- **lfd restart orphans.** `SessionRuntime` lives in memory only. Active sessions become orphans on lfd restart. Events survive in the store but sessions need a startup recovery pass to mark orphaned `active`/`starting` sessions as `failed`.

## Metrics

- All three harnesses (Codex, Claude, OpenCode) pass shared conformance tests
- Session reconnect replays full event history and resumes live streaming without data loss
- Concerto renders typed transcript with item cards for all event types
- Session lifecycle (create → interact → end) works identically for local and remote lfd

## Architecture

```
Concerto ──HTTP/SSE──▶ lfd session API
                         ├── SessionManager (lifecycle, state machine)
                         │     └── SessionRuntime (harness + broadcast + seq counter)
                         ├── session store (sessions + session_events tables)
                         └── Harness impl (Codex | Claude | OpenCode)
                               ├── event bridge task (harness → store + broadcast)
                               └── agent process
                                     Codex: codex --app-server (JSON-RPC stdio)
                                     Claude: claude -p --resume (NDJSON stdio)
                                     OpenCode: opencode serve (HTTP + SSE)
```

## API

```
POST   /v0/sessions              # create session (harness, config)
GET    /v0/sessions/{id}         # session status + metadata
POST   /v0/sessions/{id}/input   # send user message
GET    /v0/sessions/{id}/events  # SSE replay + follow
DELETE /v0/sessions/{id}         # end session
```

## Harness Comparison

| Harness | Process model | Output | Input | Auth |
|---------|--------------|--------|-------|------|
| Codex | subprocess stdio | JSON-RPC notifications | JSON-RPC requests | OAuth (harness-owned) |
| Claude | subprocess stdio | NDJSON (stream-json) | New process per turn (`--resume`) | OAuth (harness-owned) |
| OpenCode | HTTP client | SSE events | REST calls | Harness-owned |

## Future direction

Prompt convergence and naming cleanup shipped. `lf` and `lfd` share `engine/launch.rs` for prompt assembly, `ClaudeArgs` for Claude flag construction, and `build_codex_thread_start_params()` for Codex session setup. The "provider" concept is replaced by "harness" throughout (DB, API, types).

Runtime convergence continues (tracked in `scratch/agentapi-runtime-convergence.md`): extract the `Harness` trait from `lfd/sessions/harness/` into `engine/` so `lf` one-shot and `lfd` sessions are explicitly two API surfaces over the same harness lifecycle core. The OpenCode adapter will validate whether the harness abstraction holds across three transports.
