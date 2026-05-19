---
status: in-progress
claimed_by: b48afe94-3db7-4c34-9e93-7d59038e3edf
claimed_at: 2026-05-19T01:05:11.053435Z
asana_id: '1214269992004911'
---
# Embedded terminal build driver

> Reshaped by `review-design` (headless). The kickoff's mechanics held up
> against the code; its data model did not. The kickoff invented two new
> `TerminalSession` fields (`interactive`, `provider`) and a new
> `PaneConfig.terminalSessionId`. The codebase already has `source`, `agent`,
> and `PaneConfig.terminalSessionName`. Persistence is a Postgres table with an
> explicit column list, so each invented field costs a migration plus three
> hand-mirrored DTOs. This version reuses what exists: **zero new
> `TerminalSession` fields, no schema migration, one new wire type (the create
> request).** See `scratch/questions.md` for the soft spots.

## Problem

Concerto's workspace multiplexer ships, but the embedded terminal is still a
second-class citizen. Daily build work happens in external Ghostty/Terminal.app
because the embedded pane can't do what a real terminal does: launch a flow with
the right worktree and provider, survive a Concerto restart with output intact,
and let you scroll back / re-run after a flow finishes.

The conductor — Jack, dogfooding loopflow on this repo — opens Concerto every
morning. The finish line is: `⌘K`, type "ship", Enter, the flow runs *in the
app*, and it's still there (progressed) after lunch. External Ghostty becomes a
deliberate choice for long interactive sessions, not the default escape from a
broken embedded experience.

## Approach

**Make lfd the single owner of every embedded terminal.** Collapse the two
parallel terminal stacks onto one: lfd's daemon-managed `TerminalSession`
(persisted in Postgres, tmux-backed, attach contract already wired and proven in
`TerminalWorkspaceView`). The Swift multiplexer pane becomes a thin client that
attaches to an lfd session by ID. This is the load-bearing decision — reattach,
provider display, correct worktree, and layout durability all fall out of having
one source of truth instead of two.

Four concrete changes ride on that decision:

1. **On-demand session creation.** New `POST /v0/terminal-sessions` endpoint
   builds an `lf <flow>` invocation for a given `{wave_id, flow, worktree,
   agent}`, launches the tmux-backed session, returns the `TerminalSession` plus
   attach info. Decoupled from the full wave-run machinery — the palette calls
   this directly. The request body is the **only genuinely new wire type** in
   this design.

2. **Sessions stay alive after the flow exits.** The wrapped command at
   `wave/mod.rs:689-693` ends in `exit "$EXIT_CODE"`. For palette-created
   sessions, end it in `exec "$SHELL"` instead: the flow runs, writes its exit
   code to the exit file, then drops into a shell at the worktree. Scrollback,
   re-run, and reattach now mean something *after* a flow completes — the single
   biggest parity fix. The branch is keyed off the session's existing `source`
   field, not a new flag (see Key decisions).

3. **Durable pane↔session binding.** `PaneConfig` already has an optional
   `terminalSessionName: String?` (MultiplexerLayout.swift:23), and
   `MultiplexerView.swift:240` already falls back to a synthesized
   `lf-{waveId}-{paneId}` when it's nil. Store the lfd **session id** in that
   existing field and delete the synthesized fallback. On Concerto restart the
   pane reattaches by id if `tmux has-session`, else shows a "session ended —
   relaunch" affordance. Layout already persists in `UserDefaults`
   (MultiplexerStore.swift:219); this makes the persisted layout *reconnect*
   instead of spawning fresh blank shells.

4. **Provider visible in the header.** `TerminalSession` already carries
   `agent: String` ("claude", "claude:opus", …). The create path passes
   `lf <flow> -m <agent>` and stores that same string as the session's `agent`.
   The pane header reads `session.agent`. **No new `provider` field** — it would
   duplicate `agent` across a Postgres column and three DTO mirrors, the exact
   drift the DTO rule forbids.

## De-risking

Every line reference below was re-verified against the tree (kickoff numbers
drifted; these are current).

| Question | Finding | Impact on design |
|----------|---------|------------------|
| Can the embedded pane attach to an lfd tmux session at all? | Yes, and it's already done in one view. `POST /v0/terminal-sessions/{id}/attach` returns `TerminalConnectionInfoDto { session_name, host, cwd, status }` (`http/routes/terminal_sessions.rs:89`, struct at `:290-295`). `TerminalWorkspaceView.swift:187-191` already turns that into `argv: ["tmux","attach-session","-t",sessionName]` and feeds `GhosttyTerminalView(workingDirectory:argv:)` (`GhosttyTerminalView.swift:18-30`), gated on `connection.usesLocalTmux`. The contract exists *and ships*; only the multiplexer-pane wiring is missing. | The multiplexer pane reuses `RepoState.attachTerminalSession(_:)` (`RepoState.swift:867`) — the proven path — and deletes its client-side `TmuxSession`. No new transport. |
| Does an lfd session survive a flow finishing? | **No.** `wave/mod.rs:689-693`: `…; {cmd}; EXIT_CODE=$?; printf '%s' "$EXIT_CODE" > {exit_file}; exit "$EXIT_CODE"`. When `lf` ends, the shell exits, tmux dies. `wait_for_tmux_session_exit` (`wave/mod.rs:728-743`) *depends* on that death — it polls `tmux has-session` every 250ms and breaks when the session is gone. | Palette-created sessions `exec "$SHELL"` instead of `exit`. Completion for those is detected by the exit file appearing, not by session death. Wave-executor sessions keep the existing auto-exit + has-session path (don't leak tmux servers in autonomous runs). The discriminator is `source`, already on the session. |
| Is there an API to launch a flow on demand? | **No.** Routes registered at `http/mod.rs:82-104` are list / get / `{id}/attach` / `{id}/start` / `{id}/complete` / `{id}/cancel`. No `POST /terminal-sessions`. Sessions are constructed only as a struct literal inside the wave executor (`wave/mod.rs:320-346`, then `store.create_terminal_session()`). | Add `POST /v0/terminal-sessions` (create). Argv via `build_lf_step_command` (`helpers.rs:358`, signature `(step_name, batch, directions, area, wave_name) -> Vec<String>` — note it takes **no** model or cwd arg, so the endpoint appends `-m <agent>` and sets `cwd` itself). Name via `tmux_session_name` (`terminal_session.rs:14`). Spawn via `launch_tmux_terminal_session` (`wave/mod.rs:669-725`). |
| Can `lf` take a provider override? | Yes. `-m`/`--model <harness[:model]>`, `short_alias = 'M'` (`lf/mod.rs:42`). Resolution: override → step agent → config agent → step default → `"claude:opus"` (`engine/launch.rs:138-147`). | Create endpoint appends `-m <agent>` to argv and stores the same string as `TerminalSession.agent`. No CLI change. No new field. |
| Does tmux survive an lfd restart (not just a Concerto restart)? | Yes. `tmux new-session -d` detaches into the independent tmux server; lfd isn't tmux's parent. Sessions persist as Postgres rows — `INSERT INTO terminal_sessions (16 cols)` at `store/postgres.rs:893-925`. | lfd reconciles on startup: for each non-terminal session, `tmux has-session` → keep if alive, else mark complete. Without this a restarted lfd shows stale "running" sessions. **Postgres is the only persistence path found** — adding a column means a schema migration, which is why this design adds none. |
| Two terminal stacks — how entangled? | Multiplexer `TerminalPaneView` (`MultiplexerView.swift:182`) uses client-side `TmuxSession` (`TmuxSession.swift:8`, name `lf-{waveId}-{paneId}` at `:240`). `TerminalWorkspaceView` already uses the lfd attach RPC (`RepoState.attachTerminalSession`, `RepoState.swift:867`). | Lift the proven lfd-attach path into the multiplexer pane; delete `TmuxSession` and its `attachCommand()` ("keep one implementation", CLAUDE.md). |
| Does `TerminalSession` already model the things the kickoff wanted to add? | **Yes — this is the key correction.** Rust struct (`terminal_session.rs:79-106`) and Swift mirror (`TerminalSession.swift:21-70`) both have `source: String` ("wave_step" / "user_shell") and `agent: String` ("claude" / "interactive" / …). Neither has `interactive` or `provider`. | Reuse `source` for the lifecycle discriminator (new value, see Key decisions) and `agent` for provider display. Zero new `TerminalSession` fields; no migration; no new DTO mirror for the session itself. |
| Parity ceiling — will the embedded terminal ever fully match Ghostty? | Known wave risk. `GhosttyMetalView` (`GhosttyTerminalView.swift:81`) is a real `NSTextInputClient` embedding — IME (`:326-413`), mouse (`:421-500`), clipboard (`:544-561`), keyboard (`:235-322`) all wired. The gap was never rendering; it was lifecycle. | This design closes the lifecycle gap, not the rendering gap. `openWorkspaceShellExternally` (`TerminalWorkspaceView.swift:453`) stays a one-click escape for genuine standalone-window sessions. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep client-side `TmuxSession`, teach it about lfd | Smallest Swift diff | Two sources of truth permanently. Reattach, provider, and worktree drift between the palette stack and the wave-run stack. Violates "keep one implementation." |
| Add explicit `interactive: bool` + `provider` fields to `TerminalSession` (the kickoff's model) | Reads literally | Both duplicate existing fields (`source`, `agent`). Each costs a Postgres migration + Rust/Swift DTO mirrors + a new fixture field — the precise drift the DTO rule exists to kill. Rejected in favor of reusing `source`/`agent`. |
| Drive the pane off lfd's agent **Session** SSE API instead of tmux | Reuses native-chat streaming infra | Not a real PTY — no scrollback, no interactive re-run, dies with the daemon. tmux is what makes "still there after lunch" true across lfd restarts. SSE is the right tool for chat (task 2), not a build terminal. |
| `tmux set-option remain-on-exit on` instead of `exec $SHELL` | One tmux option, no command rewrite | Leaves a *dead* pane needing `respawn-pane`; reattach lands you in a corpse. `exec $SHELL` lands you in a live shell at the worktree, ready to re-run. |
| Spawn tmux client-side from Swift, persist layout client-side only | No Rust changes | lfd can't observe or reconcile sessions it didn't create; a wave run and a palette launch produce incompatible namespaces. The daemon must own sessions for governance/observability to ever see embedded work. |

## Key decisions

- **lfd owns terminals; Swift is a client.** The decision someone will question
  ("why not just spawn tmux from the app?"). The daemon already persists
  sessions, already has the attach contract, already observes wave runs through
  the same journals. One owner means reattach, provider, worktree, and
  cross-restart survival are properties of the system, not of whichever view you
  opened.

- **Lifecycle mode is provenance, not a new flag.** `TerminalSession.source`
  already discriminates `"wave_step"` from `"user_shell"`. A palette-launched
  flow is a *third provenance*: it runs `lf <flow>` like a wave step but must
  stay alive like a user shell. Add a new `source` value — proposed
  `"palette"` — and key behavior off it: `source == "palette"` → `exec "$SHELL"`
  after the flow + completion via exit-file watch; everything else → unchanged
  `exit "$EXIT_CODE"` + has-session watch. `source` is a `String` column, so a
  new value needs no migration. The exact string is the one soft spot — see
  `scratch/questions.md`.

- **Provider display reuses `agent`.** The create request carries an `agent`
  override string; the endpoint appends it to argv as `-m <agent>` and stores it
  as `TerminalSession.agent`. The header reads `session.agent`. No `provider`
  field — it would be a synonym for `agent` replicated across three layers.

- **Completion = exit file, not session death (palette sessions only).** The
  wrapped command already writes the exit code to the exit file *before* the
  trailing `exit`/`exec`. For palette sessions the poller watches that file
  appear; this survives the session staying alive and is strictly more robust.
  Wave-executor sessions keep `wait_for_tmux_session_exit` unchanged.

- **Pane binds to the lfd session id via the existing field.**
  `PaneConfig.terminalSessionName` already persists through `UserDefaults`.
  Store the lfd session **id** there and delete the synthesized
  `lf-{waveId}-{paneId}` fallback at `MultiplexerView.swift:240`. No new
  `PaneConfig` field; one field changes meaning (and arguably should be renamed
  `terminalSessionId` for honesty — see questions).

- **DTO discipline, applied honestly.** The only new wire type is the create
  **request** (`{wave_id, flow, worktree, agent}`). It crosses the lfd HTTP
  boundary → no `#[serde(default)]`, no Swift init defaults, every field
  required-or-explicitly-Optional. Add a `tests/fixtures/dto/` fixture for it
  and a per-language fixture test. Also add the missing
  `tests/fixtures/dto/terminal_session.json` (only `session.json` /
  `session_unsupported_input.json` exist today) so the *response* shape is
  pinned — but the session DTO gains no fields, so this is pinning, not
  extending. `TerminalSessionDto` lives at `http/dto.rs:245-266`, converter at
  `:268`; Rust test `rust/loopflow/tests/dto_fixtures.rs`, Swift test
  `swift/ConcertoTests/DTOFixtureTests.swift`.

## Scope

**In scope:**
- `POST /v0/terminal-sessions` create endpoint (`{wave_id, flow, worktree,
  agent}` → `TerminalSession` + attach info); register alongside the existing
  six routes at `http/mod.rs:82-104`
- New `source` value (`"palette"`) — Rust + Swift constants, no schema change
- Create-request DTO fixture + Rust/Swift fixture tests; add the missing
  `terminal_session.json` response fixture
- `source == "palette"` tmux command (`exec "$SHELL"` after flow) and
  exit-file-watch completion path, branched at `wave/mod.rs:689-693` /
  `:728-743`
- lfd startup reconcile: for non-terminal sessions, `tmux has-session` → keep
  live, else mark complete
- Multiplexer terminal pane attaches via `RepoState.attachTerminalSession`;
  delete client-side `TmuxSession` + `attachCommand()`
- `PaneConfig.terminalSessionName` holds the lfd session id; delete the
  synthesized fallback; restore-time reattach with "session ended — relaunch"
  fallback
- Command palette flow launch → create session → bind to focused (or new) pane,
  replacing the external launch path for the in-app case
- Pane header shows `session.agent`; provider picker in the launch path
  (Claude / Codex / OpenCode) feeding the create request's `agent`
- Polish on the lifecycle surface only: focus ring on the terminal pane,
  "session ended" / "reattaching…" states, header composition

**Out of scope:**
- Native chat rendering, history, composer (task 2 — must not steal focus)
- Governance dashboards / portfolio / calibration (`workflows`, per README)
- Replacing external Ghostty for every session — pop-out stays one click
- Closing the *rendering* parity gap — rendering already works
- tmux split/window management *inside* a single session — multiplexer's job,
  already shipped
- Any new `TerminalSession` field or Postgres migration — explicitly designed
  out

## Done when

Verified by `scripts/verify_embedded_build_driver.py` (new; one command, drives
a real lfd + a scripted flow), asserting:

- `POST /v0/terminal-sessions` with a flow + worktree + agent returns a session
  whose attach info points at a live tmux session running `lf <flow> -m <agent>`
- After the flow exits, `tmux has-session` is still true and the session shows a
  shell at the worktree (`source == "palette"`); the session row is `Succeeded`
  with the captured exit code, detected via the exit file
- Killing and restarting lfd leaves the session attachable; a session whose
  tmux died is reconciled to a terminal state on startup
- `session.agent` round-trips through the DTO and matches the `-m` value
- The create-request fixture round-trips identically in Rust and Swift

Plus an observable Concerto walkthrough in the same script's `--ui` mode:
`⌘K` → "ship" → Enter runs the flow in the focused embedded pane (no
Terminal.app window); quit and relaunch Concerto → the pane reattaches with
output intact; the header reads the dispatched agent.

The subjective bar — "no longer feels second-class for build work" — is met
when external Ghostty is used by choice, not because the embedded one lost
state.

## Wave alignment

**Vision** (Desktop README): "Make Concerto the default build-driving surface."
This design is that vision's mechanism — it removes every reason the embedded
terminal loses to external Ghostty for build work.

**Goals** — advances all five: palette launch (create endpoint + rewiring),
restart survival (lfd ownership + durable pane binding + `exec $SHELL`),
multi-agent dispatch visible (reused `agent` field + `-m` + header), layout
persists (existing `terminalSessionName` reconnect), no longer second-class
(lifecycle parity).

**Risks** (README): "parity has a ceiling" — addressed: the ceiling was
lifecycle, not rendering; pop-out stays. "Polish can sprawl" — scope fences
polish to the lifecycle surface, excludes chat and governance.

New risk: leaked tmux servers if a palette session is never closed. Mitigated by
startup reconcile and the existing cancel→`stop_tmux_terminal_session`
(`terminal_sessions.rs:137-148`, `tmux kill-session`). Document a cap or idle
sweep if dogfooding shows accumulation.

## Measure

- **Baseline:** count of build flows launched via external Terminal.app/Ghostty
  vs embedded over a dogfooding day (instrument the palette launch path).
- **Target:** ≥90% of build-flow launches stay embedded.
- **Restart integrity:** quit/relaunch Concerto mid-flow N=10; 10/10 reattach
  with progressed output, no orphaned/duplicate sessions.
