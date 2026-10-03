# Closed release repair disposition

## Change

Closed, unfinished release owners now remain actionable through the existing
history and disposition operations. This slice does not implement automatic
candidate continuation across obligation segments. The Task and serial PR remain
incomplete and unpublished.

Previously, closing a schedule retained its original opportunities and attempts,
but a pending or Running owner could not receive a repair disposition. Once its
due time fell outside the requested history window, even the unassigned repair
was absent from the summary. The new temporary-record regression reproduced that
omission before the repair.

`ObligationSegment::closed_unsettled` derives execution owners from retained
`closed_at`, coalescing and product outcomes. History exposes their ids in the
required `summary.closed_unsettled` field regardless of the display window.
Collapsed dues share their owner's blocker. Windowed due/product/execution counts
keep their time bounds; the new list explicitly reports older unresolved work.
A closed due is accounted for by its closure blocker without a synthetic attempt.
The DTO fixture and its round-trip reader include the field; no Swift consumer
of this history shape was found.

Text history names the original Home, closure time, candidate and exact
opportunity id, then prints the supported disposition command. It also shows the
latest recorded owner/reason. Attempt output is labeled as the last attempt,
not an assertion that the process is currently live.

`lf cron disposition` accepts closed unsettled owner ids through its existing
registered-Task validation and append-only disposition writer. The first
assignment is late after one day from closure; repeated assignments preserve the
first timestamp. The opportunity remains unresolved after assignment. Historical
failure dispositions continue to use their original failure times.

Validation materializes closed dues using the existing calendar owner. This
matters when a successor document survived but the predecessor rewrite did not:
closure and missed due identity are recoverable from the durable successor link.
Disposition records do not rewrite either document or invent missing execution
facts. No new opportunity store, closure writer, scheduler or lock was added.

## Preservation and review

- Removal, schedule replacement, timezone replacement and Home replacement retain
  the original candidate, attempts and collapsed dues. A later missed due has no
  attempt and receives its own repair obligation.
- All closed owners remain visible outside the reporting window. Late repeated
  assignments retain dated ownership without producing a product settlement or
  qualifying pair.
- The replacement Home cannot disposition an id that exists only in the original
  Home's records. The operation supplies neither cross-Home execution authority
  nor an automatic remote Task handoff.
- The existing exact-attempt settlement writer can still accept a final result
  from an operation already executing when its schedule closes. This test is a
  storage-boundary proof, not a new permission to launch or resume on an old Home.
  Receipt-context rejection for closed segments and process-control rules remain
  unchanged. Closure does not declare any process dead.
- A completed owner stops being a closure blocker; it cannot receive a closure
  disposition without separate retained failure evidence. Previously assigned
  ownership remains in history.

The simulated review kept closure derived from the existing durable fact rather
than adding a second lifecycle status or rewriting a Running attempt as failed.
It also added the interrupted predecessor-rewrite case: validating only stored
opportunity rows would reject a due that the read projection correctly exposes.
Automatic same-candidate continuation across segments and exact overlap
continuation remain required follow-up work, not behavior supplied by assignment.

## Validation

- `cargo test -p loopflow --lib closed_release_owners_retain_evidence_and_accept_dated_repair -- --nocapture`: reproduced the missing repair obligation before the fix (0.08s); final four-case proof passed (0.49s), including interrupted predecessor replacement.
- `cargo test -p loopflow --test scheduled_release_tests closed_release_history_records_repair_without_settling_or_transferring_work -- --nocapture`: passed, 16.34s. Reads text/JSON, records an actual local registered Task disposition, reports its late assignment, and verifies byte-identical opportunity storage.
- `cargo test -p loopflow --lib ops::cron::history::tests -- --nocapture`: four passed, 0.01s, including the updated DTO round trip and unchanged qualification/timing counterexamples.
- `cargo fmt --check`, `git diff --check`, and `cargo clippy --all-targets -- -D warnings`: passed.

Commands cleared `LF_CONTROL_HOME`, `LF_CONTROL_DB_PATH`, `LF_HOME` and
`LF_DB_PATH`; the CLI fixture also isolates ambient execution context. Its Home,
registered Task, local records and built CLI are real. Its dates and candidate
are seeded; no launchd firing, provider call, publication, installation or
production repair assignment supplies proof. The CLI proof preceded the added
interrupted-predecessor test case; production code was unchanged afterward.

Actual telemetry continuity, the retained scorecard schema blocker and unaccepted
Intelligence handoff, remaining interruption proof, UI-host/public exact-tag
verification and two adjacent configured automatic settlements remain open. The
original 35 failures and retained 36-failure observation are unchanged. No full
suite, gate, CI, install/sync, trigger, publish, land or Task completion is claimed.
