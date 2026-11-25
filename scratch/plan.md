# LOO-1098: wake idle Antigravity, and make harness parity typed

History and 5 Whys: [5whys.md](5whys.md). In one line: idle wake was picked
from whichever store columns a harness filled in, gaps were README cells, and
no test looped over every harness. So `agy` fell through without any error.

## What people see

- Leave an `agy` Task work session idle and run `lf steer LOO-12 'Reply with only the word OK.'`.
  The steer shows up in the TUI as a pasted prompt and a turn runs. No typing needed.
- Quit, steer, then `lf resume --task LOO-12`: the waiting steer runs once,
  as soon as `agy` is ready for input.
- Add a `HarnessKind` variant, or switch a harness's idle wake off, and the
  build or the parity suite fails, naming the harness and the behavior.
- The parity table in `harness/README.md` is generated from code, so the
  stale `Task 540073` cell goes away.

## How `agy` gets woken (probed on agy 1.5.16, 2026-11-24)

The other harnesses are woken through a provider inbox. `agy` has none an
outside process can reach:

- `agy agentapi send-message <conv> <text>` talks to one running server's
  gRPC port (`ANTIGRAVITY_LS_ADDRESS`) and needs that server's own
  `ANTIGRAVITY_CSRF_TOKEN`. agy makes the token itself; one set in the
  environment at launch is refused ("invalid CSRF token").
- Only tool commands and sidecars get that token. Hooks (`PreInvocation`, `Stop`) get
  only `ANTIGRAVITY_CONVERSATION_ID`. Workspace `.agents/mcp_config.json`
  servers start but get no `ANTIGRAVITY_*` at all. A sidecar in a workspace
  plugin is never started; sidecars load from `~/.gemini/config` only, which
  lf doesn't write. agy writes no discovery file. No `SessionStart` hook fires
  at TUI start.
- `--remote-control` goes through Google's relay. Ruled out.

**Route chosen: the held terminal.** Every work session already runs in a
held terminal on the Machine server, which keeps a libghostty-vt model of
the screen and the input queue. The server types the claimed steers in as
one bracketed paste, `ESC[200~ <text> ESC[201~`, then `\r`. Probed: a multi-line paste plus Enter
runs one turn, and the TUI shows it like a typed prompt. Typed input is
lost if sent before agy is ready. agy turns on bracketed-paste mode
(`ESC[?2004h`) when it starts reading input: input sent 0.3 s after spawn,
before that point, was lost; input sent 50 ms after it ran.

The terminal wake types only when all three hold:

1. the store says the conversation yielded (Stop hook, or a first boundary
   reported at launch);
2. the VT says the child has bracketed paste on;
3. no view has typed into that terminal since the conversation yielded. That
   leaves a person's draft alone, and their next Enter delivers the steer
   through the `PreInvocation` hook anyway.

If any check fails, the wake leaves the steers unclaimed and they wait for
the next boundary, the same fallback every harness has today. A store or
server that is down changes nothing: typed input to the held terminal still
passes straight through, and the wake doesn't run at all.

Passed over: running `agentapi` with an inherited token (it routes through
whichever agy launched lf; see below); keystroke heuristics over the VT
screen (fragile); writing a sidecar into `~/.gemini` (lf writes no global
provider settings).

**Found while probing (fixed here).** `agent::CALLER_SESSION_ENV` drops
Claude, Codex and OpenCode markers but no `ANTIGRAVITY_*`. An `agy` that lf
launches from inside another agy's tool shell inherits the parent's
`ANTIGRAVITY_LS_ADDRESS`, `ANTIGRAVITY_CSRF_TOKEN` and
`ANTIGRAVITY_SOURCE_METADATA`, and agy doesn't overwrite them for its hooks.
Every `agentapi` call from the child then goes to the parent's server. Seen
live: messages sent that way were attributed to the parent conversation and
ran in a second backend ("server restart" notice), so the receiving TUI never
showed them. Add the ten `ANTIGRAVITY_*` names agy sets: `AGENT`, `AGENTAPI_EXE`,
`APP_DATA_DIR`, `CONVERSATION_ID`, `CSRF_TOKEN`, `LS_ADDRESS`, `LS_VERSION`,
`PROJECT_ID`, `SOURCE_METADATA`, `TRAJECTORY_ID`.

## Reopening

`agy` reports no boundary until its first model call. Like Codex in LOO-1068,
the `agy` open path sends the first `session.boundary` through `hook_request`
once its argv is built: `SessionStart` (`source: resume|startup`) when it has
no opening input, `UserPromptSubmit` when it opens with `-i`. Move Codex's
`report_first_boundary` (`harness/codex.rs:1244`) somewhere both can use it,
not into a second copy. A new `agy` conversation has no thread until its first hook names one, so
only a resumed one reports at launch. A new one always opens with `-i`, and
its first `PreInvocation` reports it.
`record_first_boundary` then marks the reopened process idle.
The wake runs, finds the terminal not ready yet, and leaves the steers. When
the host's VT first sees `?2004h` on a managed terminal, the server runs
`wake::after_turn(session)`. That event covers both reopen with waiting steers
and a steer that arrives before the first turn.

## Source of truth

- `harness::Capabilities.idle_wake: IdleWake`, a required field with no
  `Option`: `Inbox` (Claude), `AppServer` (Codex), `Socket` (OpenCode, Pi),
  `Terminal` (Antigravity). `steer::idle_conversations` passes the providers
  whose `idle_wake == Terminal` to `idle_conversation_candidates`. The store
  then returns `IdleReach::Terminal` for them, with no column. SQL no longer
  hard-codes which provider is reached how. Codex is the exception and stays
  keyed by its endpoint.
- `harness::parity`: `Behavior` (exhaustive, with `ALL`) and
  `fn support(HarnessKind, Behavior) -> Support`, where `Support` is
  `Yes` or `Gap { why, owner }`. Matches are exhaustive with no wildcard
  arms, so a new harness or behavior doesn't compile until it's filled in.

Behaviors, as the README names them: worker step runs and advances; records
usage; receives step context; hears a mid-turn steer; never asks permission;
passed over when not installed; unanswered question fails the turn; stops on
interrupt; refusal falls through; idle work session hears a steer; reopened
work session takes waiting steers; installed skills listed; viewer handoff
keeps the conversation (LOO-1097). The cells start as today's README
answers. Known gaps: unanswered question on Claude, Codex and Antigravity
(owner Task 48edb9); viewer handoff on Antigravity (owner LOO-1097).

## Conformance

- Each behavior's test loops over `HarnessKind::ALL` and asserts
  `support(h, b)` both ways: `Yes` must pass, and `Gap` must still fail, so
  fixing a gap fails the test until the table changes. The message names
  `{harness:?}` and the behavior.
- `server::wake::tests::every_harness_wakes_its_idle_work_session` replaces
  the per-reach tests where it can. For each harness it builds the idle
  conversation through that harness's `idle_wake` (inbox key, endpoint stand-in,
  socket stand-in, held-terminal stand-in), saves a steer, and asserts
  exactly one turn start. Antigravity's stand-in is a held-terminal program
  that enables `?2004h` and echoes the paste it reads.
- The three interrupt tests split by harness (`…_on_claude_and_codex`,
  `…_on_opencode_and_pi`, `…_on_antigravity`) become one loop over `ALL`.
- `parity::tests::readme_table_matches_support` renders the table from
  `support()` and compares it with the README section between markers.
  `cargo test … -- --ignored regenerate` writes it.

## Absent and error states

- Terminal gone, child exited, VT unreadable: the wake leaves steers unclaimed
  and logs once per changed reason. The next boundary delivers.
- Paste mode off: a `/` menu, an auth screen, or the TUI not up yet. Same as
  above. The `?2004h` event wakes it later.
- Person typing: left alone; their Enter delivers through the hook.
- `agy` mid-turn: not yielded, no wake. The hook delivers between tools.
- A worker or chat `agy`: not interactive, no held terminal, never a candidate.

## Operational boundary

No new process or poll per Session. The `?2004h` check reads state the VT
already keeps. The "typed since yield" check is one `SystemTime` per terminal
in the registry. The candidate query keeps `INDEXED BY agent_sessions_unended_agent`;
the provider list adds one `IN` term. Check it with `EXPLAIN QUERY PLAN`
against the 10k-Task store.

## Exclusions

- Real-provider viewer handoff for `agy` (LOO-1097). It gets a typed `Gap`
  owned by LOO-1097, not a fix.
- Reading a headless question on Claude, Codex or agy (Task 48edb9 stays the owner).
- A gate check that a `Gap`'s owner Task is still open.

## What goes

- README hand-written parity table and the stale `Task 540073` cell. The
  "Antigravity takes queued steers on its next turn" and "any of the four" sentences.
- The SQL `CASE`/`OR` that infers reach from filled-in columns, replaced by
  `idle_wake`.
- Codex-only `report_first_boundary` (moved, not copied).
- Per-harness interrupt test split.

## Passes

1. **Terminal wake on a live idle `agy`** (could prove the plan wrong).
   `IdleWake` in `Capabilities`; `IdleReach::Terminal`; registry
   `type_steers(session, text) -> Result<bool>` gated on paste mode and
   typing since yield; `Reach::Terminal` in `wake.rs` (the Registry reaches
   `wake_idle` and `boundary::after`); `ANTIGRAVITY_*` in `CALLER_SESSION_ENV`.
   The held-terminal stand-in test. Then real `agy` in a held terminal on this
   Machine: `lf resume --task`, let it settle, `lf steer`. Shows: a turn
   starts once, it shows in the TUI, `lf session steers` marks it delivered, and the
   `PreInvocation` hook records the turn without delivering the steer twice.
2. **Reopen.** Shared first-boundary report on `agy` open; the `?2004h`
   event triggers `wake::after_turn`. Real `agy`: quit, steer, `lf resume --task`,
   and it runs once before anyone types.
3. **Parity contract.** `harness/parity.rs`, behavior tests switched to loops
   over `ALL` that check both ways, README generated and checked, dead text
   and split tests removed. Demo 2: setting
   Antigravity's `idle_wake` to a `Gap`, or deleting `IdleReach::Terminal`,
   fails `server::wake::tests::every_harness_wakes_its_idle_work_session`
   and the README check, each naming Antigravity.

## Acceptance at gate

```sh
cargo test -p loopflow --lib server::wake::tests::
cargo test -p loopflow-execution --lib harness::parity::
cargo test -p loopflow-execution --lib harness::conformance_tests::
```

Each runs every harness. A real `agy` check runs at demo: steps 1 and 2 in a
held terminal, with the turn visible in the TUI and one `delivered_at` per steer.
