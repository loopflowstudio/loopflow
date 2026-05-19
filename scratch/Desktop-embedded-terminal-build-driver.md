---
status: in-progress
claimed_by: b48afe94-3db7-4c34-9e93-7d59038e3edf
claimed_at: 2026-05-19T01:05:11.053435Z
asana_id: '1214269992004911'
---
# Embedded terminal build driver

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
(persisted, tmux-backed, attach contract already wired). The Swift multiplexer
pane becomes a thin client that attaches to an lfd session by ID. This is the
load-bearing decision — reattach, provider display, correct worktree, and layout
durability all fall out of having one source of truth instead of two.

Four concrete changes ride on that decision:

1. **On-demand session creation.** New `POST /v0/terminal-sessions` endpoint
   builds an `lf <flow>` invocation for a given `{flow, worktree, provider}`,
   launches the tmux-backed session, returns attach info. Decoupled from the
   full wave-run machinery — the palette calls this directly.

2. **Sessions stay alive after the flow exits.** Replace the wrapped command's
   trailing `exit "$EXIT_CODE"` (wave/mod.rs:690) with `exec "$SHELL"` for
   interactive sessions. The flow runs, writes its exit code to the exit file,
   then drops into a shell at the worktree. Scrollback, re-run, and reattach now
   mean something *after* a flow completes — the single biggest parity fix.

3. **Durable pane↔session binding.** `PaneConfig` stores the lfd
   `terminalSessionId` (not a synthesized local name). On Concerto restart the
   pane reattaches by ID if `tmux has-session`, else shows a "session ended —
   relaunch" affordance. Layout already persists in `UserDefaults`; this makes
   the persisted layout *reconnect* instead of spawning fresh blank shells.

4. **Provider visible in the header.** The create path passes `lf <flow> -m
   <harness[:model]>` and stores the provider on the session. The pane header
   shows "claude:opus" / "codex:o3" / "opencode" — multi-agent dispatch you can
   see.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|------------------|
| Can the embedded pane attach to an lfd tmux session at all? | Yes. `POST /v0/terminal-sessions/{id}/attach` returns `{session_name, host, cwd, status}` (terminal_sessions.rs:89). `GhosttyTerminalView` already takes `argv`; `tmux attach-session -t {name}` is exactly what the local `TmuxSession.attachCommand()` produces. The contract exists; only the wiring is missing. | Swift pane swaps its local `TmuxSession` for an attach against an lfd session ID. No new transport. |
| Does an lfd session survive a flow finishing? | **No.** Wrapped command is `…; {cmd}; EXIT_CODE=$?; printf … > exit_file; exit "$EXIT_CODE"` (wave/mod.rs:690). When `lf build` ends, the shell exits, tmux session dies. `wait_for_tmux_session_exit` (wave/mod.rs:728) *depends* on that death to mark completion. | Interactive sessions must `exec "$SHELL"` instead of `exit`. Completion detection must switch from "tmux session died" to "exit file appeared" — the poller watches the exit file, not `has-session`. Headless wave-executor sessions keep the old auto-exit behavior (don't leak tmux servers in autonomous runs); the difference is a per-session `interactive` flag. |
| Is there an API to launch a flow on demand? | **No.** Routes are list / get / attach / start / complete / cancel (http/mod.rs:82-102). Sessions are created only inside the wave executor (wave/mod.rs:310). | Add `POST /v0/terminal-sessions` (create). Reuse `build_lf_step_command` (helpers.rs:358) for argv, `tmux_session_name` for naming, `launch_tmux_terminal_session` for spawn. |
| Can `lf` take a provider override? | Yes. `lf <step> -m/--model <harness[:model]>` (lf/mod.rs:42); launch resolves override → step agent → config agent → default (launch.rs:138). | Create endpoint injects `-m <provider>` into argv. No CLI change needed. Provider stored on the session for the header. |
| Does tmux survive an lfd restart (not just a Concerto restart)? | Yes. `tmux new-session -d` detaches into the independent tmux server; lfd's process isn't tmux's parent. lfd persists `TerminalSession` rows via `Store` (store/mod.rs:597). | lfd reconciles on startup: for each non-terminal session, `tmux has-session` → keep if alive, else mark complete. Without this, a restarted lfd shows stale "running" sessions. |
| Two terminal stacks — how entangled? | Multiplexer `TerminalPaneView` uses client-side `TmuxSession` (`lf-{waveId}-{paneId}`). `TerminalWorkspaceView` already uses the lfd attach RPC (`RepoState.attachTerminalSession`, RepoState.swift:867). | The lfd-attach path is proven in `TerminalWorkspaceView`; lift it into the multiplexer pane and delete the client-side `TmuxSession` ("keep one implementation", CLAUDE.md). |
| Parity ceiling — will the embedded terminal ever fully match Ghostty? | Known wave risk. Ghostty-in-Ghostty is solid (full `NSTextInputClient`, IME, mouse, clipboard already wired in `GhosttyMetalView`). The real gap was never rendering — it was lifecycle (death on exit, no reattach, wrong cwd). | This design closes the lifecycle gap, not the rendering gap. "Pop out to external Ghostty" stays a one-click escape (`openWorkspaceShellExternally`, TerminalWorkspaceView.swift:453) for sessions that genuinely want a standalone window. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep client-side `TmuxSession`, teach it about lfd | Smallest Swift diff | Two sources of truth permanently. Reattach, provider, and worktree all drift between the stack the palette uses and the stack a wave run uses. Directly violates "keep one implementation." |
| Drive the pane off lfd's agent **Session** SSE API (`/sessions/{id}/events`) instead of tmux | Reuses streaming infra from native-chat work | Not a real PTY — no scrollback, no interactive re-run, dies with the daemon. tmux is what makes "still there after lunch" true across lfd restarts. SSE sessions are the right tool for chat, not for a build terminal. |
| `tmux set-option remain-on-exit on` instead of `exec $SHELL` | One tmux option, no command rewrite | Leaves a *dead* pane that needs `respawn-pane` to become usable; reattach lands you in a corpse. `exec $SHELL` lands you in a live shell at the worktree, ready to re-run. |
| Spawn tmux client-side from Swift, persist layout client-side only | No Rust changes | lfd can't observe or reconcile sessions it didn't create; a wave run and a palette launch produce incompatible session namespaces. The daemon must own sessions for governance/observability to ever see embedded work. |

## Key decisions

- **lfd owns terminals; Swift is a client.** The decision someone will question
  ("why not just spawn tmux from the app?"). Answer: the daemon already persists
  sessions, already has the attach contract, already observes wave runs through
  the same journals. One owner means reattach, provider, worktree, and
  cross-restart survival are properties of the system, not of whichever view you
  opened.

- **Interactive vs managed session mode.** A new `interactive: bool` on
  `TerminalSession`. Interactive (palette-launched) → `exec "$SHELL"` after the
  flow, completion detected via exit-file watch. Managed (wave-executor) →
  unchanged `exit "$EXIT_CODE"`, completion via session death. Headless
  autonomous runs must not leak idle tmux servers; interactive build sessions
  must not vanish. Same code path, one flag.

- **Completion = exit file, not session death.** The wrapped command already
  writes the exit code to `{session_id}.exit` *before* exiting (wave/mod.rs:690).
  The poller watches for that file for interactive sessions. This is a strictly
  more robust completion signal — it survives the session staying alive.

- **Pane stores the lfd session ID.** `PaneConfig.terminalSessionId` replaces the
  synthesized `lf-{waveId}-{paneId}` name. Layout JSON in `UserDefaults` already
  persists; binding to a daemon ID is what makes a restored layout reconnect to
  live output instead of blank shells.

- **DTO discipline.** The create request and the session DTO cross the lfd HTTP
  boundary and are mirrored in Rust/Swift. No `#[serde(default)]`, no Swift init
  defaults — `interactive` and `provider` are required-or-explicitly-Optional,
  with a round-trip fixture under `tests/fixtures/dto/` and a per-language
  fixture test (CLAUDE.md "DTOs").

## Scope

**In scope:**
- `POST /v0/terminal-sessions` create endpoint (`{wave_id, flow, worktree, provider, interactive}` → session + attach info)
- `interactive` + `provider` fields on `TerminalSession`; DTO fixtures + tests
- Interactive-mode tmux command (`exec "$SHELL"` after flow) and exit-file-watch completion path
- lfd startup reconcile: prune dead sessions, keep live ones
- Multiplexer terminal pane attaches to an lfd session by ID; delete client-side `TmuxSession`
- `PaneConfig.terminalSessionId`; restore-time reattach with "session ended — relaunch" fallback
- Command palette flow launch → create session → bind to focused (or new) pane, replacing the external `runWave` spawn for the in-app path
- Pane header shows provider; provider picker in the launch path (Claude / Codex / OpenCode)
- Polish on the lifecycle surface only: focus ring on the terminal pane, "session ended" / "reattaching…" states, header composition

**Out of scope:**
- Native chat rendering, history, composer (task 2 — must not steal focus)
- Governance dashboards / portfolio / calibration (belongs to `workflows`, per Desktop README "Not here")
- Replacing external Ghostty for every session — pop-out stays a one-click escape
- Closing the *rendering* parity gap — rendering already works; this is lifecycle
- tmux split/window management *inside* a single session — pane-level splits are the multiplexer's job, already shipped

## Done when

Verified by `scripts/verify_embedded_build_driver.py` (new; one command, drives a
real lfd + a scripted flow), asserting:

- `POST /v0/terminal-sessions` with a flow + worktree + provider returns a
  session whose `attach` info points at a live tmux session running `lf <flow>`
- After the flow exits, `tmux has-session` is still true and the session shows a
  shell at the worktree (interactive mode); the session row is `Succeeded` with
  the captured exit code
- Killing and restarting lfd leaves the session attachable; a session whose tmux
  died is reconciled to a terminal state on startup
- The created session's `provider` round-trips through the DTO and matches `-m`

Plus an observable Concerto walkthrough in the same script's `--ui` mode:
`⌘K` → "ship" → Enter runs the flow in the focused embedded pane (no
Terminal.app window); quit and relaunch Concerto → the pane reattaches with
output intact; the pane header reads the dispatched provider.

The subjective bar from the finish line — "no longer feels second-class for
build work" — is met when the external-Ghostty path is used by choice, not
because the embedded one lost state.

## Wave alignment

**Vision** (Desktop README): "Make Concerto the default build-driving surface."
This design *is* that vision's mechanism — it removes every reason the embedded
terminal loses to external Ghostty for build work.

**Goals** (item "Done when"): advances all five —
- "Flow launch from command palette runs in the embedded terminal, not
  Terminal.app" → create endpoint + palette rewiring
- "Sessions survive Concerto restart and reattach cleanly" → lfd ownership +
  durable pane binding + `exec $SHELL`
- "Multi-agent dispatch visible in the session header" → `provider` field +
  `-m` injection + header
- "Layout persists per wave across launches" → `PaneConfig.terminalSessionId`
  reconnect on restore
- "No longer feels second-class" → lifecycle parity (the actual gap)

**Risks** (Desktop README): "Embedded terminal parity has a ceiling" — addressed
head-on in de-risking: the ceiling was lifecycle, not rendering; rendering
already works, and pop-out stays for the genuine standalone-window cases.
"Build-driver polish can sprawl" — scope explicitly fences polish to the
lifecycle surface and excludes chat (task 2) and governance (workflows).

New risk introduced: leaked tmux servers if an interactive session is never
closed. Mitigated by the startup reconcile and a session `cancel` that kills
tmux (terminal_sessions.rs:137 already does this) — but document a cap or idle
sweep if dogfooding shows accumulation.

## Measure

- **Baseline:** count of build flows launched via external Terminal.app/Ghostty
  vs embedded over a dogfooding day (instrument the palette launch path).
- **Target:** ≥90% of build-flow launches stay embedded (the finish line's
  "90% of daily build work stays in the app").
- **Restart integrity:** quit/relaunch Concerto mid-flow N=10 times; 10/10
  reattach with progressed output and no orphaned/duplicate sessions.
