# LOO-1098: wake idle Antigravity, and make harness parity typed

History and 5 Whys: [5whys.md](5whys.md). Concept review: [concept-review.html](concept-review.html).
In one line: idle wake was inferred in SQL from whichever columns a harness filled
in, parity gaps were hand-written README cells, and feature tests hardcoded subsets
of harnesses. So `agy` fell through without any error.

## What people see

- Leave an `agy` Task work session idle and run `lf steer LOO-12 'Reply with only the word OK.'`.
  The steer appears in the TUI as a pasted prompt and a turn runs. No typing needed.
- Quit, steer, then `lf resume --task LOO-12`: waiting steers (direct steers, Task
  updates, and wave/repo steers) run once as soon as `agy` is ready for input.
- Typing a draft — while `agy` is idle or mid-turn — is never clobbered: focusing
  the terminal or scrolling with the mouse still allows wakes, while unsubmitted
  keystrokes hold the wake off until Enter delivers the steers through `PreInvocation`.
- Add a `HarnessKind` variant, drop a harness's idle wake, or break an automated
  behavior on any harness, and the build or parity suite fails naming the harness
  and the behavior.
- The parity table in `harness/README.md` is generated from code and checked in CI,
  removing the stale `Task 540073` cell and keeping `server/README.md` in sync.

## How `agy` gets woken (probed on agy 1.5.16, 2026-11-24)

The other harnesses are woken through a provider inbox or socket. `agy` has none
an outside process can reach:

- `agy agentapi send-message <conv> <text>` talks to one running server's gRPC
  port (`ANTIGRAVITY_LS_ADDRESS`) and requires that server's own
  `ANTIGRAVITY_CSRF_TOKEN`. `agy` generates the token itself; one passed at launch
  is refused ("invalid CSRF token").
- Only tool commands and sidecars receive that token. Hooks (`PreInvocation`,
  `Stop`) receive only `ANTIGRAVITY_CONVERSATION_ID`. Workspace
  `.agents/mcp_config.json` servers receive no `ANTIGRAVITY_*` variables. Workspace
  plugin sidecars are not started; sidecars load only from `~/.gemini/config`,
  which lf does not write. `agy` writes no discovery file and fires no
  `SessionStart` hook at TUI start.
- `--remote-control` goes through Google's relay. Ruled out.

**Route chosen: the held terminal.** Every work session runs in a held terminal
hosted by the Machine server (`Registry` / `Hosted`), which already owns the
`libghostty-vt` screen model and `PendingInput` queue. The server sends claimed
steers as one sanitized bracketed paste, `ESC[200~ <text> ESC[201~\r`. Probed on
`agy` 1.5.16: a multi-line paste plus `\r` runs one turn, shown in the TUI like a
typed prompt. Input sent before `agy` is ready is lost; `agy` enables bracketed
paste mode (`ESC[?2004h`) when it begins reading input (0.3 s after spawn, before
`?2004h`, was lost; 50 ms after `?2004h` ran).

### Safe terminal wake & draft guard

Expose `Terminal::bracketed_paste(&self) -> bool` on `loopflow-terminal`'s
`ghostty_vt::Terminal` using the existing `DATA_MODE` + `ModeConfig` FFI call with
`MODE_BRACKETED_PASTE: u16 = 2004`.

In `loopflow-execution/src/transport/`:

1. **Distinguish user keystrokes from terminal traffic** (`input.rs`): extend
   `input::keys` (or add a sibling predicate) to ignore terminal query replies
   (already handled by `report_length`: DA, CPR, theme, focus `\x1b[I`/`\x1b[O`)
   **and** SGR/X10 mouse events (`\x1b[<...M`/`m`), so focusing a window, attaching
   a viewer, or scrolling with the mouse wheel never counts as a typed draft.
2. **Track exact draft and yield state per `Hosted` terminal** (`hosted.rs`):
   - `last_user_key: Option<Instant>` — updated on actual user keystrokes only.
   - `unsubmitted_input: bool` — set `true` on user keystrokes; cleared `false`
     when keystrokes end with bare `\r` (outside a paste), `\x03` (Ctrl-C), or
     `\x15` (Ctrl-U). This protects type-ahead drafts typed *before* `Stop` as
     well as drafts typed after.
   - `yielded_at: Option<Instant>` — updated by `Registry::note_yielded(session)`
     when `boundary::answer` records a turn end or first-boundary idle state.
     Using `Instant` inside the server avoids 1-second truncation races against
     SQLite's integer-second `observed_at`. After a server restart, adopted
     terminals start with `last_user_key: None` and `unsubmitted_input: false`,
     so idle sessions wake immediately.
3. **Readiness predicate** (one check on `Hosted`, run both *before* `steer::claim`
   and atomically right before pushing to `PendingInput`):
   - VT has `bracketed_paste() == true` (`?2004h` is on),
   - `!pending_input.in_paste()` (no viewer is mid-paste),
   - `!unsubmitted_input`, and
   - `last_user_key.is_none_or(|at| yielded_at.is_some_and(|y| at <= y))` (holds
     off during the window between Enter and `PreInvocation`, and whenever the
     person typed after the last yield).
   Checking readiness *before* `steer::claim` mirrors `wake.rs`'s existing
   "connect before claiming" rule: if the person is typing, steers are never
   held under a 15-second `CLAIM_SECS` lock that would hide them from the person's
   own Enter.
4. **Sanitize before framing**: strip `\x1b` and control bytes `< 0x20` other than
   `\n` and `\t` (plus `\x7f`) from the rendered steer text before wrapping in
   `\x1b[200~ <text> \x1b[201~\r`, so steer content (e.g. a comment or terminal
   excerpt) can never break out of bracketed paste and inject raw keystrokes.
5. **Paste-ready wakeup off the terminal loop**: when live (non-replay) output
   transitions `bracketed_paste()` from `false -> true` on a terminal bound to a
   `Session` (initial startup readiness, or returning from a `/` menu or
   `$EDITOR`), `Hosted` sends the `LfSessionId` on a channel to `wake.rs`, which
   runs `wake::after_turn` off the `held-terminals` thread so SQLite never blocks
   terminal I/O.

If any check fails, the wake leaves the steers unclaimed and they wait for the
next boundary or next `?2004h` transition. A store or server that is down changes
nothing: typed input still passes straight through, and the wake does not run.

Passed over: `agentapi` with an inherited token (routes through whichever `agy`
launched `lf`; fixed below); screen-scraping the prompt line (fragile); writing a
global sidecar into `~/.gemini` (`lf` writes no global provider settings).

**Found while probing (fixed here).** `agent::CALLER_SESSION_ENV` strips Claude,
Codex and OpenCode markers but no `ANTIGRAVITY_*`. A nested `agy` launched from
inside another `agy`'s tool shell inherits `ANTIGRAVITY_LS_ADDRESS`,
`ANTIGRAVITY_CSRF_TOKEN` and `ANTIGRAVITY_SOURCE_METADATA`, routing child
`agentapi` calls to the parent server. Add all ten `ANTIGRAVITY_*` markers `agy`
sets: `ANTIGRAVITY_AGENT`, `ANTIGRAVITY_AGENTAPI_EXE`, `ANTIGRAVITY_APP_DATA_DIR`,
`ANTIGRAVITY_CONVERSATION_ID`, `ANTIGRAVITY_CSRF_TOKEN`, `ANTIGRAVITY_LS_ADDRESS`,
`ANTIGRAVITY_LS_VERSION`, `ANTIGRAVITY_PROJECT_ID`, `ANTIGRAVITY_SOURCE_METADATA`,
`ANTIGRAVITY_TRAJECTORY_ID`.

## Reopening

`agy` runs no hook until its first model call. Like Codex in LOO-1068, a managed
reopened `agy` work session reports its first `session.boundary` at launch:
`SessionStart` (`source: "resume"`) when it opens without `-i`, and
`UserPromptSubmit` when it opens with `-i`.

- Move Codex's `report_first_boundary` (`harness/codex.rs`) into a shared helper
  (`harness::common::report_first_boundary`), not a second copy.
- **Timing:** do **not** send `report_first_boundary` inside `antigravity.rs::open()`
  (which runs before `which_command`, before `context_environment` reserves
  `SteerAdapter` and identifies the delivery file, and before `exec_native`
  records `agent_pid` / `agent_os_started_at`). Instead, attach the planned first
  boundary to `NativeClient` (only when `launch.kind.is_managed()` and a resumed
  `thread` is known) and send it inside `agent_process::exec_native` /
  `spawn_native` right after `record_agent_process_identity` and before
  `command.exec()`.
- A brand-new `agy` work session has no conversation ID until its first hook; it
  opens with `-i` and its first `PreInvocation` reports it.
- In `lf/commands/context.rs::read_news`, replace the direct
  `store.idle_conversation_candidates` call with `loopflow_execution::steer`'s
  shared reach check, and never defer steers to an idle wake when `prompt == true`
  (because `agy`'s `PreInvocation` only fires when a turn is already starting).
- **Live check in Pass 2:** verify whether real `agy 1.5.16` executes `-i <prompt>`
  when combined with `--conversation <id>` (used by `take_steers_to_start` on
  `lf resume --task`). If `agy` ignores `-i` on `--conversation`, skip
  `take_steers_to_start` for harnesses with `idle_wake == IdleWake::Terminal` (or
  a capability flag) so waiting steers stay unread and are delivered once by
  terminal wake on `?2004h` instead of being marked delivered and dropped.

## Source of truth

### 1. Runtime idle wake (`Capabilities.idle_wake`)

- Add required field `pub idle_wake: IdleWake` (no `Option`, no `None`) to
  `harness::Capabilities`:
  - `IdleWake::Inbox` (Claude)
  - `IdleWake::AppServer` (Codex)
  - `IdleWake::Socket` (OpenCode, Pi)
  - `IdleWake::Terminal` (Antigravity)
- **Remove reach dispatch from SQL completely.**
  `SqliteStore::idle_conversation_candidates(&self, session: Option<&str>)`
  returns raw stored facts (`IdleCandidate`: `adapter`, `provider`, `thread`,
  `cwd`, `endpoint`, `inbox`, `socket`, `terminal: Option<TerminalId>`, `identity`)
  for every unfinished, interactive, yielded `AgentProcess` with a
  `provider_thread` and `steer_adapter`. No provider names, no `CASE`, no
  `terminal_providers` parameter in SQL; keeps `INDEXED BY agent_sessions_unended_agent`.
- `loopflow_execution::steer::idle_conversations` is the single owner of reach
  resolution: it parses `provider -> HarnessKind`, reads
  `launcher(kind).capabilities().idle_wake`, and maps the matching column into
  a flat `IdleReach`:
  - `IdleReach::Claude(PathBuf)`
  - `IdleReach::Codex(PathBuf)`
  - `IdleReach::Socket(PathBuf)`
  - `IdleReach::Terminal(TerminalId)`
  An `agy` session with `held_terminal_id = NULL` (bare terminal / chat) yields
  no `IdleConversation`.
- `server/service.rs` passes `terminals: Option<Arc<Registry>>` into
  `boundary::answer` (for `note_yielded`), `boundary::after`, and `wake`.
  `Registry` exposes:
  - `note_yielded(&self, session: &LfSessionId)`
  - `terminal_wake_ready(&self, terminal: &TerminalId) -> bool`
  - `type_steers(&self, terminal: &TerminalId, text: &str) -> Result<bool>`

### 2. Executable UX parity contract (`harness::parity`)

Create `rust/loopflow-execution/src/harness/parity.rs`:

- `pub enum Behavior` (exhaustive, with `Behavior::ALL`)
- `pub enum Support { Yes, Gap { why: &'static str, owner: &'static str } }`
- `pub enum Verification { Automated(&'static str), RealProvider(&'static str) }`
- `pub fn support(harness: HarnessKind, behavior: Behavior) -> Support` — exhaustive
  match over `(HarnessKind, Behavior)` with no wildcard arms.
- `pub fn verification(behavior: Behavior) -> Verification` — distinguishes the 12
  behaviors verified across `HarnessKind::ALL` by automated tests from behaviors
  whose real-provider failure cannot be reproduced honestly by a stand-in script
  (currently `ViewerHandoff` / LOO-1097).

| Behavior | Verification | Current gaps |
|---|---|---|
| `WorkerStep` (runs, settles, records answer & thread, Graph advances) | Automated (`a_task_step_runs_as_a_worker_on_every_harness`) | none |
| `UsageRecording` (records usage in Task history) | Automated (same test) | none |
| `StepContext` (receives step context) | Automated (same test) | none |
| `MidTurnSteer` (hears steer after next tool) | Automated (same test) | none |
| `HeadlessPermissions` (never asks permission whatever `yolo` says) | Automated (`harness::launch_matrix`) | none |
| `MissingCli` (passed over when uninstalled, or named once when sole choice) | Automated (`a_step_that_cannot_launch_is_not_started_again`) | none |
| `UnansweredQuestion` (fails turn with question when nobody is there) | Automated (`a_question_with_nobody_there_ends_the_step`) | Claude, Codex, Antigravity (Task 48edb9) |
| `Interrupt` (stops on `lf task interrupt`, next run retries step) | Automated (`a_worker_stops_on_interrupt_and_the_next_run_starts_its_step_again`) | none |
| `RefusalFallthrough` (classifies quota, revoked login, unserved model) | Automated (`conformance_tests::each_harness_classifies_its_own_refusals`) | none |
| `IdleWorkSessionWake` (idle work session hears a steer) | Automated (`server::wake::tests::every_harness_wakes_its_idle_work_session`) | none |
| `ReopenWaitingSteers` (reopened work session takes waiting steers) | Automated (`server::wake::tests::every_harness_takes_waiting_steers_on_reopen`) | none |
| `InstalledSkills` (synced skills & Graphs listed) | Automated (`conformance_tests::every_harness_reads_a_synced_skill`) | none |
| `ViewerHandoff` (opening in another viewer keeps the conversation) | RealProvider (`transport_tests` proves held-terminal sharing; provider TUI checked live) | Antigravity (LOO-1097) |

## Conformance

Every `Verification::Automated` behavior test loops over `HarnessKind::ALL` with
an exhaustive `match` (never a hardcoded subset or string array) and checks
`parity::support(harness, behavior)` both ways: `Support::Yes` must pass, and
`Support::Gap` must still exhibit the gap, so fixing a gap fails the test until
`parity.rs` and the README are updated. Failure messages name `{harness:?}` and
the behavior.

Specifically:
- `server::wake::tests::every_harness_wakes_its_idle_work_session`: loops
  `HarnessKind::ALL`, sets up each harness's idle reach from
  `Capabilities.idle_wake` (Claude inbox, Codex app-server stand-in, OpenCode/Pi
  socket stand-in, Antigravity held-terminal stand-in that enables `?2004h` and
  echoes the paste), saves a steer, and asserts one turn start. Also unit-tests
  terminal draft guard (focus/mouse allowed, mid-turn and post-yield drafts held
  off, no 15s claim lock when held off, bracketed-paste sanitization).
- `server::wake::tests::every_harness_takes_waiting_steers_on_reopen`: loops
  `HarnessKind::ALL` and verifies a reopened work session starts a turn on waiting
  steers once before anyone types (for Antigravity, via launch first-boundary +
  `?2004h` paste-ready wakeup).
- `task_graph_launch_tests::a_task_step_runs_as_a_worker_on_every_harness`: switch
  the hardcoded 5-tuple array to `for harness in HarnessKind::ALL`.
- `task_graph_launch_tests::a_step_that_cannot_launch_is_not_started_again`: check
  missing-CLI install hint and pass-over across `HarnessKind::ALL`.
- `task_graph_launch_tests::a_question_with_nobody_there_ends_the_step`: loop
  `HarnessKind::ALL`, asserting `Support::Yes` fails with `unanswered_question`
  and `Support::Gap` does not.
- Unify the three split interrupt tests (`…_on_claude_and_codex`,
  `…_on_opencode_and_pi`, `…_on_antigravity`) into one loop over `HarnessKind::ALL`.
- `harness::conformance_tests::each_harness_classifies_its_own_refusals`: loop
  `HarnessKind::ALL` with an exhaustive `match` so every harness has refusal
  classification fixtures.
- `parity::tests::readme_table_matches_support`: renders the parity table between
  markers (`<!-- parity:start -->` / `<!-- parity:end -->`) in
  `rust/loopflow-execution/src/harness/README.md` and asserts exact match.
  `cargo test -p loopflow-execution --lib harness::parity::tests::regenerate_readme_table -- --ignored`
  rewrites it. Update `rust/loopflow/src/server/README.md` ("Waking an idle
  conversation") to describe Antigravity's held-terminal wake alongside the other
  four.

## Absent and error states

- Terminal gone, child exited, `held_terminal_id` is `NULL`, or VT unreadable:
  readiness check returns `false` before claiming; steers remain unclaimed and
  wait for the next boundary.
- Paste mode off (`?2004l` — `/` menu, auth screen, `$EDITOR`, or TUI booting):
  readiness check returns `false` before claiming; the next `?2004h` transition
  triggers `wake::after_turn`.
- Person typing (idle draft or mid-turn type-ahead, or Enter before
  `PreInvocation`): readiness check returns `false` before claiming; their Enter
  delivers waiting steers through `PreInvocation` without a 15-second claim lock.
- Focus events, terminal query replies, mouse wheel scrolling: ignored by draft
  tracking; never disable idle wake.
- `agy` mid-turn (`yielded = 0`): not an idle candidate; `PreInvocation` delivers
  between tools.
- Worker or chat `agy`: not interactive or no held terminal; never a candidate.
- Store or server unavailable: held terminal stays a pure pass-through; wake does
  not run.

## Operational boundary

No new process or poll per Session. `bracketed_paste()` reads state `libghostty-vt`
already tracks. Draft tracking adds two fields (`Option<Instant>`, `bool`) and one
`Option<Instant>` (`yielded_at`) per `Hosted` terminal. `?2004h` wakeup sends on
an in-process channel and runs off the `held-terminals` loop. `idle_conversation_candidates`
keeps `INDEXED BY agent_sessions_unended_agent` with fewer predicates than before;
verify with `query_plans.rs` (`EXPLAIN QUERY PLAN` against the 10k-Task fixture).

## Exclusions

- Real-provider viewer handoff for `agy` (LOO-1097): tracked as a typed
  `RealProvider` `Gap` owned by LOO-1097, not fixed here.
- Reading a headless question on Claude, Codex, or Antigravity: tracked as a
  typed `Automated` `Gap` owned by Task 48edb9.
- Offline gate check verifying that a `Gap`'s owner Task is still open.

## What goes

- Hand-written parity table in `harness/README.md`, the stale `Task 540073` cell,
  and the "Antigravity takes queued steers on its next turn" / "any of the four"
  prose in `harness/README.md` and `server/README.md`.
- SQL `CASE` / `OR` provider-reach dispatch in `sqlite/steers.rs::idle_conversation_candidates`
  and `context.rs`'s direct `idle_conversation_candidates` call.
- Codex-only `report_first_boundary` (moved to `harness::common`, not copied).
- Three-way split interrupt tests and hardcoded harness arrays in behavior tests.

## Passes

1. **Terminal wake on a live idle `agy`** (first slice — proves the core mechanism
   end to end).
   - `Terminal::bracketed_paste()` (`MODE_BRACKETED_PASTE = 2004`) in
     `loopflow-terminal`.
   - `IdleWake` on `Capabilities`; remove SQL reach dispatch from
     `idle_conversation_candidates`; flat `IdleReach` (including
     `Terminal(TerminalId)`) resolved in `loopflow_execution::steer::idle_conversations`;
     `context.rs::read_news` switched to the shared helper with `!prompt` guard.
   - `Hosted` user-keystroke draft guard (filtering reports + mouse, tracking
     `last_user_key`, `unsubmitted_input`, `yielded_at`), paste sanitization, and
     `Registry::{note_yielded, terminal_wake_ready, type_steers}`; wire `terminals`
     through `service.rs` -> `boundary` -> `wake`.
   - Add the 10 `ANTIGRAVITY_*` markers to `CALLER_SESSION_ENV`.
   - Unit tests for VT mode, draft guard, paste sanitization, `query_plans`, and
     the held-terminal stand-in wake test.
   - Live check on real `agy` in a held terminal on this Machine: let it settle,
     `lf steer` -> one turn starts, visible in the TUI, `lf session steers` marks
     it delivered once, `PreInvocation` does not duplicate it.
2. **Reopen on `agy`.**
   - Move `report_first_boundary` to `harness::common`; wire `NativeClient` first
     boundary into `agent_process::exec_native` / `spawn_native` after
     `record_agent_process_identity`.
   - Emit `?2004h` `false -> true` paste-ready events off the `held-terminals`
     loop into `wake.rs`.
   - Live check on real `agy`: verify both `agy --conversation <id> -i <prompt>`
     (for `take_steers_to_start`) and quit -> Task update / steer -> `lf resume --task`
     -> waiting steer runs once before anyone types. If `agy` ignores `-i` with
     `--conversation`, keep waiting steers for terminal wake instead of
     consuming them in `take_steers_to_start`.
3. **Typed parity contract & full `HarnessKind::ALL` conformance.**
   - `harness/parity.rs` (`Behavior`, `Support`, `Verification`, `support`,
     README renderer & check).
   - Switch `server::wake::tests::` (idle wake + reopen), `task_graph_launch_tests`
     (worker, missing CLI, unanswered question, unified interrupt), and
     `conformance_tests::each_harness_classifies_its_own_refusals` to exhaustive
     `HarnessKind::ALL` loops checking `parity::support` both ways.
   - Regenerate `harness/README.md` table and update `server/README.md`.
   - Demo 2 check: flipping Antigravity's `IdleWorkSessionWake` to a `Gap` or
     removing `IdleReach::Terminal` fails `server::wake::tests::` and the README
     check, each naming Antigravity.

## Docs updated

Written before implementation so the next step implements against them:

- `docs/sessions.md` — idle wake on all five harnesses (including Antigravity's pasted-prompt wake and draft guard), reopened work sessions taking waiting steers, and nested-session isolation for Antigravity.
- `docs/config.md` — Antigravity section covering idle work session wake, draft guard, and reopen with waiting steers.
- `rust/loopflow-execution/src/harness/README.md` — `Capabilities.idle_wake`, `<!-- parity:start -->` / `<!-- parity:end -->` generated parity table covering all 13 `Behavior` rows and `Verification::RealProvider`, and updated idle-wake/reopen prose.
- `rust/loopflow/src/server/README.md` — `Capabilities.idle_wake` reach resolution, Antigravity held-terminal wake and draft guard, and reopened work sessions across all five harnesses.
- `docs/dev/codebase.md` — "Add a provider" checklist updated for `Capabilities.idle_wake`, `harness::parity::support`, `HarnessKind::ALL` conformance suites, and parity-table regeneration.

## Acceptance at gate

```sh
cargo test -p loopflow-terminal --lib ghostty_vt::
cargo test -p loopflow-storage --lib query_plans::
cargo test -p loopflow-execution --lib harness::parity::
cargo test -p loopflow-execution --lib harness::conformance_tests::
cargo test -p loopflow-execution --lib transport::
cargo test -p loopflow --lib server::wake::tests::
cargo test -p loopflow --lib server::boundary::tests::
cargo test -p loopflow --lib task_graph_launch_tests::
```

Plus real `agy` demo verification: Pass 1 (idle steer) and Pass 2 (quit, steer,
reopen) in a held terminal, with one turn visible in the TUI and one
`delivered_at` per steer.
