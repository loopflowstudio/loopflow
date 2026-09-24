# Telemetry recovery slice review

## Verdict

**Iterate. Keep the serial PR unpublished and the Task incomplete.** Frozen
prerequisite associations and one reserved automatic retry advance the complete
design through the existing cron executor. They do not close historical
obligation coverage, bounded execution/recovery or configured acceptance. No
additional bounded defect was established in this review; executable code is
unchanged.

Starting head: `e10a837993e6597f8ed50425adc67de9bb3df67e`. Obtained the complete
Task patch with `lf task diff LOO-285 --json`: 904,487 characters,
`binary: false`, `truncated: false`. Reviewed the directive, full design,
current slice, forbidden outcomes and Done when. Source inspection concentrated
on the telemetry implementation and its accounting, execution, history and
verification consumers. The preceding compression changed only its report;
earlier validation remains historical evidence.

## Demonstration

`cargo test -p loopflow --test scheduled_release_tests -- --nocapture` passed
all eight scenarios in **92.40 seconds**, after 1m37s compilation.

The fixture reaches the installed cron executor, built CLI, authored mechanical
Flow, release operation, durable settlement and history in disposable Homes.
Eight scenarios cover publication, no-change, persistent telemetry failure,
successful recovery, missing telemetry, older prerequisite evidence, public
smoke failure and missing public-stage proof. It checks frozen catch-up coverage,
one recovery per wake, unchanged old failures, conservative same-second ordering,
older linked failures outside the requested display window and exact caller
HEAD/branch/index/staged/unstaged/untracked preservation.

Git, bare origins, CLI dispatch, Home storage and OS locks are real. Telemetry is
a shell verifier; GitHub and publisher proof are simulated. Original due dates
and historical observations are seeded. This does not execute the configured
`doctor`/scorecard Flow, real launchd firings, hosted builds, UI-host checks or
public artifact smoke. It supplies neither automatic settlement required by the
KR. No production mutation was used to manufacture evidence.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Original prerequisite association | Retain each frozen due's evidence or uncertainty | Attempt snapshot maps covered keys to observed intervals and receipt ids | Joined historical/unknown cases; `record_telemetry` | Local proof; previous segments remain a gap |
| One automatic prerequisite retry | Reserve before launch; never retry per collapsed day | Shared executor invokes fenced reservation before launching | Joined recovered/missing/failed cases; reservation writer and prior focused test | Local proof |
| Recovery cannot erase failure | Preserve old receipts and ownership age | History includes referenced receipts outside its window | Joined older-failure and recovery assertions; unchanged disposition reader | Local proof; production repair ownership gap |
| Running/overlapping prerequisite | Bounded wait with truthful continuation, no duplicate launch | Running defers; occupied job returns typed Deferred; target execution remains synchronous | Executor/source; prior job-lock test | Runtime/controller recovery gap |
| Same-second ambiguity | UUID ordering cannot manufacture a repaired check | Running/Failed precede acceptance of a tied pass | Joined tied-pass case | Local proof |
| Stable dues and repeated firing | Preserve activation, due identity and accepted settlement | Deterministic keys and atomic fenced writers | `observe`, `begin`, `settle`; prior focused receipts | Retained local evidence |
| Delayed wake and later dues | One frozen execution; later dues wait | Saved coverage and atomic collapse; completion materializes later waits | Joined coverage assertions; `finish_process` and prior delayed-wake test | Local proof; real wake timing unproved |
| Candidate retry and interrupted collapse | Retain exact owner, candidate and prior failures | Whole-document replacement; selection survives failed attempts | Accounting writers and earlier regressions | Retained local evidence |
| Manual trigger/repair and timing | Preserve provenance; reject false unattended pairs | Separate Recovery source; existing trigger/intervention and coverage readers | Source, fixture and prior history regressions | Local evidence; live trigger race unproved |
| Release overlap/child survival | One mutation owner through surviving children | Existing target/checkout inheritance; some continuation remains generic prose | Reachable release entry; prior child-survival reports | Remaining interruption/continuation gap |
| Late settlement and zero process exit | No terminal regression or invented publication | `settle` fences exact attempt; wrapper cannot overwrite accepted success | Writer inspection and prior preservation cases | Retained local evidence |
| No-change/resume verification | Exact source, empty range and complete baseline/current proof | Shared candidate completion and public proof remain required | Joined no-change and product-failure cases; prior exact-source regression | Local proof; live evidence gap |
| Incomplete public result | Reject missing stage/hash/smoke; preserve external effects | Existing publisher reconciliation and settlement gates | Joined smoke/missing-stage cases; prior publisher counterexamples | Local proof; actual public/UI gap |
| Corruption and calendar changes | Preserve evidence, reject corruption, retain calendar semantics | Strict readers, atomic replacement and shared calendar | Source and prior corruption/DST cases | Retained local evidence |
| Schedule/removal/timezone/Home history | Preserve denominator and actionable unfinished work | Release segments retained; prior telemetry segments absent; closed recovery rejected | `add_cron`, `remove_cron`, `observe`, `close`, `receipt_context` | Gap |
| Caller preservation and independent scopes | Preserve bytes on all exits; unrelated work progresses | Joined outcomes preserve caller; existing distinct target/checkout scopes | Current demonstration and prior isolation/interruption reports | Local proof at exercised boundaries |
| Failure ownership within one day | Dated actionable repair disposition | Command/read model exist; no production ownership or accepted handoff recorded | Retained baseline and disposition source | Gap |
| Two adjacent configured settlements | Two executions, at least one publication, all checks, no manual repair | No configured qualifying pair demonstrated | Acceptance ledger; synthetic rows are ineligible | Gap |

## Source and authority

`verify_scheduled_telemetry` reads retained receipts rather than the former
two-day list. It freezes original associations and current observations before
recovery. `record_telemetry` requires the exact current Running attempt, compares
coverage, accepts identical repetition and rejects reservation replacement.
The later receipt read observes the reserved result; it does not select a new
retry. Original observations remain on failed attempts and later wakes append
their own evidence.

Both ordinary firing and recovery use `run_cron_recorded` and the same
`spawn_cron_target`. Reservation failure writes a failed physical receipt before
returning; placement checks remain before launch. The existing telemetry job
descriptor reaches its child. Recovery does not receive release-attribution CLI
arguments and cannot become a natural scheduled receipt. The release operation
retains its separate target lock before prerequisite work and selection.

The count limit is not a deadline: `command.status()` still waits for the target.
Also, a retained Running prerequisite causes deferral before attempting recovery.
Source therefore does not establish eventual recovery after its controller dies,
even if its OS lock becomes free. Silence or elapsed age must not be used to
invent process authority when addressing this gap.

The real telemetry Flow remains `doctor` followed by `__telemetry-scorecard`.
Doctor's continuity reader accepts Scheduled receipts only, independently of
target success. A missing natural firing can still make an automatic recovery
fail that prerequisite. The passing shell verifier cannot settle this configured
behavior; no continuity exemption is introduced or recommended as a shortcut.

`settle` remains the typed product-success writer; `finish_process` fills missing
process outcomes without promoting a zero exit. The repository release Flow is
one mechanical op. Searches found no restored duplicate success-proof wrappers,
`record_verification`, separate Python candidate/publish receipt classes or
`sync_main` in release and named-worktree creation. Historical schema-1 receipts
retain their reader. These scoped searches do not replace the outstanding
interruption and configured-path proof.

## Next implementation direction

1. Bound prerequisite waiting and complete controller-death recovery through the
   existing executor and exact ownership evidence. Preserve the one reserved
   receipt per wake, surviving-child exclusion and truthful missing-result state.
   Exercise the actual telemetry Flow's continuity boundary with missing natural
   evidence; reconcile that behavior without relabeling Recovery as Scheduled or
   bypassing required verification.
2. Retain telemetry obligation segments before replacement/removal, then bind
   original dues to the correct schedule/timezone/Home segment. Keep unknown
   pre-observation coverage explicit. Give closed unfinished release obligations
   a supported continuation/disposition without transferring old Home authority.
3. Record dated repair ownership and the actual dependency disposition. The
   retained 36 telemetry failures include the original 35; the missing
   `agent_turns` scorecard table remains the observed blocker. Intelligence's
   handoff is still unaccepted. No independent new publication blocker or sibling
   Task was established here.
4. Finish remaining interruption/isolation proof before supported installation
   and cron sync. Preserve 09:00/10:00 cadence, required UI-host/public exact-tag
   proof and two adjacent automatic executions with at least one publication and
   no manual repair. Collapsed dues cannot supply a second settlement.

## Validation

The joined CLI demonstration passed all eight scenarios (one Rust test), with
no executable changes during review. `git diff --check` passed. No other tests
or compiler/lint checks were rerun; prior focused receipts keep their recorded
scope. No affected-suite
gate, full CI, installation/sync, cron trigger, production publication, PM handoff,
PR publication, landing or Task completion occurred.
