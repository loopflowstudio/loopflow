# LOO-823 process leaks

Outcome: account for every resident lf process, fix fixture/demo leaks and
observer lifetime bugs, and show the affected suites leave no new lf, provider
or Dev-app processes. Existing processes are report-only: no signaling anything
this work did not start, no name-pattern kills, no synthetic CPU load, and no
step-tree reaper in lf.

Report: `uv run python -I scratch/process_audit.py` writes `scratch/process-audit.html`
(read-only; it opens `loopflow.db` with `mode=ro` for shell receipts).

Readings, 2026-11-10 ~17:30, 343 lf processes:

- 113 `session observe-status`, every one with its Desktop client gone or
  replaced. `lsof` on PID 2046: its stdin pipe's write end (Desktop fd 43) was
  held by 15 pane processes (owners, zsh, codex, git), so EOF never came.
- 24 from `/tmp/lf-7501-*`: `transport_tests` owners and stand-ins up to 8 days
  old; the fixture had no Drop. 297 `/tmp/lf-7501-*` dirs remain from them.
- Owners: 78 held by `native input` (any unacknowledged byte: arrows, Esc,
  an edit), 1 by a running Graph, 9 Desktop shells (below); rollover's dry run
  moved 0.

- `__codex-observe` (50), `__context-block` (23): all beside a live parent.
- 17 demo/Dev-app (LOO-672 Redemo owners, Dev-app observers) from
  `~/.lf/demos/` and `/tmp`; no script here started them.
- 18:40: no `lf` from any other test fixture's home or a `target/` path; the
  `/tmp/lf-7501-*` ones are all 3+ days old.

Since `ab682d71f`, `observe-status` is one write per reading; a resident one
is from an older Desktop.

Remaining at demo: regenerate the audit report (the committed one is the 20:20
reading: 354, 358 at its own second `ps`, 358 on a separate recount) and show
the affected test suite's process diff again. Earlier checks and the pre-review
concept discussion are in `551becf586687262921e0681a169b5b7973fc75b:scratch/process-leaks.md`.
The audit describes the installed build at the time; its input holds disappear
with this change. `scratch/qa.html` predates this lifetime revision.

## Desktop shells (read 2026-11-10 18:55)

The nine owners rollover reads as "no open Session" are unviewed Desktop
shells; the report lists them under `desktop shell` with Task and state from
`desktop_shells`, `task_retirements` and `lf task status`.

- 8933 (LOO-434 active), 88963 (LOO-774 shipping), 92273 (MTG-269 active):
  unfinished Tasks; a hidden shell runs until Close.
- 32670 (LOO-775), 55749 and 89413 (LOO-776), 89840 (LOO-812), 24677
  (MTG-252), 8687 (MTG-268): Task done, `close_requested=0`. The close path
  never started: `retirement::advance` stops at `require_settled_work` because
  the Task's interactive owner has `OwnerRetention::NativeInput`, so the
  retirement stays `waiting` and `request_retirement` is never reached. The
  sweep itself runs (`com.loopflow.cleanup`; `Task cleanup pending` lines in
  `logs/lf.log` through 18:53). No shell-specific bug: it is the input hold in
  the five Tasks listed above.

## Design: conversations live as long as their scope (2026-11-11)

Direction (Jack Heart, word for word, in the demo-step review):
- "with bare lf, noting holds its at all"
- "with task sessions, i think we can just hold them until the task is closed basically"
- "wave/repo should behave exactly like open tasks tho"
- On what to change before bare-direct lands: "whatever gets to the cleantest possible world fastest. transition costs ok"
- Where: "3a - here" (this Task, PR #1877). "Closed" means both done and abandoned ("4a").
- Suspension under a resident limit: "6a" (agent count per Machine, in config); "no opinion on 5".

Implemented; gate and demo remain:

1. Conversations no longer expire from quiet output or absent viewers.
   Abandonment ending, Desktop shell Close, and completed-Task retirement remain.
   The shared `end_detached_after` config key is parseable but ignored until
   LOO-829 removes it after release.
2. Input flows through the bounded byte queue without a store write, retention
   ledger or retry recording. The released `processes.native_input` column has
   no runtime consumers; its removal needs a later staged contract migration.
3. Completed-Task cleanup ends conversations with unsent input after viewing
   grace and live work settle. A running Graph or pending request still holds
   cleanup; abandonment retains its existing rule of ending unviewed terminal
   owners even during a turn.
4. Explicit rollover and takeover may replace an idle provider with a draft,
   discarding its editor. Help and docs describe that loss. Regression coverage
   includes retirement with typed input and forwarding during store locks.

Not in this PR: suspension under a limit (6a). It needs to know the editor is
empty, and the ledger is being deleted; proposed (nobody picked it) to build
it with the server-first holder and read the input line off the server's
emulator (5a). Bare launches with no owner stay the server-first piece; the
14 unscoped owners in the audit run until someone quits them during this
transition.

## Current approach and related work

The lifetime and input deletions are implemented, including the demo teardown
and Swift README corrections. Quitting a demo app alone leaves its owners
running; demo teardown quits each provider after live work settles and keeps
its saved history. Fixture teardown joins recorded processes before deleting
their home, including completed owners still writing their final screen.
Desktop pipes use close-on-exec so unrelated panes cannot retain their writers.

Kept on purpose: `Ending::may_end` still vetoes completed-Task retirement when
output arrived after the judgment. Hooks are best-effort, so streaming output
is the one sign of a turn the store never recorded. It no longer times anything.

No new product choice. Still open, and not this PR's: the resident-limit
default count and how suspension knows the editor is empty (`questions.md`).

Concept review, 2026-11-11: no further implementation direction or open choice
for LOO-823. Advance to verification. The review page now shows this model;
its former input-ledger alternatives and 01:10 store reading remain in
`a6bb4d5a5:scratch/concept-review.html`.

The interaction to show: leave an open Task with a draft, return to the same
provider, then complete the Task and leave; eligible cleanup ends it despite
the draft. A locked store must neither stall input nor authorize closure;
recovery retries the closure judgment without replaying input. A return before
cleanup cancels it; after cleanup, history and the saved screen survive, but
the editor and deleted checkout do not return. Keep the missed-hook
counterexample: output arriving after a cleanup judgment invalidates it even
when no live turn was recorded. Abandonment deliberately differs: it can end
an unviewed conversation during a turn.

Consequences of the rule, no change here:

- `BACKGROUND_TASK_CONVERSATIONS` (16, `ops/human_session/primary.rs`) counts
  every running Task conversation. While conversations ended after two quiet
  minutes it bound only in bursts. Now, on a Machine with sixteen open Tasks
  whose conversations run, every later `lf task run` skips its background start
  and the conversation begins at `lf resume` or when Desktop opens it. It is a
  second agent count beside the deferred per-Machine one ("6a"); see
  `questions.md`.
- `.lf/wave/infrastructure/MEMORY.md`, "Idle owners end", records the superseded
  rule ("idle conversations to cost nothing", the two-minute limit, editable
  drafts). This Task has no Wave and does not write there. Proposed replacement
  for whoever maintains it: conversations stay running while their scope is
  open; Task closure, quitting, `lf resume`, rollover or an explicit end are the
  only endings; typed input holds nothing; an idle state under a resident limit
  is deferred (quotes in `questions.md`).

## Remaining verification

The branch contains the current local `origin/main` (`976b53db0`), including
#1882's conversation-opening readiness changes and #1884's package migration
ordering. The sync repair removed four references to the deleted `StopOwner`
from three new tests; cleanup remains owned by `Fixture::drop`.
LOO-824 will adopt this lifetime rule after this PR lands (exact coordination
quote in `questions.md`); resident-limit suspension still awaits its emulator
and a decision on how to detect an empty editor.

The late screen directories noted in `qa.html` need a fresh SSH-suite check at
gate. The audit no longer calls an open conversation stale solely for being old
and unviewed; regenerate it at demo rather than presenting the pre-revision
readings as current behavior.

- `cargo test -p loopflow --lib -- transport::lifetime::tests:: transport::lifetime_tests::`: 7 passed after compression; linker warned that `__eh_frame` exceeded its compact-unwind size limit.
- `cargo test -p loopflow-test-support cleanup_tests::`: 1 passed; completed-process final write precedes cleanup return.
- `cargo test -p loopflow --lib -- retirement_waits_for_final_work_and_shell_job_settlement obsolete_detached_limit_is_readable_but_not_written`: 2 earlier passing cases unchanged.
- `cargo test -p loopflow --test transport_tests -- completed_task_cleanup_ends_a_conversation_with_typed_input a_locked_store_does_not_delay_input_or_replay_it_on_return resumed_session_completes_turns_after_build_replacement_and_viewer_return unanswering_superseded_owner_preserves_pending_turn_and_queued_input`: 4 passed across focused runs after compression. The responsive upgrade case initially failed before startup because macOS denied system Python 3.13's `termios` signature; `uv run cargo test` with that filter passed using the repository Python.
- `cargo test -p loopflow --test transport_tests -- quiet_unviewed_conversations_ignore_the_obsolete_detached_limit silent_native_turns_survive_until_settlement native_background_watches_survive_until_settlement`: 3 earlier passing cases; gate reruns them with the changed fixture teardown.
- `swift build --build-tests --package-path swift`, then `scripts/test_desktop.sh --skip-build --filter UninheritedPipeTests`: built and 1 passed in 0.001s. The first run spent 30s in SIGTERM teardown; SIGKILL removes the inherited-mask wait. The rebuilt test bundle initially failed signature validation; local `codesign --force --sign - --timestamp=none` on that bundle restored the headless check.
- `ps -axo pid=,command=` before/after focused Rust tests: 42 prior matching processes remained, no new fixture/build process (excluding the reading commands). Nothing pre-existing was signalled.
- `cargo fmt --check`, `git diff --check`: passed; audit script syntax parses.
- `uv run cargo test -p loopflow --test transport_tests -- conversation_open_` (inherited `LF_*` unset, stdin closed): 3 passed after the sync repair; 11.74s.
- `uv run cargo test -p loopflow --test integration documented_commands::` (inherited `LF_*` unset, stdin closed): 4 passed after replacing the retired status phrase in `docs/troubleshooting.md` found on the first run.
- Compress follow-through: `git diff --check` passed; obsolete symbol/doc search found no remaining runtime references. Only prose and a test comment changed; no build or test rerun. `lf context`: within both targets.
- 2026-11-11 realign: `lf context` and `lf task status LOO-823 --json` succeeded; no outstanding context-loading failure. Code inspection agrees with the lifetime direction; no code repair or test rerun in this pass.

Gate: affected transport, retirement, native-turn/steers, rollover, config,
CLI resolution/parser, server schema and `documented_commands::` suites; Clippy.
`cargo package --locked --allow-dirty -p loopflow` remains with gate for the
embedded demo skill edit. Demo: process diff and regenerated audit. Full build/test results before
this lifetime revision do not cover these deletions.
