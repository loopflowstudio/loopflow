# PR mutation ownership slice review

## Verdict

**Iterate. Keep the serial PR unpublished and the Task incomplete.** Shared PR
creation, metadata editing and readiness now retain the release capabilities.
This advances the full design through the existing finalization owner. It does
not establish complete mutation-child exclusion or configured acceptance. No
additional bounded defect was established in this slice; no executable code
changed during this review.

Starting head: `4ecc50540260b03f8dfd7ec23c9f95147df37744`.
Obtained the complete Task patch with `lf task diff LOO-285 --json`: 647,150
characters, `truncated: false`. Compared it with the preceding model review:
only the compression report changed. Reviewed the directive, full design,
current slice, forbidden outcomes and Done when against the implementation.
Prior test receipts below remain historical evidence, not fresh validation.

## Demonstration

The built-CLI fixture
`surviving_release_pr_mutation_retains_target_and_checkout` passed all eight
scenarios in 60.85 seconds. It crosses creation,
base retargeting, title/body editing and readiness with killed-controller and
failed-launcher exits. Each case observes a competing release defer, ordinary
checkout removal fail, and release-note bytes remain accessible. After the child
exits, it checks simulated remote head/base/title/readiness state and restored
release and cleanup access.

Git, bare origins, processes and OS locks are real. GitHub and notes generation
are simulated. The metadata case observes the resulting title; body transmission
shares that command but is not independently asserted as remote state. These
cases cannot establish hosted PR behavior, provider descendant inheritance,
launchd timing, UI-host verification, public artifacts or either configured
settlement. No production mutation was used to manufacture evidence.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| PR mutation survival | Exclude competing release and checkout removal through child exit | Creation, retarget, edit and ready inherit both held capabilities | Eight-case built-CLI demonstration | Pass locally |
| One release PR creation owner | Commit/push, then shared finalization creates or reuses PR | Release disables commit's draft creation and enables finalization creation | `prepare_release_in_worktree`, `ensure_pr`, `create_pr_from_pushed_branch` | Pass by source; creation exercised locally |
| Ordinary PR and release recovery | Preserve normal arming and advancing-main reintegration | Shared commands retained; ordinary callers supply no release capability | Prior ordinary-arm, reintegration and auto-merge survival passes; unchanged executable content | Retained local evidence |
| All mutation children | Every surviving side-effect child retains its owner's capabilities | Shared Git, notes, tools, worktree and Task compensation paths remain uncovered | Call graph below | Gap |
| Repeated firing and stable due identity | One due key and accepted settlement; activation survives sync | Deterministic keys and exact attempt fencing | Accounting writers and retained focused receipts | Retained local evidence |
| Delayed wake and execution crossing next due | Frozen catch-up set, one execution/result; later due waits | Atomic obligation document retains coverage and links; completion records later waits | `begin`, `finish_process`, prior joined fixture | Retained local evidence |
| Interrupted linking and candidate retry | No partial ownership; preserve original candidate and failures | Whole-document replacement and retained selection across preflight | Writer source and prior regressions | Retained local evidence |
| Manual trigger and repair | Preserve provenance; exclude unattended qualification | Trigger records, interventions and collapsed provenance remain | History qualification and prior regressions | Local evidence; live trigger race unproven |
| Overlap | One mutator with exact continuation | Target/job locks exclude overlap; some continuation remains prose | Lock source and demonstration | Partial |
| Crash around tag/publication | Resume the same exact candidate, retaining external effects | Existing recovery and publisher reconciliation | Prior tag, same-tag and publisher proofs | Full interruption/configured proof gap |
| Late result and process zero exit | No terminal regression or fabricated product success | Fenced atomic `settle`; wrapper fills only missing process outcome | `settle`, `finish_process`, prior tests | Retained local evidence |
| Failed telemetry | Scheduling health cannot clear verification; dated repair owner required | Current telemetry gates mutation; history retains failures and dispositions | `verify_scheduled_telemetry`, history, retained 36-failure baseline | Historical linkage/retry and accepted repair handoff gap |
| No-change and resumed candidate checks | Exact source, empty range and complete applicable verification | Shared completion and baseline public proof remain required | Prior joined and wrong-source regressions; producer source | Retained local evidence |
| Draft, wrong hash, missing asset or failed smoke | No complete publication; preserve external effects | Publisher requires stages, hashes, UI proof and public read-back | Prior Python and joined counterexamples | Local evidence; actual public/UI proof gap |
| Corrupt persistence | Fail with path and preserve evidence | Strict readers and atomic replacement | Accounting source and prior corruption case | Retained local evidence |
| Schedule/timezone/DST/Home changes | Preserve denominator and actionable old work | Calendar/segments retained; closed unfinished attempts lack continuation | `observe`, `close`, `receipt_context`, prior calendar tests | Continuation gap |
| Caller preservation and independent scopes | Exact caller bytes across every exit; unrelated scopes progress | Earlier joined/hook proofs and independent target/checkout locks | Prior preservation/isolation receipts; current checkout demonstration | Partial; remaining interruption paths open |
| Two adjacent automatic settlements | Distinct executions, at least one publication, all checks and no repair | No qualifying configured pair demonstrated | Acceptance ledger; fixtures ineligible | Gap |

## Source and negative architecture

Followed release preparation into `commit_workflow`, shared finalization and its
creation/metadata commands. The release call sets `create_draft_pr: false`; it
cannot enter commit's best-effort draft writer. Ordinary commit/Flow callers
still intentionally use that capability. Finalization reuses an existing PR or
creates one with the release title and notes. Its callback configures existing
commands and supplies no new lock owner, executor or persisted state.

The callback reaches direct PR mutations, including auto-merge replacement.
It does not reach `clear_task_pr_merge_before_head_mutation`, merge-request
replacement or stale-head invalidation inside Task operations: those revocations
still provide a no-op callback. The disposable release fixture does not exercise
managed Task compensation. This is retained required work, not a claim that
those paths are protected by passing metadata cases.

Inspected cron attribution, settlement/history, CLI/Flow context, the history
fixture, publisher receipt types and release documentation as direct consumers.
The repository flow remains one mechanical release operation. `settle` remains
the typed product-success writer; wrapper completion cannot promote process
success or overwrite accepted settlement. Searches found no restored duplicate
success-proof wrappers, `record_verification` writer or separate Python candidate
and publish receipt classes. Direct release worktree creation still disables
default-branch synchronization. These scoped findings do not establish capability
inheritance in every indirect helper or arbitrary external program.

## Next implementation direction

1. Complete ownership through shared commit/push, notes agents and lockfile
   tools, source checkout creation/reset/removal and Task compensation. The
   current preparation calls `commit_workflow`, `run_release_notes_stage` and
   `bump_manifest_versions` without the borrowed capabilities; rebuild reset
   still uses ordinary `run_stdout`. Preserve independent target/checkout scopes
   and prove controller death plus failed-launcher descendants at those boundaries.
2. Retain telemetry prerequisite identity and missing/failed evidence for every
   covered original due, with linked current recovery and the approved bounded
   once-per-wake retry. Record dated repair ownership. The observed missing
   `agent_turns` scorecard table remains a blocker; the Intelligence handoff is
   unaccepted. The retained 36 failures include the original 35 counterexamples.
3. Give closed unfinished obligations a supported continuation/disposition
   without transferring old Home authority. `close` retains their attempts but
   `receipt_context` rejects reuse; rows can remain Running indefinitely.
4. Complete the remaining interruption/isolation proof before supported
   installation/sync and configured acceptance. Required UI-host/public exact-tag
   proof and two adjacent automatic executions remain mandatory. No new
   independent live publication blocker or sibling Task was identified here.

## Validation

`cargo test -p loopflow --test release_lock_tests surviving_release_pr_mutation_retains_target_and_checkout -- --nocapture`
passed: eight scenarios, 60.85 seconds execution after 23.16 seconds compilation.
No other tests or static checks were rerun for this documentation-only review.
Prior focused passes retain their stated scope. No affected-suite gate, full CI,
install/sync, cron trigger, production publication, PM handoff, PR publication,
landing or Task completion occurred.
