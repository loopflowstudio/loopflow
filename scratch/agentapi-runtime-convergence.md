# Runtime Convergence

## Problem

Loopflow currently has two independent agent runtimes:

1. `lf` spawns providers directly via `engine/agent.rs`.
2. `lfd` sessions spawn providers via `lfd/sessions/harness/*` and persist typed events.

That split creates product drift and operational risk:

- provider launch behavior can diverge between CLI and session API
- prompt assembly is duplicated (`build_step_prompt`, `prepare_step_prompt`, `lf::commands::run::build_prompt`)
- interactive `lf design/explore/review/refine` cannot reconnect/replay because they are terminal-local processes
- wave executor steps do not share session history/replay semantics with interactive sessions

Who benefits:

- **Users:** consistent behavior between `lf` and Concerto, resumable interactive sessions
- **Maintainers:** one provider integration surface, one conformance test suite
- **Wave operators:** run history and replay for automated steps, not just interactive chat

Why now: this directly addresses the wave risk **“Provider layer drift”** and advances goals **“lfd owns the session lifecycle”** and **“Provider-agnostic: same client code works regardless of which agent runs the session.”**

## Approach

Adopt a **session-runtime-first architecture** with one execution core and two entrypoints (CLI + HTTP).

### 1) One runtime core, two surfaces

Create a shared runtime module used by both `lf` and `lfd`:

- shared session orchestration primitive (create/start/input/stop/wait)
- shared provider launch + mapping logic (Claude/Codex/OpenCode)
- shared prompt-prep path returning `LaunchConfig`

`lfd` keeps HTTP/SSE as an API surface over this runtime. `lf` uses the same runtime through a local client facade (not direct subprocess spawning).

### 2) Unify prompt assembly into one function

Replace split prompt-prep paths with one builder used by:

- interactive `lf` launch
- wave executor step launch
- session API create-session flow

This single function owns:

- context gathering and trimming
- step loading and merged directions
- summary attachment
- `LaunchConfig` + run-mode-specific process options

### 3) Move provider command builders into harness layer

`engine/agent.rs` stops owning provider CLI arg construction.

- Claude/Codex/OpenCode command builders live with their harnesses
- batch and interactive entrypoints call the same harness-owned builders
- provider-specific normalization remains inside mapping modules

### 4) Interactive `lf` becomes a session launcher

For interactive steps (`design`, `explore`, `review`, `refine`):

1. `lf` creates a session through lfd
2. `lf` opens Concerto via a deep link containing repo + session id
3. Concerto attaches to that existing session in `WaveChatView`
4. `lf` exits after launch

No terminal-agent fallback path. If lfd/Concerto is unavailable, fail with explicit fix instructions.

### 5) Wave executor runs steps via sessions

For local/native execution mode, wave step execution becomes:

1. create session (`auto: true` / batch mode)
2. stream and persist session events
3. map terminal session status to agent success/failure
4. advance run state from session completion

This gives wave runs the same replayable event history as interactive sessions.

### 6) Conformance tests enforce parity

Add shared provider conformance tests that run both entry surfaces against identical traces/scenarios:

- create/start/input/end lifecycle
- turn/item event shape parity
- error and interruption behavior parity

If a provider change breaks one surface, tests fail both.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep dual runtime, add adapters | Lowest short-term churn | Preserves drift risk and duplicates provider work forever |
| HTTP-only everything (`lf` always calls daemon API) | Very clean architecture | Hard cutover risk; poor offline/dev ergonomics; large migration blast radius |
| Session-runtime-first shared core with CLI + HTTP surfaces (chosen) | Requires runtime extraction and API reshaping | Best long-term convergence while keeping explicit surfaces for CLI and daemon |

## Key decisions

- **No terminal fallback for interactive `lf`.** Two interactive paths recreate drift immediately.
- **Deep-link contract is explicit and versioned.** Concerto navigation uses a stable URL payload (session id + repo + step context).
- **Session ownership stays in lfd.** Supports the goal that disconnecting UI does not affect session state.
- **Convergence prioritizes local/native execution first.** Containerized wave execution parity is tracked as follow-on work, not hidden complexity.

### Wild success (what makes this great)

- Users launch `lf design`, immediately land in chat UI, reconnect from Concerto after restarts, and never lose transcript state.
- `lf` and session API produce identical provider behavior, so docs and debugging become straightforward.
- Wave runs gain replayable typed event history, enabling better diagnostics and future UI reuse.

### Wild failure (what kills this in 6 months)

- Fragile app handoff (session created but Concerto fails to attach).
- Silent lfd dependency failures that feel like random command breakage.
- “Temporary” dual paths left in place after migration.

Mitigations in this design:

- health check + actionable startup errors before interactive launch
- deep-link ack/attach validation path in Concerto
- explicit deletion of legacy direct-launch code after cutover

## Scope

- In scope:
  - single prompt-prep builder used by session API + wave executor + `lf` launch flow
  - harness-owned provider command builders
  - interactive `lf` → create session → open Concerto
  - wave executor local/native path routed through session runtime
  - shared provider conformance tests across CLI/session surfaces
- Out of scope:
  - approval-routing/permission redesign
  - advanced multi-panel Concerto UI work
  - multi-agent per step
  - cross-wave session sharing
  - full container executor parity in this phase

## Done when

- `build_step_prompt` and `prepare_step_prompt` are replaced by one shared launch builder.
- Interactive `lf design/explore/review/refine` no longer spawn agent subprocesses directly; they create sessions and open Concerto.
- Wave executor local/native step runs are session-backed and emit persisted session events.
- Provider command builders are removed from `engine/agent.rs` and owned by harness modules.
- Shared provider conformance tests pass for both CLI-launched and session-API-launched runs.
- This phase advances wave goals (from `wave/agentapi/README.md`):
  - **“lfd owns the session lifecycle”**
  - **“Provider-agnostic: same client code works regardless of which agent runs the session”**
  - **“Reconnect replays persisted events then follows live stream”**
