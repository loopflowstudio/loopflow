# Auto-merge ownership slice review

## Verdict

**Iterate. Keep the serial PR unpublished and the Task incomplete.** The
implemented auto-merge inheritance advances the full design through the shared
PR implementation. It does not close the remaining mutation-child, prerequisite
or configured acceptance obligations. No additional bounded defect was
established in this slice; no executable change was made during this review.

Starting head: `288b954bff8f6b926e1dfc26ac6df0aa18c559c1`.
Obtained the complete Task patch with `lf task diff LOO-285 --json`: 615,587
characters, `truncated: false`. Reviewed the directive, full design, current
slice, forbidden outcomes and Done when against the implementation and retained
reports. Compared the patch with the preceding compression input: only its
report changed after the implementation. Historical test receipts remain
historical evidence.

## Demonstration

Replayed `cargo test -p loopflow --test release_lock_tests
surviving_release_auto_merge_child_excludes_another_release -- --nocapture`.
All eight scenarios passed in 45.04 seconds. This uses the built CLI, disposable repositories,
real Git/bare origins, OS locks and processes. Eight cases cross initial arming
and re-arming, enable and disable, killed controller and failed launcher leaving
a descendant. The fixture observes a competing release defer, simulated remote
arm state, and restored target access after child exit. Preparation cases also
exercise blocked ordinary removal and retained checkout after controller cleanup.

GitHub and notes generation are simulated. The test proves those local process
boundaries; it cannot establish actual GitHub merge behavior, provider descendant
inheritance, caller preservation at every other interruption, launchd timing,
UI-host verification, public artifacts or either configured settlement. No
production mutation was used to manufacture proof.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Auto-merge child survival | Exclude a competing release until the child exits | Initial arming inherits target and checkout; waiting inherits target | Eight-case built-CLI demonstration | Pass locally |
| Replacement and failure | Revocation retains the same capabilities; cleanup preserves descendants | Enable forwards inheritance into disable; cleanup reacquires independently | Shared PR source and demonstration | Covered locally |
| Normal release and ordinary PR behavior | Preserve exact-head arming and ordinary callers | Same shared commands, explicit no-op inheritance for ordinary callers | Prior dropped-arm, advancing-main and ordinary-arm passes; unchanged source | Retained evidence |
| Complete mutation exclusion | Every side-effect child retains its owner's capabilities | Commit/push, other PR, notes/tools and source/cleanup children remain uncovered | `prepare_release_in_worktree`, `finalize_remote`, `finish_release_pr`, Git helpers | Gap |
| Stable due identity and same-minute replay | Preserve activation; one accepted settlement | Deterministic due keys and exact attempt fencing | Accounting source and prior cases | Retained local evidence |
| Delayed wake and frozen coverage | Three dues share one execution; later dues wait | Whole obligation document retains links/coverage; later waits recorded | `begin`, `finish_process`, prior joined proof | Retained local evidence |
| Crash during collapse and candidate retry | No partial ownership; resume original candidate | Atomic document replacement; selection retained through preflight | Writer source and prior regressions | Retained local evidence |
| Manual trigger and intervention | No fabricated due or unattended pair | Trigger history, interventions and collapsed provenance retained | History qualification, prior provenance cases | Local evidence; live race gap |
| Overlap continuation | One mutator, exact active attempt or retry | OS exclusion works; some continuation remains prose | Lock source, demonstration, prior overlap tests | Partial |
| Crash before/after tag or publication | Same candidate resumes; external effects retained | Existing recovery and publisher reconciliation | Prior same-tag and publisher proofs | Full interruption/live proof gap |
| Late result or zero process exit | No terminal regression or fabricated success | Atomic fenced `settle`; zero exit without proof becomes Unverified | Writer source and prior tests | Retained local evidence |
| Failed telemetry and ownership | Failures stay visible and have dated repair ownership | Current telemetry gates mutation; historical linkage/retry and accepted handoff absent | Latest-interval/two-day reader; retained 36-failure baseline | Gap |
| No-change and exact candidate | Empty captured range and complete applicable proof | Shared completion checks exact source and baseline | Prior joined and wrong-source regression; source | Retained local evidence |
| Draft/wrong hash/missing asset/smoke failure | Never qualify incomplete publication | Publisher requires public stages, hashes, UI proof and smoke | Prior Python/CLI counterexamples | Local evidence; live proof gap |
| Corrupt records | Preserve evidence and fail with path | Strict readers, atomic replacement | Accounting source and prior corruption case | Retained local evidence |
| Schedule/timezone/DST/Home changes | Preserve denominator and actionable continuation | Calendar/segments retained; closed unfinished attempts can remain Running | `close`, `observe`, closed `receipt_context` rejection | Continuation gap |
| Caller preservation | Exact branch/index/bytes across every exit | Earlier joined/hook cases preserve them; remaining child paths uncovered | Prior preservation proofs, current call graph | Partial |
| Independent scopes | Other targets/repos/checkouts progress | Separate target and checkout locks | Prior isolation proofs and owner source | Retained local evidence |
| Two adjacent automatic settlements | Two executions, at least one publication, all checks, no repair | No configured qualifying pair demonstrated | Acceptance ledger; fixtures ineligible | Gap |

## Source and negative architecture

Followed release preparation and waiting through `finish_arm_after_rebase`,
`prepare_pr`, `finalize_remote`, `enable_auto_merge` and `disable_auto_merge`.
Replacement revocation receives the same callback. The callback only configures
existing commands; it does not acquire ownership or create another PR writer.
Task-owned revocation paths still supply no release capability. The disposable
release checkout proof does not establish capability propagation through every
Task-owned compensation path; preserve that distinction in further shared work.

Inspected target-lock acquisition and child inheritance, checkout inheritance
and cleanup, CLI callers, accounting settlement, history qualification and their
fixture/documentation consumers. No duplicate success-proof wrappers, separate
`record_verification` writer or separate Python candidate/publish receipt classes
were found. `settle` remains the product-success writer. The repository flow is
one mechanical release operation; cron attribution remains separate from target
and checkout authority. Direct release worktree creation still explicitly
disables default-branch sync. This scoped inspection does not assert that every
indirect helper or external program obeys inheritance.

## Required next work

1. Carry existing target/checkout capabilities through shared commit/push and
   PR create/edit/ready, notes and lockfile children, then source creation/reset
   and cleanup. In particular, `commit_workflow` and `run_release_notes_stage`
   still receive neither capability; rebuild's `git reset --hard` uses ordinary
   `run_stdout`. Prove controller death and failed-launcher descendants for the
   concrete boundaries, including compensation paths where applicable.
2. Retain the prerequisite due identity for every original covered release due,
   its missing/failed evidence and linked current recovery. Implement the
   approved once-per-wake prerequisite retry. Record dated repair ownership;
   the missing `agent_turns` scorecard blocker and unaccepted Intelligence
   handoff remain. The baseline's 36 failures preserve the original 35.
3. Give closed unfinished obligations an actionable continuation/disposition
   while preserving old Home authority. `close` retains attempts but does not
   disposition them; `receipt_context` refuses the closed segment.
4. Complete interruption/isolation proof before supported install/sync and
   configured acceptance. Required UI-host/public exact-tag proof and two
   adjacent automatic executions remain mandatory. No new independent live
   publication blocker or sibling Task was identified here.

## Validation

The built-CLI demonstration passed all eight scenarios in 45.04 seconds
(23.12 seconds compilation). No other tests or static checks were rerun:
this review changed documentation only. Prior focused passes retain their stated
scope. No affected-suite gate, full CI, install/sync, cron trigger, production
publication, PM handoff, PR publication, landing or Task completion occurred.
