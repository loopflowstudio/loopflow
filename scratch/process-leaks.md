# LOO-823 process leaks

Outcome: account for resident lf processes, fix fixture/demo leaks and observer
lifetimes, and show affected suites leave no new lf, provider or Dev-app processes.
Existing processes are report-only. No signaling processes this work did not
start, name-pattern kills, synthetic CPU load, or step-tree reaper in lf.

PR #1877. Earlier design, experiments and verification are in
`f5cb942098e0ae9af447dfd350a38ff49d38be53:scratch/process-leaks.md`.
The same commit keeps the preceding `questions.md` and
`simplification-opportunities.md`; earlier alternative input-ledger designs are
in `551becf586687262921e0681a169b5b7973fc75b:scratch/process-leaks.md`.

## Direction, 2026-11-11

Jack Heart, word for word:

- "with bare lf, noting holds its at all"
- "with task sessions, i think we can just hold them until the task is closed basically"
- "wave/repo should behave exactly like open tasks tho"
- "whatever gets to the cleantest possible world fastest. transition costs ok"
- "3a - here" selects this PR; "4a" means done and abandoned.
- "6a" selects a per-Machine agent count in config; "no opinion on 5" leaves
  editor-empty detection undecided. Resident-limit suspension is deferred.
- Demo review: "1a 2b". Resume still replaces idle owners on another build and
  loses the draft; delete the sixteen-conversation background cap.
- Comment `2bcb32d3-8e6b-474d-8fcb-2cbaead48ce1`:
  "Review answers 1a 2b: drop the background conversation cap; owners obey a close request instead of judging Task closure (scratch/process-leaks.md)"

## Implementation contract

Open Task, Wave, repository and bare conversations remain running until quit or
explicit replacement. A done Task's conversations end after fifteen minutes
without viewing once live work settles, on a cleanup pass. Completion and a new
owner's launch are lower bounds on that grace. This applies without a checkout
and after checkout retirement. Abandonment ends unviewed terminal conversations
even mid-turn. Headless Runs retain their existing lifetime.

Task cleanup owns the decision and writes a close request on the existing
Process. Owners record viewing and consume requests; they do not read Task
state. The cleanup inventory includes closed Tasks with live conversations,
including missing or retired checkouts. Shell Close waits for requested work
in the cleanup producer; shells and conversations consume the same request path.
Shell viewing adds no Task hold, and explicit shell Close remains effective.

Returning viewing, reopening, changed membership and new work revoke automatic
requests. A revision check prevents a decision from overwriting an intervening
write. The final output veto remains: streaming can reveal work whose hook
never reached the store. Compare with the last observed absence of a request,
including output before the owner polls a newly written request. The watcher
cancels it outside the byte path; no idle timer is involved. Store failure cannot
authorize closure. Input is forwarded through the bounded byte queue with no
store write, ledger or retry recording.

Retirement still checks merged delivery before deleting a checkout; a PR-less
or unresolved checkout stays. Resuming retains provider history and the saved
screen, never an editor or replayed input. Ordinary shell jobs settle before
checkout deletion. `docs/sessions.md` and the transport README own this behavior.

### Delete — do not maintain

Runtime cuts are complete: idle ending; native-input ledger and retries;
`BACKGROUND_TASK_CONVERSATIONS` and its count/skip path; owner-side Task and
retention polls; the second Task judgment at consumption; runtime reads of
`desktop_shells.close_requested`. Released columns remain for staged removal
in LOO-840; the ignored config key goes in LOO-829.

## Concept review, 2026-11-11

No further design change or open product choice for this PR. The earlier review
page's resume and cap questions are answered by "1a 2b"; old-build replacement
can discard a draft, and background starts have no count cap. Resident-limit
suspension remains deferred, not a condition for shipping this change.

The model is one Session's history, one Process's execution and viewing, and a
close request from Task cleanup. Checkout retirement remains separate: an
unmerged checkout must survive even when its closed Task's conversation ends.
Returning, reopening or starting work can revoke completion cleanup; abandonment
still ends an unviewed terminal mid-turn. An unavailable store leaves execution
running, with failed viewing writes retried before closure. Output that races a
completion request cancels it without putting a write in the input path.

Next: gate and the private-Machine demo already listed below. Show return before
closure, return after closure with history but no editor, and store recovery with
input still flowing. The page at `scratch/concept-review.html` now shows this
model rather than the superseded proposals. No code changed in this review.

## Readings that cannot be recovered

2026-11-10 ~17:30: 343 lf processes; 113 `session observe-status` with gone or
replaced Desktop clients. PID 2046's stdin writer was inherited by 15 pane
processes. 24 `/tmp/lf-7501-*` owners/stand-ins were up to eight days old; their
fixture had no Drop. 78 owners were held by native input. Nine Desktop shells
included six on done Tasks whose retirement never reached closing because the
conversation's input hold blocked it. No existing process was signaled.

The initial audit's argument-based selector counted shell commands naming an
lf path. The corrected selector uses `ps comm`. QA's 2026-11-11 05:20 audit
counted 368, matching independent fresh readings before and after (368, 368).
`scratch/process_audit.py` generates `scratch/process-audit.html` read-only.

`scratch/qa.html` records the 05:13–05:55 walkthrough: open Task draft kept
40 minutes; done Task without checkout ended at completion +15:00; checkout
Task ended likewise, and its resumed conversation ended +15:00 after leaving.
The observed opening-input loss was #1882's path; that change is now in this
branch. A post-return checkout reason stayed stale until the next sweep;
ended providers retained open process rows that readers did not treat as live.

Two 110-test transport runs passed before the final close-request design and
left no new fixture processes. The latest sampled 797 descendant PIDs and 94
fixture-home paths; none remained. These runs do not cover this implementation.
A controlled 17-second provider-start delay reproduced the old upgrade startup
failure; both corrected upgrade cases passed with the delay. The temporary
wrapper was removed.

The previous publish gate stopped before suites at resource preflight (16.8
GiB free, 32 GiB reserve); log `/tmp/loo823-publish-gate.log`. No product failure
or process-cleanup result follows from that attempt.

2026-11-11 realign: remote main remained `49037487d`, already merged here.
PR #1877 still says "Native-input retention is unchanged"; its validation predates
the close-request design. That description needs replacement at publish.

## Focused checks, 2026-11-11

Inherited `LF_*` unset; stdin closed. The first implementation run preceded the
output/poll cancellation repair; the second covers it and the same six transport
cases. Both preceded compression. Existing compact-unwind linker warning only.

- `uv run cargo test -p loopflow --lib --test transport_tests --test integration -- transport::lifetime::tests:: transport::lifetime_tests:: ops::task::retirement::tests:: store::sqlite::task_retirement::tests:: desktop::shells::store::tests:: owner_close_upgrade_keeps_explicit_closes_and_rejudges_automatic_closes completed_task_ returning_task_viewer_cancels_shell_close_without_another_sweep silent_native_turns_survive_until_settlement native_background_watches_survive_until_settlement a_locked_store_does_not_delay_input_or_replay_it_on_return a_task_checkout_starts_its_conversation_in_the_background a_task_run_starts_its_conversation_and_abandon_stops_it documented_commands:: --test-threads=4`: 25 library, 6 integration and 6 transport cases passed. Background-start proofs include sixteen pre-existing conversations.
- `uv run cargo test -p loopflow --lib --test transport_tests -- transport::lifetime::tests:: transport::lifetime_tests:: conversation_cleanup_owns_grace_work_and_reversible_close_requests owner_close_upgrade_keeps_explicit_closes_and_rejudges_automatic_closes shell_close_waits_for_requested_work_and_new_work_revokes_readiness completed_task_closes_a_conversation_resumed_after_checkout_retirement completed_task_without_checkout_ends_a_conversation_with_typed_input returning_task_viewer_cancels_shell_close_without_another_sweep silent_native_turns_survive_until_settlement native_background_watches_survive_until_settlement a_locked_store_does_not_delay_input_or_replay_it_on_return --test-threads=4`: implementation tree passed 8 library cases (23.62s) and 6 transport cases (59.74s). Recovery revokes the stale request, then cleanup issues the next one; cancellation under a store lock stays off the byte path.

- `ps -axo pid=,comm=` after focused tests: 0 processes executing from this checkout’s `target/`; the full descendant/fixture census remains for demo. No provider or watcher was started outside fixtures.

- `cargo fmt --check`, `git diff --check`, `lf context`: passed; scratch is within both targets. Clippy remains for gate.

- Compression: `uv run cargo test -p loopflow --lib --test transport_tests -- transport::lifetime::tests:: transport::lifetime_tests:: ops::task::retirement::tests:: store::sqlite::task_retirement::tests:: desktop::shells::store::tests:: completed_task_ returning_task_viewer_cancels_shell_close_without_another_sweep ssh_shells:: --include-ignored --test-threads=4`: 25 library (86.23s) and 6 transport (115.37s) passed; includes all three real SSH cases and 20 reconnect/close cycles with process, PTY and socket checks. Final build has only the existing compact-unwind linker warning.
- `uv run python -` with isolated SQLite shell receipts omitting `close_requested`: audit reading passed; no schema fallback remains.

- Post-compression `ps -axo pid=,comm=`: 0 processes executing from this checkout’s `target/` (also 0 before the final run). Full suite census remains for demo.

## Remaining

- Gate: affected transport, cleanup/store, native-turn, rollover, config, CLI,
  parser, server schema, documented-command, context-launch and migration suites;
  Clippy and `cargo package --locked --allow-dirty -p loopflow`.
  Store trigger changes also reach `two_people`; shared fixture cleanup needs
  the `loopflow-test-support` regression. Include architecture, website docs,
  and headless Swift build/model/view checks for the pipe changes. The
  `scripts/test.py --list` reading on 2026-11-11 selects architecture, Rust,
  website and Swift. Reuse only identical tree/plans; earlier Swift results in
  PR #1877 are historical.
- Demo: process diff after affected suites and corrected audit recount. Walk the
  draft/return/close cases again with cleanup as the close-request producer.
- Publish: rewrite PR #1877 around scope-based lifetime, deletion of input
  retention, central close requests, no background cap, and fixture/pipe cleanup.

LOO-840 (`task_1ddbd3851a8e424e9ce887fa8f39353b`) removes `processes.native_input` and
`desktop_shells.close_requested` after the migration's staging window, exporting
released data first. LOO-829 removes the ignored config key after release.
