# Task compensation slice review

## Verdict

**Iterate. Keep the serial PR unpublished and the Task incomplete.** Task
revocation now retains the caller's release capabilities through the shared
command path. The local demonstration passes and advances the full design.
No additional bounded defect was established in this review; no executable code
changed. Historical prerequisite recovery, closed-obligation continuation and
configured acceptance remain implementation and evidence gaps.

Starting head: `f490993db4b48ec5e2618910b5640bfefda2fb73`.
Obtained the complete Task patch through `lf task diff LOO-285 --json`: 854,114
characters, `binary: false`, `truncated: false`. Reviewed the directive, complete
design, current slice, forbidden outcomes and Done when against the current
owners. Source inspection concentrated on the latest Task compensation change
and its callers, with accounting and verification boundaries checked separately.
The only change since implementation was the compression report. Historical
reports and receipts in the patch are not fresh validation.

## Demonstration

`cargo test -p loopflow --test release_lock_tests surviving_task_revocation_retains_release_ownership_and_settlement_intent -- --nocapture`
passed both interruption cases and replay in **12.00 seconds**, after 24.63
seconds compilation. The built release CLI reaches shared commit's Task push
fence in a disposable registered checkout. The fixture kills its exact controller
or lets a failed launcher leave a paused revocation descendant.

While revocation is paused, a competing release defers, ordinary checkout removal
fails, the original durable merge request remains and the bare origin has no new
release branch. After the child records remote revocation, the interrupted
controller still has not cleared local intent. Ordinary commit/push replay reads
the disabled remote state, clears intent and pushes the prepared checkout's
exact HEAD. Caller HEAD, branch, raw index and edited/untracked bytes survive.

Git, bare origin, CLI, Task store, processes and OS locks are real. GitHub and
notes generation are simulated. This proves the reachable pre-push revocation
boundary and replay, not hosted GitHub behavior, every compensation interruption,
arbitrary programs retaining descriptors, installed scheduling, UI-host proof or
either configured automatic settlement. The fixture's Task registration is not
a production placement change. No production mutation supplied evidence.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Task revocation survives controller exit | Preserve target/checkout ownership until child exit | Existing capabilities reach shared remote revocation | Two-case built-CLI demonstration | Pass locally |
| Revocation before push, failure retention and replay | Keep durable intent and old remote head until revocation returns | Clearing follows successful disable; replay observes prior remote success | Durable request and bare-origin assertions | Pass locally |
| Replacement, stale-head and finalization compensation | Same inheritance, exact-head authority and error ordering | All three Task revocation owners receive their caller's callback | Source; prior ordinary revocation/replacement/failure tests | Retained evidence; separate kill points unproved |
| Ordinary Task callers | No implicit release authority | Observation, restart, resume, rebase and ordinary publication explicitly supply no capability | Call-site inspection | Pass by source |
| Stable due and repeated firing | Preserve activation and one accepted settlement | Deterministic keys and exact attempt fencing | Accounting source and prior tests | Retained local evidence |
| Delayed wake and later dues | One execution for frozen coverage; later dues wait | Atomic obligation document, saved coverage, later due wait records | `begin`, `finish_process`, prior joined proof | Retained local evidence |
| Interrupted collapse and candidate retry | Retain owner, selection and failures | Atomic document; selection survives intervening preflight failure | Writers and prior regression | Retained local evidence |
| Trigger, intervention and timing | Preserve manual provenance and first attempt through collapse | Qualification reads owner/collapsed provenance; timing uses frozen coverage | History source and prior regressions | Local evidence; live trigger race unproved |
| Overlap | One mutation owner with exact continuation | Exclusion holds at demonstrated boundaries; target-lock continuation still generic prose | Demonstration, `ReleaseLock::acquire`, `record_overlap` | Partial |
| Crash around tag/publication | Resume exact candidate and retain external effects | Existing recovery and publisher reconciliation | Prior same-tag/publication cases; current entry path | Full interruption/configured proof gap |
| Late result or process zero exit | No terminal regression or fabricated product success | Fenced atomic settlement; wrapper only fills Running outcome | `settle`, `finish_process`, prior cases | Retained local evidence |
| Failed telemetry and repair ownership | Retain each original prerequisite, current recovery and dated owner | Current telemetry gates mutation; historical links and bounded retry absent | `verify_scheduled_telemetry`; retained 36-failure baseline | Gap |
| No-change and resume checks | Exact source, empty range and complete applicable verification | Shared completion and verified public baseline remain required | Producer source and prior joined/wrong-source cases | Retained local evidence |
| Incomplete public result | Reject wrong hash, absent asset, draft or failed smoke; retain effects | Publisher requires stages, UI proof and public read-back | Publisher/settlement consumers; prior counterexamples | Local evidence; actual public/UI gap |
| Corrupt records | Fail explicitly and preserve evidence | Strict readers and atomic replacement | Writer source and prior corruption case | Retained local evidence |
| Schedule/timezone/DST/Home changes | Preserve denominator and actionable old work | Closed segments retained; receipt context rejects closed recovery | `observe`, `close`, `receipt_context`, prior calendar cases | Continuation gap |
| Caller preservation and independent scopes | Preserve caller bytes on all exits; unrelated work progresses | Current caller proof, independent target/checkout locks | Demonstration plus prior isolation/preservation receipts | Partial; complete interruption matrix open |
| Supported cutover and history | Preserve 09:00/10:00 cadence and unknown historical denominator | Local history exists; installed cutover not demonstrated | Design and retained Home observations | Gap |
| Two adjacent automatic settlements | Distinct executions, at least one publication, all checks, no manual repair | No configured qualifying pair demonstrated | Acceptance ledger; synthetic fixtures ineligible | Gap |

## Source and negative architecture

Followed release preparation into shared commit/push and `finish_arm_after_rebase`.
Both callbacks carry the already-held target lock and checkout lease. Task
resolution still owns whether the checkout is managed; either descriptor alone
cannot grant Task settlement authority. Clearing, Auto-to-User replacement and
stale-head invalidation all forward inheritance to the existing
`disable_auto_merge`. Remote failure returns before durable intent is cleared
or replaced. Failed finalization forwards the same callback through compensation
and retains both errors if revocation fails.

The Task PR mutation guard retains its existing controller lifetime. This cut
does not claim to extend every Task lock into descendants or serialize arbitrary
independent Task operations after controller death. The demonstrated guarantee
is the release target and exact checkout lifetime. Other compensation routes
have ordinary behavioral evidence and source propagation, not independently
interrupted-child demonstrations.

Checked explicit cron receipt/descriptor context through Flow dispatch, the
accounting settlement and history readers, history fixture and publisher proof
consumers. The repository Flow remains one `op: release run patch`; `settle`
remains the typed product-success writer. Process completion cannot promote zero
exit or replace accepted settlement. Searches of the inspected production paths
found no restored duplicate success-proof wrappers, `record_verification`,
separate Python candidate/publish receipt classes or `sync_main` in release and
named-worktree creation. There is no second Task revocation implementation or
new executor, store, liveness ledger or lock owner in this cut. These searches
do not prove arbitrary external descendants retain descriptors.

## Next implementation direction

1. Retain the telemetry prerequisite for every original covered release due,
   including missing/failed historical evidence. Implement the accepted bounded
   once-per-prerequisite-per-wake retry through the existing cron target executor,
   linking current recovery to the frozen coverage and original failures.
   `verify_scheduled_telemetry` still selects the current interval from two days
   of receipts; it does not retain those original associations or launch a retry.
   Correct the earlier no-retry implementation note in `questions.md` when this
   approved behavior is implemented; it does not override the accepted design.
2. Record dated repair ownership without erasing late assignment or implying an
   accepted remote handoff. Retain all 36 observed telemetry failures, including
   the original 35, and the missing `agent_turns` scorecard blocker. Intelligence's
   handoff remains unaccepted; no independent publication blocker or sibling Task
   was established here.
3. Give unfinished closed obligations a supported continuation or disposition
   while preserving old Home authority. Closing currently retains attempts,
   while `receipt_context` rejects the segment; old Running rows can remain
   unresolved. Replace generic overlap continuation with exact active-attempt or
   next-due evidence where required by the design.
4. Finish the remaining interruption/isolation proof, including explicit limits
   on the unexercised compensation/candidate/reconciliation boundaries, before
   supported installation/sync. Preserve the required UI-host/public exact-tag
   proof and two adjacent automatic executions, at least one publishing without
   manual repair. Collapsed misses cannot supply the second settlement.

Only review documentation changed. No other behavioral tests or static checks
were rerun; previous focused passes retain their recorded scope. No affected-suite
gate, full CI, install/sync, cron trigger, production release, PM handoff, PR
publication, landing or Task completion occurred.
