# LOO-823 process leaks

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

Build in this PR:
1. Delete idle ending: the quiet/viewer grace in `transport/lifetime.rs`,
   `idle_retention`, `DetachedLimit` and `session.end_detached_after`. Keep
   abandonment ending, Desktop shell Close, and completed-Task retirement ending.
   `.lf/config.yaml` is read by the released lf too and `SessionConfig` is
   `deny_unknown_fields`: keep `end_detached_after` parseable and ignored,
   marked with the Task that removes it, and file that Task (AGENTS.md rule).
2. Delete the native-input ledger: `store/sqlite/native_input.rs`,
   recording before forwarding in `transport/input.rs` (and its retry path),
   `acknowledge_native_input`, `OwnerRetention::NativeInput`. Stop reading and
   writing `processes.native_input`; dropping the column is a later staged
   contract migration, not this PR.
3. A closed Task ends its conversation at cleanup whatever its input:
   `retirement::require_settled_work` no longer waits on input. A running
   Graph or live turn still holds cleanup as today.
4. Rollover and takeover (`lf process rollover`, `human_session.rs`) lose
   input as a reason. Rollover is a person's or operator's explicit act; it
   now moves an owner with a draft the way quitting would. Say so in its
   help and docs.
5. Docs: `docs/sessions.md` (lifetime = scope; no two-minute rule),
   `docs/config.md`, the troubleshooting section this PR added (its "send or
   clear" sentence goes), transport README, `lifetime.rs` module doc,
   `docs/dev/server-first.md` (CLI cleanup row no longer lists
   `end_detached_after`).
6. Tests pinning the ledger or idle ending go; add one that a done Task's
   conversation with typed input ends at cleanup.

Not in this PR: suspension under a limit (6a). It needs to know the editor is
empty, and the ledger is being deleted; proposed (nobody picked it) to build
it with the server-first holder and read the input line off the server's
emulator (5a). Bare launches with no owner stay the server-first piece; until
then the 14 unscoped owners run until someone quits them, accepted as
transition cost.

## Delete — do not maintain

Runtime cuts below are complete. Only the ignored shared-config key and the
released SQLite column remain for their post-release removal.

- `transport/lifetime.rs`: `Grace`, `limit`, quiet/viewer idle expiry and its tests.
  Keep viewer observations, stale-candidate rejection, abandonment and retirement.
- `config.rs`: `DetachedLimit`; keep only the ignored parser until
  `task_c118380f31f14d07ba28669499efa395` removes shared-config compatibility.
- `store/sqlite/native_input.rs`, `OwnerRetention::NativeInput`,
  `acknowledge_native_input`, `try_record_owner_input`, `clear_owner_input`.
  Keep current-turn recording and the released column without runtime consumers.
- `transport/input.rs`: retention recording, budget and replay; delete
  `transport/input_retry_tests.rs` and its serve-loop hook. Keep the bounded
  byte queue and partial-write forwarding. Input must flow during store locks.
- `desktop::shells::holds_input`, `SqliteStore::idle_retention`, ledger-only
  steers/rollover fixtures and transport tests requiring idle expiry.
  Keep live-turn/request/Graph, shell Close and takeover coverage. The Task-Graph
  check belongs in `owner_retiring_tasks`; it does not hold explicit rollover.

## Remaining verification

The upgrade fixture uses native turn hooks for both responsive and unresponsive
owners, then one replacement/reconnect path; no synthetic live-turn SQL or
draft-retention branch remains. Abandonment skips retirement-only reads. Fixture
teardown waits for completed owners' final writes as well as unfinished owners;
the late screen directories noted in `qa.html` need a fresh SSH-suite check at gate.
The audit no longer calls an open conversation stale solely for being old and unviewed.

- `cargo test -p loopflow --lib -- transport::lifetime::tests:: transport::lifetime_tests::`: 7 passed after compression; linker warned that `__eh_frame` exceeded its compact-unwind size limit.
- `cargo test -p loopflow-test-support cleanup_tests::`: 1 passed; completed-process final write precedes cleanup return.
- `cargo test -p loopflow --lib -- retirement_waits_for_final_work_and_shell_job_settlement obsolete_detached_limit_is_readable_but_not_written`: 2 earlier passing cases unchanged.
- `cargo test -p loopflow --test transport_tests -- completed_task_cleanup_ends_a_conversation_with_typed_input a_locked_store_does_not_delay_input_or_replay_it_on_return resumed_session_completes_turns_after_build_replacement_and_viewer_return unanswering_superseded_owner_preserves_pending_turn_and_queued_input`: 4 passed across focused runs after compression. The responsive upgrade case initially failed before startup because macOS denied system Python 3.13's `termios` signature; `uv run cargo test` with that filter passed using the repository Python.
- `cargo test -p loopflow --test transport_tests -- quiet_unviewed_conversations_ignore_the_obsolete_detached_limit silent_native_turns_survive_until_settlement native_background_watches_survive_until_settlement`: 3 earlier passing cases; gate reruns them with the changed fixture teardown.
- `swift build --build-tests --package-path swift`, then `scripts/test_desktop.sh --skip-build --filter UninheritedPipeTests`: built and 1 passed in 0.001s. The first run spent 30s in SIGTERM teardown; SIGKILL removes the inherited-mask wait. The rebuilt test bundle initially failed signature validation; local `codesign --force --sign - --timestamp=none` on that bundle restored the headless check.
- `ps -axo pid=,command=` before/after focused Rust tests: 42 prior matching processes remained, no new fixture/build process (excluding the reading commands). Nothing pre-existing was signalled.
- `cargo fmt --check`, `git diff --check`: passed; audit script syntax parses.

Gate: affected transport, retirement, native-turn/steers, rollover, config,
CLI resolution/parser, documented-command and server schema suites; Clippy. Demo: process diff and regenerated audit. Full build/test results before
this lifetime revision do not cover these deletions.
