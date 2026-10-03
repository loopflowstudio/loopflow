# Closed opportunity disposition slice review

## Verdict

**Iterate. Keep the serial PR unpublished and the Task incomplete.** Closed
owners now have a useful repair surface without another execution owner. Review
reproduced and repaired a contradictory continuation in text history. Automatic
continuation across closed segments and configured acceptance remain open; repair
assignment does not satisfy those obligations.

Starting HEAD: `7458ab8d6cd920b1de1c391757a151d72ea638a9`, with the telemetry
segment and closed-disposition work uncommitted. Captured the complete tracked
and untracked patch from base `88cf10641b0e88fc4bfcf504ef62bed4aa057539` in
`/tmp/loo285-review-closed.patch`: 1,055,104 bytes, 20,552 lines. The earlier Task
diff API exceeded its output limit, so this capture uses Git and no-index diffs.
Reviewed the directive, full design, current slice, Done when and forbidden
outcomes. Fresh source inspection concentrates on closure, disposition, history
and their execution/verification boundaries. Patch availability and historical
reports do not establish a new exhaustive review of every earlier mutation path.

## Demonstration and repair

The built-CLI disposition/history demonstration passed in **11.41 seconds**
after 25.44 seconds compilation. A disposable registered Home contains a closed
Running owner outside the one-day display window. JSON and text expose its
original Home, candidate and repair command. The actual disposition command
records a registered Task, retains late assignment, leaves the owner unresolved
and preserves byte-identical opportunity storage. No product settlement or
qualifying pair appears.

Source inspection found that a closed owner with a retained `OpportunityWait`
still used the ordinary detail rendering: `deferred: ...; next firing ...`.
The new closure block above it said there was no future firing. The expanded
CLI regression exposes both rows by widening the display window, retaining a
wait recorded before closure whose expected retry was after closure. It failed
in **12.91 seconds** with both contradictory statements in the same output.

The detail renderer now labels a closed segment's saved wait as `last wait`
with its `previously expected firing`. Open segments keep their current retry
wording. This retains historical timing without promising execution from a
removed schedule. No wait record, attempt, disposition, DTO or execution policy
changes. The regression also checks unchanged stored bytes and the original
wait in JSON. Final validation is recorded below.

Dates, candidate and waits are seeded. CLI dispatch, local registry and file
storage are real. This is local reporting/assignment proof, not a real launchd
firing, publication, installed cutover, UI-host run or automatic settlement.
No production mutation supplied evidence.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Closed owners outside the window | Retain actionable original-Home work | Shared derived predicate feeds summary and text | Built-CLI demonstration | Pass locally |
| Dated/repeated repair assignment | Preserve ownership history and lateness without settlement | Existing append-only writer; earliest assignment determines lateness | CLI demonstration; prior repeated-assignment library case | Local proof; production ownership remains open |
| Removal, schedule/timezone/Home replacement | Retain attempts, candidate, collapsed dues and missed wakes | Retained segments and successor closure; disposition uses shared materialization | Source; prior four-case regression including interrupted predecessor write | Retained local proof |
| Closed saved wait | Do not promise a firing from a closed schedule | Historical wait wording, original JSON unchanged | New CLI counterexample | Reproduced and repaired |
| Settlement after closure | Existing exact writer can finish; closure cannot declare death | Settlement fence unchanged; new receipt entry still rejects closure | `settle`, `receipt_context`; prior storage-boundary case | Local storage proof; automatic continuation gap |
| Stable dues, repeated firing and delayed wake | One due key, frozen coverage, one execution/result; later dues wait | Deterministic keys and atomic document; completion materializes later waits | `begin`, `finish_process`; prior joined/accounting cases | Retained local proof |
| Interrupted collapse, candidate retry and late evidence | Preserve original selection, reject superseded/conflicting results | Atomic links/coverage and exact attempt settlement | Writers; prior preservation regressions | Retained proof; full interruption matrix open |
| Zero exit versus product result | No invented publication | Wrapper fills missing process failure/Unverified only | `finish_process`, typed settlement producer | Pass by source and prior proof |
| Manual trigger/repair and timing | Preserve provenance and original first attempt | Sources/interventions and frozen coverage remain qualification inputs | History source; prior counterexamples | Live trigger race unproved |
| Overlap and surviving children | One mutation owner, exact continuation | Existing target/job/checkout locks and inheritance | Entry/lock source; prior survival demonstrations | Exact overlap continuation and remaining interruption proof open |
| Historical telemetry and current recovery | Retain original failures and one bounded retry | Segment references, frozen reservation, bounded executor | Current segment/recovery source; prior joined proof | Actual doctor/scorecard path and repair handoff open |
| No-change and resume checks | Exact source, empty range and complete applicable evidence | Shared completion and public baseline checks | Release producer; prior joined/exact-source tests | Retained local proof; configured proof open |
| Public publication and partial effects | Reject drafts, wrong hashes, missing stages or failed smoke; retain effects | Existing publisher stage/read-back and settlement gates | Prior Python/CLI counterexamples | Actual UI/public exact-tag proof open |
| Corruption, DST and historical uncertainty | Fail explicitly and retain denominator | Strict reads, atomic replacement and shared calendar | Current readers; prior corruption/calendar cases | Retained local proof |
| Caller preservation and independent scopes | Preserve branch/index/bytes and unrelated progress | Existing source selection and distinct capabilities | Prior joined/child/isolation proofs | Complete interruption matrix open |
| Two adjacent configured settlements | Two executions, one publication, all checks, no manual repair | No qualifying configured pair demonstrated | Acceptance ledger | Gap |

## Ownership and next direction

`closed_unsettled` reads closure, coalescing and product outcome; it persists no
second status. Disposition appends ownership under the existing accounting lock
and does not alter attempts or acquire execution authority. A missed due recovered
from a successor link uses the same calendar function for history and command
validation. The repair changes only how a retained wait is presented.

The repository release Flow remains one mechanical operation. Explicit cron
receipt/descriptor validation precedes scheduled attribution. `settle` remains
the typed product writer, while `finish_process` cannot promote a successful
wrapper exit. Scoped searches found no restored `ReleaseObligation` alias,
duplicate success-proof wrappers, separate verification writer, separate Python
candidate/publish receipt classes, or `sync_main` in release/named-worktree source.
Historical schema-1 receipts retain their reader. No scheduler, process liveness
store, ownership transfer or publication bypass was introduced.

Continue within this Task:

1. Implement supported same-candidate continuation across closed segments while
   preserving original due identity and Home authority. `begin` still selects
   within one segment and `receipt_context` rejects closed entry. Assignment
   supplies an actionable repair owner but cannot perform that continuation.
   Replace target-lock overlap's generic prose with exact active-attempt or
   next-due evidence.
2. Exercise the actual telemetry Flow's missing-natural-receipt boundary and
   record dated repair ownership through the supported path. Doctor still
   requires Scheduled evidence; Recovery cannot relabel it. The scorecard still
   queries `agent_turns`/`agent_invocations`; the retained missing-table blocker
   and unaccepted Intelligence handoff remain. All 36 retained failures,
   including the original 35, remain counterevidence.
3. Complete remaining interruption/isolation proof and supported install/sync,
   preserving 09:00 telemetry and 10:00 release. Required UI-host/public proof
   and two adjacent automatic executions, at least one publishing without
   manual repair, remain mandatory. Collapsed dues cannot supply a second win.

No independent new live publication blocker or sibling Task was established.

## Validation

- `cargo test -p loopflow --test scheduled_release_tests closed_release_history_records_repair_without_settling_or_transferring_work -- --nocapture`: initial demonstration passed (11.41s); expanded saved-wait case failed before repair (12.91s), then passed afterward (15.81s execution, 30.31s compilation).
- `cargo fmt --check` and `git diff --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed, 16.62s.

The CLI test cleared `LF_CONTROL_HOME`, `LF_CONTROL_DB_PATH`, `LF_HOME` and
`LF_DB_PATH`; its fixture separately isolates ambient execution context. The
final proof covers the changed rendering and preserves the previous disposition
assertions. No other behavioral suite was rerun; earlier checks retain their
recorded scope. Only documentation changed after these checks. No full gate,
hosted matrix, production installation/sync, trigger, publication, PM handoff,
PR publication, landing or Task completion is claimed.
