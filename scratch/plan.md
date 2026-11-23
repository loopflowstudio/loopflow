# LOO-1068: a reopened work session takes its waiting steers as a turn

Quit a Claude, Codex, OpenCode or Pi work session, `lf steer` it, reopen it:
on each, a turn starts on the steer before anyone types, once. Today only
OpenCode and Pi do; Claude and Codex fold the steer into the next typed turn.

LOO-974 (#1988) is merged (`3578f78f6d` is this branch's base); no `lf sync`.

## Why Claude and Codex fold today

- **The start-hook claim predates the wake.** `emit_block` → `read_news`
  (`rust/loopflow/src/lf/commands/context.rs`) claims steers at every moment,
  Start included, since #1923 ("every launch reads its own steers"). #1890
  (the wake) came later and never revisited it. No recorded reason: the
  claim at Start is just "a boundary like any other". Nothing found tying it
  to #1972's recovery relaunch: a relaunched conversation is a new process,
  which the new rule below covers.
- **Claude races today.** Claude's SessionStart hook names its inbox;
  `record_agent_inbox` resets `session_activity` to idle and returns
  `After::Reachable`, so the server wakes in `boundary::after` while the
  hook, which just got its answer, claims the same steers in `read_news`.
  Whichever claims first wins; the hook usually does, so the steer lands in
  context with no turn.
- **Codex has no pre-turn boundary at all.** Codex runs SessionStart hooks
  lazily inside the first turn: `run_pending_session_start_hooks` is called
  from `codex-rs/core/src/session/turn.rs:425` (openai/codex main, read
  2026-11-22). A reopened Codex conversation reports nothing until someone
  types, and its endpoint is written by lf's launch
  (`record_session_connection` in `harness/codex.rs::open_native`), never by
  `record_agent_inbox`. So the wake never runs on Codex reopen.

## Design

**One owner for "a new provider process is idle until its first turn":**
`SqliteStore::record_first_boundary(adapter) -> StoreResult<bool>` in
`loopflow-storage/src/sqlite/steers.rs`. When `session_activity` names
another AgentProcess, or none, or `observed_at=0` (a held terminal's status
stream got there first, `begin_program_status`), it writes this process's
row as idle (`yielded=1, open_tools=0, pending_input=0`, attachment token,
program status kept only for the same process) and answers `true`. This is
the SQL that sits in `record_agent_inbox` today, moved.

`boundary::answer` calls it first, for every boundary with an adapter,
before `record_native_turn`. So a first boundary that is itself a prompt or
a Stop ends in the state that boundary says, and the wake (which reads
`yielded` when it runs) does nothing for a working conversation.

`record_agent_inbox` goes back to saving an address. `After::Reachable`
runs when the boundary was the process's first **or** named a new address
(an inbox key Claude publishes late still gets its wake). The wake claims,
so running it twice delivers once.

**Work sessions leave waiting steers to the wake.** In `read_news`, a Start
the server answered on an interactive Session claims nothing; it still
writes Task updates and the startup baseline, and prints the context block.
The next tool or prompt boundary claims anything the wake could not deliver
(unreachable inbox, failed send), so the fallback is today's behavior.
Chats (no server, `LF_BARE_STEER`) and workers (`interactive=0`, never
woken) keep the start claim. Made for review: skip keyed on
`answered && interactive` rather than a new answer field, because
`SessionBoundaryAnswer` is `deny_unknown_fields` and hooks from the released
build would reject a new field.

**Codex reports its own first boundary.** `open_native` already records the
connection and binds the delivery before the TUI attaches. It then sends one
`session.boundary` through `machine::client::hook_request` with the
launch's adapter and delivery, as the hook would:
- no opening turn (a reopen): moment `Start`, callback
  `{"hook_event_name":"SessionStart","session_id":thread,"source":"resume"}`
  → first boundary → idle → `After::Reachable` → `deliver_update` on the
  app-server. The turn runs before the TUI joins, as lf's opening turn for a
  new launch already does.
- an opening turn: moment `Tool`, `UserPromptSubmit`, sent **before**
  `start_turn`, so the process is working before Codex's own in-turn
  SessionStart hook arrives.
A server that does not answer is a recording gap; the steer waits for the
first typed turn, as today.

**Opening-prompt race** (LOO-974 questions): Claude launched with a prompt
reports Start (first → idle → wake) and then UserPromptSubmit. The wake may
queue the steers as Claude's next turn while the prompt runs. Claims are
exclusive, so the steers arrive once; the prompt and the wake each run
once. Codex avoids the race by reporting the prompt first. Accepted as is.

## Risk to answer in the first slice

Claude's inbox at SessionStart. The wake writes a `priority:"next"` message
with no acknowledgement and stamps it delivered. If Claude, still starting,
drops a message that early, the steer is lost, not delayed. Probe on a real
Claude reopen before building the rest; if dropped, the wake for a first
Claude boundary waits for Claude's `sessions/<pid>.json` `status` to read
`idle` (bounded, one retry) rather than sending at once.

## Done when

On each of Claude, Codex, OpenCode and Pi: quit a work session, `lf steer`
it, `lf resume` it; a turn starts on the steer before anyone types, and
`lf session steers <session>` shows it `delivered` once. A Claude or Codex
work session launched with an opening prompt and a waiting steer runs the
prompt once and hears the steer once.

## Acceptance check at gate

- `cargo test -p loopflow --lib server::boundary::tests:: server::wake::tests::`
  - a first boundary records a new process idle whatever the last one did;
    a later boundary of the same process does not.
  - a first boundary that is a prompt leaves it working; the wake sends
    nothing.
  - a Codex launch's start report wakes an idle reopened Session through its
    endpoint (`wake::tests::codex_app_server` stand-in).
- `context.rs` unit: an answered Start on an interactive Session claims no
  steer; a chat's Start claims it.
- `tests/e2e/idle_wake.py` by hand on OpenCode and Pi (reopen leg), plus the
  same reopen by hand on real Claude and Codex. Pi is not installed on this
  Machine: `npm install -g` it at Machine level for the run.

## Surfaces

- `loopflow-storage/src/sqlite/steers.rs`: `record_first_boundary` (new),
  `record_agent_inbox` (reset removed).
- `loopflow/src/server/boundary.rs`: call order, `Reachable` condition.
- `loopflow/src/lf/commands/context.rs`: Start claim skip.
- `loopflow-execution/src/harness/codex.rs::open_native`: first-boundary report.
- `loopflow/src/server/README.md`: "Every launch reads its own steers" (work
  sessions leave start steers to the wake) and "Waking an idle conversation"
  (replace "A conversation reopened after it was quit runs no turn until
  someone types… Claude's start hook reads those steers into its context
  instead"). `harness/README.md` reopen paragraph.
- No wire change, no migration.

## What goes

- The `session_activity` UPDATE inside `record_agent_inbox`, and the
  comment saying a provider naming its inbox has run no turn.
- `boundary::tests` assertions that tie the idle reset to inbox naming;
  rewrite them against the first boundary.
- README sentences above.

## Slices

1. **Built:** `record_first_boundary` (read first, so later boundaries take no
   write lock; interactive Sessions only), its call at the top of `answer`,
   `Reachable` on first-or-new-address, the Start-claim skip. The skip keys on
   "the server can wake this conversation" (`idle_conversation_candidates`
   names this process), not on `interactive` alone: a Claude whose inbox key
   is not yet published still folds the steer into its start, as today,
   instead of waiting for a typed turn. Tests: `boundary::tests`
   (`a_reopened_conversation_is_idle_at_its_first_boundary_and_woken_once`,
   `a_first_boundary_that_is_a_prompt_leaves_the_conversation_working`),
   `context::tests::a_wakeable_start_leaves_waiting_steers_to_the_wake`.
   **Not run:** the real Claude reopen probe (the inbox-at-SessionStart risk
   above). It needs a held terminal and the account's Claude login; run it at
   qa with the branch `lf` on a private Machine: quit, `lf steer`, `lf resume`,
   watch for a turn and `lf session steers` reading `delivered`. If Claude
   drops the early message, the steer shows `delivered` with no turn: then
   build the bounded `sessions/<pid>.json` `status: idle` wait in the wake.
2. **Built:** `open_native` reports Start (reopen) or the opening prompt
   (before `start_turn`) through `hook_request`; `HOOK_DEADLINE` moved to
   `machine::client`. Test:
   `wake::tests::a_reopened_codex_work_session_starts_a_turn_on_its_waiting_steer`
   (stand-in app-server, server side only). **Not run:** real Codex reopen.
3. **Remaining:** OpenCode/Pi reopen through `idle_wake.py` (the script has no
   reopen leg yet; add one: quit the TUI, steer, `lf resume`, wait for the
   marker); the unreachable-inbox fallback test; README rewrite is done.

## Checks

- `cargo nextest run -p loopflow --lib -E 'test(server::boundary::tests::) | test(server::wake::tests::) | test(lf::commands::context::tests::)'`: 25 passed (implement).
- `cargo test -p loopflow --test integration documented_commands::`: passed (implement).
- clippy, real Claude/Codex/OpenCode/Pi reopen: gate and qa.
