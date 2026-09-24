# Interrupted telemetry slice review

## Verdict

**Iterate. Keep the serial PR unpublished and the Task incomplete.** The slice
advances the full design: an interrupted prerequisite can recover through the
existing executor without rewriting its old receipt or displacing a surviving
child. The local demonstration passes. No additional bounded defect was
established; executable code is unchanged. Runtime bounds, historical obligation
coverage and configured acceptance still prevent full-design approval.

Starting head: `3875aa5beabb82010e369b8bea666d1e04262a51`.
Obtained the complete Task patch with `lf task diff LOO-285 --json`: 941,778
characters, `binary: false`, `truncated: false`. Reviewed the directive, full
design, current slice, forbidden outcomes and Done when. Inspected the latest
implementation delta and its execution, persistence, history and verification
owners. Only the compression report changed after implementation. Earlier
reports in the complete patch retain their historical scope; obtaining that
patch does not constitute a fresh exhaustive review of every previous slice.

## Demonstration

`cargo test -p loopflow --test scheduled_release_tests interrupted_telemetry_recovers_only_after_its_runner_and_child_exit -- --nocapture`
passed in **18.22 seconds**, after 28.54 seconds compilation.

The built cron CLI starts telemetry in a registered disposable Home. Three
release firings then exercise the real executor, authored mechanical Flow,
release operation, atomic settlement and history:

1. A live runner leaves release Deferred with no recovery reservation or product
   mutation.
2. After killing and reaping that exact fixture-owned controller, its surviving
   check retains the job lock. The one reserved recovery receipt records Failed
   overlap; release stays Deferred and the check has not been relaunched.
3. After the check finishes and independent lock acquisition proves exclusion
   has ended, the next firing runs one recovery and settles the simulated
   publication. Its telemetry verification names the new Recovery receipt.

The original Running receipt remains unchanged. All three attempts, frozen
catch-up coverage and caller HEAD/branch/raw index/staged/unstaged/untracked
bytes survive. The fixture rejects its synthetic population as a qualifying pair.

Git, bare origin, CLI, registry, processes and OS locks are real. Telemetry is
a shell verifier; GitHub and publisher evidence are simulated. Due dates are
seeded. This proves the local interruption boundary, not real launchd firings,
the configured doctor/scorecard Flow, hosted builds, UI-host verification or
public artifact smoke. No production process or schedule was changed for proof.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Live or unknown runner | Defer without takeover | Recorded PID/start is checked; absent identity remains Unknown | Current live-runner demonstration; legacy test and reader source, prior pass | Pass locally; unknown case not rerun |
| Dead controller, surviving child | Permit recovery only through existing exclusion | Recovery reserves once, then must acquire inherited job lock | Current three-firing demonstration | Pass locally |
| Recovery after child exit | New check supplies current proof; old result stays unknown | Same executor writes a distinct Recovery receipt | Current receipt, retry-count and verification-subject assertions | Pass locally |
| Bounded prerequisite execution | One retry and finite declared wait/failure | Retry count bounded; target still uses synchronous `command.status()` | Executor source | Runtime gap |
| Original due identity and repeated wake | Stable activation/key and one accepted settlement | Deterministic accounting and exact attempt fencing | Accounting writers and prior tests | Retained local evidence |
| Delayed wake and later dues | One frozen execution; subsequent dues wait | Atomic coverage/links and completion materialization | Current collapsed coverage; prior crossing-due test | Local evidence; actual wake timing unproved |
| Interrupted linking and candidate retry | Preserve owner, candidate and failures | Whole-document replacement; retained selection | Accounting source and prior regressions | Retained local evidence |
| Manual trigger/repair and timing | Preserve interventions and first attempt; no false unattended pair | Separate sources, trigger records, collapsed provenance and coverage readers | History/source and prior regressions | Local evidence; live trigger race gap |
| Overlap and mutation children | One mutator through child lifetime, exact continuation | Target/checkout/job scopes remain separate; some overlap continuation is generic prose | Current job proof; earlier child proofs and `record_overlap` | Partial |
| Crash around tag/publication | Resume exact candidate and retain public effects | Existing recovery, stage receipts and reconciliation | Prior same-tag/publisher cases; completion source | Remaining interruption/configured proof gap |
| Late result or zero exit | No terminal regression or invented publication | Fenced `settle`; `finish_process` cannot promote success | Writer source and prior preservation tests | Retained local evidence |
| Original telemetry and dated repairs | Retain every prerequisite/failure and owning disposition | Frozen references retain linked receipts outside window; previous telemetry segments absent | History and prerequisite source; retained 36-failure baseline | Historical coverage and production ownership gaps |
| No-change/resume verification | Exact source, empty range, complete current/baseline proof | Shared candidate checks and public verification | Producer source and prior joined/wrong-source cases | Local evidence; configured proof gap |
| Draft/wrong hash/missing asset/smoke failure | Reject incomplete success; retain external effects | Required publisher stages, UI and public read-back gates remain | Earlier publisher/joined counterexamples | Actual UI/public proof gap |
| Corruption, DST and reconfiguration | Explicit errors/uncertainty; preserve due population | Strict readers/shared calendar; closed release segments retained | Source and prior corruption/calendar tests | Closed continuation and prior telemetry segments remain gaps |
| Caller preservation and independent scopes | Keep bytes on all exits; unrelated work progresses | Current caller proof and distinct lock scopes | Demonstration and prior isolation/child cases | Pass at exercised boundaries; full interruption matrix open |
| Installed cutover and two adjacent settlements | Preserve cadence; two automatic executions, one publication, all checks, no repair | No installed cutover or qualifying configured pair demonstrated | Acceptance ledger; fixtures explicitly ineligible | Gap |

## Source and negative architecture

New receipts capture process birth separately from execution start. The shared
journal comparison retains its existing three-second observation tolerance.
Task evidence still requires its registered Exec receipt or matching terminal
event; cron uses its own retained receipt. Neither gains process-control authority
from the other. Probe errors and missing historical identity remain Unknown.

Release recovery checks identity only to decide whether to attempt the existing
executor. Placement checks and independent job-lock acquisition still precede
launch. The child inherits that existing descriptor. A failed acquisition writes
a physical failure and returns typed Deferred to release; it does not overwrite
the interrupted invocation. Reservation stays before launch and exact-attempt
fencing prevents another reservation within that wake. No new liveness store,
worker, timer, lock owner or target launcher was introduced.

The six-hour stale hint remains confined to presentation and trigger waiting;
it does not authorize recovery. The executor still waits synchronously, so
count-bounded recovery is not time-bounded execution. Arbitrary programs closing
inherited descriptors are outside the shell demonstration's guarantee.

Doctor still accepts Scheduled firing evidence independently of target outcome.
Recovery remains a different source. A missing natural receipt can therefore
fail continuity inside the actual telemetry Flow, which still runs doctor before
the scorecard. The simulated verifier does not settle this boundary.

`settle` remains the product-success writer. Physical completion can fill a
missing failure/unverified result but cannot promote process success or replace
accepted settlement. Searches found no restored success-proof wrappers,
`record_verification`, separate Python candidate/publish receipt classes or
`sync_main` in release and named-worktree source. The repository Flow remains
one mechanical release operation; historical schema-1 receipts retain decoding.
These scoped searches do not establish all remaining interruption obligations.

## Next implementation direction

1. Bound prerequisite waiting through the existing executor with a truthful
   continuation, preserving the reserved receipt and surviving-child exclusion.
   A deadline must not fabricate a child exit, kill a process to gain authority,
   or authorize a second check. Exercise the actual telemetry Flow with a missing
   natural firing; resolve its continuity boundary without relabeling Recovery
   as Scheduled or bypassing doctor.
2. Retain telemetry schedule/timezone/Home segments before replacement/removal
   and associate original dues with those segments. Give unfinished closed
   release obligations an actionable continuation/disposition without transferring
   old Home authority. Make remaining overlap continuations identify the active
   attempt or exact next due where required.
3. Record dated repair ownership through the supported disposition operation.
   All 36 retained telemetry failures, including the original 35, remain
   counterevidence. The missing `agent_turns` scorecard blocker and unaccepted
   Intelligence handoff remain. No new independent publication blocker or sibling
   Task was established in this review.
4. Finish remaining interruption proof before supported install/sync and real
   configured acceptance. Preserve 09:00/10:00 cadence, required UI/public proof,
   and two adjacent automatic executions with at least one publication and no
   manual repair. Collapsed dues cannot supply the second settlement.

Only review documentation changed. The single focused demonstration above is
the fresh behavioral result; prior ten-scenario, library and static checks were
not rerun. No full gate, hosted matrix, install/sync, cron trigger, production
publication, PM handoff, PR publication, landing or Task completion occurred.
