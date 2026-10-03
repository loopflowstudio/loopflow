# Telemetry wait slice review

## Verdict

**Iterate. Keep the serial PR unpublished and the Task incomplete.** Bounded
observation advances the complete design through the existing executor. No
additional bounded defect was established in this review; executable code is
unchanged. Historical obligation handling and configured acceptance remain open.

Starting head: `7458ab8d6cd920b1de1c391757a151d72ea638a9`, with the preceding
compression report uncommitted. Obtained the complete Task patch through
`lf task diff LOO-285 --json`: 974,921 characters, `binary: false`,
`truncated: false`. Reviewed the directive, complete design, current slice,
forbidden outcomes and Done when. Source inspection concentrated on the latest
executor change and its receipt, prerequisite, settlement and history consumers.
The earlier slice reports retain their historical scope; obtaining the full
patch does not establish fresh exhaustive review of every earlier mutation.

## Demonstration

The three focused cron tests passed in **0.64 seconds**. The deadline case uses
the production launcher and wait function with a 50ms window, real processes,
physical receipt files and OS locks. It observes Deferred while the check
continues, verifies exclusion after dropping the parent's lock, then releases
the check and observes a distinct successful recovery. The old Running receipt
stays unchanged even after the test observes the child's late successful exit.
The observation-error case reaps its own child outside `Child` and verifies a
command failure without a fabricated exit. The third case checks job exclusion.

The built-CLI interruption demonstration exercises three release firings through
the installed executor, authored Flow, release operation, settlement and history:
a live runner defers without a retry; confirmed controller death permits a
reserved retry that the surviving child's lock excludes; after child exit, one
fresh recovery supplies current verification. It checks original receipt
preservation, frozen coverage, verification subject and exact caller
HEAD/branch/index/staged/unstaged/untracked bytes. It passed in **19.12 seconds**.

These are disposable Home/repository proofs. Git, CLI dispatch, registry,
processes and locks are real; telemetry is a shell verifier and GitHub/publication
proof is simulated. Due dates are seeded. The short deadline does not exercise
a one-hour CLI timeout or persist a release attempt at that timeout. The CLI
demonstration covers interruption and recovery, not deadline expiry. Neither
executes the configured doctor/scorecard Flow, UI-host checks, public artifact
smoke or a qualifying automatic settlement. No production mutation supplied proof.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Finite prerequisite observation | One-hour wait, truthful continuation | Recovery alone selects a deadline; timeout names receipt/log; release adds next due | Focused deadline proof and caller source | Pass locally; joined deadline expiry unexercised |
| Unknown exit | No fabricated terminal receipt or success | Timeout/observation errors return before terminal writes | Deadline and observation-error cases | Pass locally |
| Surviving child | Preserve exclusion without killing or granting takeover | Existing job descriptor reaches child; deadline drops only parent handles | Focused real-process proof; CLI interruption demonstration | Local proof |
| Fresh recovery | New check proves current health; old receipt remains unknown | Existing executor reserves one new receipt after exclusion ends | Focused recovery and CLI receipt/subject assertions | Local proof |
| Ordinary target execution | Preserve blocking waits and observed terminal results | Non-Recovery sources use `Child::wait`; same terminal writer | Source and prior ordinary success/failure test | Retained evidence |
| Stable dues and repeated firing | Preserve activation, deterministic keys and accepted settlement | Existing accounting and exact attempt fencing | Writers and prior cases | Retained local evidence |
| Delayed wake and later dues | Freeze coverage; one execution/result; later dues wait | Whole-document collapse and completion materialization | CLI coverage assertions; prior crossing-due case | Local evidence; real wake timing gap |
| Candidate continuity and crash recovery | Preserve owner/tag and public effects across retries | Existing selection, stage receipts and publisher reconciliation | Source and prior candidate/publication cases | Remaining interruption/configured proof gap |
| Late evidence and zero process exit | No terminal regression or invented publication | Fenced atomic settlement; wrapper fills only missing process outcome | `settle`, `finish_process`, prior cases | Retained local evidence |
| Manual provenance and timing | Preserve intervention/first attempt; reject false automatic pair | Separate sources, trigger records and historical coverage reader | History source and prior regressions | Live trigger race unproved |
| Release overlap | One mutator with exact continuation | Target exclusion retained; target-lock continuation remains generic prose | Lock source and prior survival proofs | Exact continuation gap |
| Original verification and repair ownership | Retain each original prerequisite, failure and dated owner | Frozen references and linked history; previous telemetry segments absent | Prerequisite/history source; retained 36-failure baseline | Historical coverage and production ownership gaps |
| No-change/resumed checks | Exact subject, empty range and complete applicable proof | Shared candidate completion and baseline/current checks remain | Source and prior joined/wrong-source cases | Retained local evidence |
| Incomplete publication | Reject draft/wrong hash/absent stage/smoke failure; retain effects | Publisher requires stages, UI and exact public proof | Producer source and prior counterexamples | Actual UI/public proof gap |
| Corruption/calendar/reconfiguration | Preserve denominator and actionable old work | Strict storage/shared calendar; closed segments retained but recovery rejected | `observe`, `close`, `receipt_context`, prior cases | Closed continuation and prior telemetry segments gap |
| Caller and independent scopes | Preserve bytes and unrelated progress at every exit | Current caller assertions and separate job/target/checkout locks | CLI proof and prior isolation cases | Complete interruption matrix open |
| Configured acceptance | Supported cutover, two adjacent automatic executions, one publication, all checks, no repair | No installed cutover or qualifying pair demonstrated | Acceptance ledger; synthetic cases ineligible | Gap |

## Source and authority

Spawn errors still write terminal failure. Once a child exists, only an observed
exit reaches the terminal writer. Deadline expiry returns typed Deferred; an OS
observation error returns failure. The release caller records that distinction
through existing `settle`; physical wrapper failure cannot overwrite it.
Reservation precedes launch and cannot be replaced within the wake. History
retains linked prerequisite receipts outside its display window.

The deadline never signals a child. The existing `kill(pid, 0)` probe belongs to
stale display/trigger waiting, not timeout recovery authority. Exact runner
identity permits attempting recovery; the inherited job lock still excludes a
surviving child. No detached observer later settles the old receipt. Arbitrary
programs that close inherited descriptors remain outside the shell proof.

Doctor still accepts only Scheduled receipts, independently of target success.
Recovery cannot fill a missing natural-firing interval inside the actual
`doctor` then `__telemetry-scorecard` Flow. Successful shell recovery therefore
does not prove configured verification can pass. No exemption was introduced.

The repository release Flow remains one mechanical operation; `settle` is the
typed product-success writer. Inspected launch paths contain one cron target
launcher and no new background observer or receipt writer. Searches found no
restored success-proof wrappers, `record_verification`, separate Python
candidate/publish receipt classes or `sync_main` in release/named-worktree source.
Schema-1 physical receipt decoding remains required historical compatibility.

## Next implementation direction

1. Exercise the actual telemetry Flow's missing-natural-receipt boundary and
   retain a truthful verification blocker without relabeling Recovery as
   Scheduled or bypassing doctor. Preserve the observed scorecard query against
   missing `agent_turns`; its Intelligence handoff remains unaccepted.
2. Retain telemetry schedule/timezone/Home segments before replacement/removal
   and associate original dues with their proper segment. Give closed unfinished
   release obligations a supported continuation/disposition without transferring
   old Home authority. Make overlap continuation identify the active attempt or
   exact next due where the design requires it.
3. Record dated repair ownership through the supported operation. All 36 retained
   telemetry failures, including the original 35, remain counterevidence; a late
   disposition must stay visibly late. No sibling Task or independent new live
   publication blocker was established here.
4. Complete remaining interruption and installed-path proof before claiming the
   two adjacent automatic settlements. Preserve 09:00/10:00 cadence, required
   UI/public proof, at least one publication and no manual repair. Collapsed
   dues cannot supply the second success.

## Validation

- `cargo test -p loopflow --lib ops::cron::tests::telemetry_ -- --nocapture`:
  three passed, 0.64s execution, 30.84s compilation including package-cache waits.
- `cargo test -p loopflow --test scheduled_release_tests interrupted_telemetry_recovers_only_after_its_runner_and_child_exit -- --nocapture`:
  one test / three firing boundaries passed, 19.12s execution, 57.67s compilation
  including package-cache and build-directory waits.
- `git diff --check`: passed.

Only review documentation changed. No other behavioral suites or compiler/lint
checks were rerun. No full gate, hosted matrix, installation/sync, cron trigger,
production publication, PM handoff, PR publication, landing or Task completion
occurred. Prior validation retains its recorded scope.
