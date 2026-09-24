# Source ownership slice review

## Verdict

**Iterate. Keep the serial PR unpublished and the Task incomplete.** Source
fetching, checkout mutation and cleanup now retain their existing capabilities.
This review reproduced and repaired an additional cleanup bypass after worktree
creation. The slice advances the complete design, but Task compensation,
prerequisite recovery, closed-obligation continuation and configured acceptance
remain open.

Starting head: `96d71a1c468955905f439fad91d87a566a96df26`.
Obtained the complete Task patch with `lf task diff LOO-285 --json`: 811,039
characters, `binary: false`, `truncated: false`. Reviewed the directive, complete
design, current slice, forbidden outcomes and Done when. Source inspection
concentrated on the source/worktree changes, the subsequent removal of main-sync
mode, and their ownership and evidence consumers. Prior receipts remain
historical evidence; fresh demonstrations are distinguished below.

## Demonstration and repair

The built-CLI fixture
`surviving_release_source_mutation_retains_ownership_and_caller_state` passed
all twelve scenarios in 46.98 seconds. Initial origin fetch, recovery-branch
fetch, checkout creation, rebuild reset, removal and branch deletion each run
with controller death and a failed launcher leaving a descendant. The fixture
observes competing-release exclusion, exact checkout protection where held,
actual resulting Git state, unchanged remote branches and caller HEAD, branch,
raw index, staged/unstaged/untracked bytes and unpublished commit.

Git, bare origins, the CLI, processes and OS locks are real. GitHub observations
are simulated; shell barriers pause Git operations. This is local ownership and
resulting-state proof, not installed scheduling, live publication, UI-host proof
or either configured automatic settlement.

Source inspection then found a distinct bypass: after creating an exact-source
checkout, a HEAD mismatch caused `materialize_exact_source_worktree` to remove
it using the same borrowed lease passed into creation, then delete its branch.
A descendant can still hold that shared description. The mismatch also means
unexpected commits exist; creation ownership is no authority to discard them.

The new CLI regression installs a real Git post-checkout hook. It commits
`repair.txt`, starts a descendant paused before reading that file, then exits
unsuccessfully. Shared Git creation correctly recognizes the materialized
checkout despite the hook exit, so release reaches its HEAD-mismatch path.
Before repair, the controller deleted the live descendant's checkout; the test
failed in 5.46 seconds with `mismatch cleanup removed live hook checkout`.

Removed those two cleanup calls. The mismatch now reports observed/expected
HEAD, checkout path and branch, retaining both for inspection before retry.
Existing pre-creation classification still rejects divergence; normal stage
cleanup still drops its lease and independently reacquires it. No new lock,
classifier, ownership store or recovery mode was added.

The new regression passed after repair in 5.77 seconds. It checks that the
branch still names the hook's commit, a competing release defers, ordinary
removal is blocked, and the surviving descendant reads its original work.
After the descendant exits, target access and ordinary removal return; removal
retains the branch and its commit. Caller HEAD, branch and raw index are unchanged.
The test uses real Git/hooks/CLI/processes/locks and simulated GitHub. It does not
prove arbitrary vendor descendants forwarding descriptors or every hook side
effect. No production mutation supplied evidence.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Source mutation survival | Keep target ownership through child exit; protect exact checkout where held | Fetch/add/reset/remove/delete inherit existing capabilities | Twelve-case CLI demonstration | Pass locally |
| Creation failure with live descendant | Preserve unexpected work and active source | HEAD mismatch retains checkout and branch | New real-hook regression failed before repair, passed after | Pass for reproduced boundary |
| Actual Git result and caller preservation | Refresh intended refs/source; preserve caller and remote branch set | Shared Git owners, explicit source selection, local named creation | Twelve-case demonstration; prior compression test | Pass at exercised boundaries |
| One source/publication owner | No implicit caller reset or source branch push | Named helper has no sync mode or background push; ordinary placement retains its own behavior | Reachable source/caller inspection | Pass by source |
| Complete child ownership | Every side-effect child retains required capabilities | Task revocation/compensation still passes no-op inheritance | Task/PR call graph | Gap |
| Stable dues and repeated firing | Preserve activation, one due key and terminal settlement | Deterministic keys and fenced atomic settlement | Accounting source and retained tests | Retained local evidence |
| Delayed wake and later dues | One execution for frozen coverage; later dues wait | One obligation document retains links/coverage; completion records later waits | `begin`, `finish_process`, prior joined proof | Retained local evidence |
| Interrupted collapse and candidate retry | Preserve owner, selection and failed attempts | Atomic document and carried selection | Accounting writers and earlier regressions | Retained local evidence |
| Manual provenance and timing | Retain intervention and first attempt through collapse | History reads collapsed provenance and frozen coverage | History source and earlier regressions | Local evidence; live trigger race gap |
| Overlap continuation | One mutator, exact continuation | OS exclusion works at covered boundaries; some continuation remains prose | Demonstrations and target-lock error path | Partial |
| Crashes around tag/publication | Resume exact candidate and retain external effects | Existing recovery and publisher reconciliation | Prior same-tag and publisher cases | Full interruption/configured proof gap |
| Late result or process zero exit | No terminal regression or fabricated publication | Atomic fenced `settle`; wrapper only fills missing process result | Settlement/finish source and prior preservation cases | Retained local evidence |
| Failed telemetry and repair ownership | Every original prerequisite retained with current recovery and dated owner | Current interval gates mutation; historical association and bounded retry absent | `verify_scheduled_telemetry`; retained 36-failure baseline | Gap |
| No-change/resumed candidate checks | Exact source, empty range and complete required evidence | Shared completion and verified baseline remain required | Producer source and prior joined/wrong-source cases | Retained local evidence |
| Incomplete publication | Reject draft/wrong hash/missing asset/smoke failure; retain effects | Publisher requires stages, hashes, UI and public proof | Prior Python and joined counterexamples | Local evidence; actual public/UI gap |
| Corrupt persistence | Fail explicitly and preserve evidence | Strict readers and atomic replacement | Accounting source and prior corruption case | Retained local evidence |
| Schedule/timezone/DST/Home changes | Preserve denominator and actionable old work | Calendar/segments retained; closed unfinished attempts lack continuation | `close`, `receipt_context`, prior calendar cases | Continuation gap |
| Independent scopes and all caller exits | Unrelated work progresses; no caller/source loss | Separate target/checkout locks and covered preservation cases | Current demonstration plus prior isolation receipts | Partial; complete interruption matrix open |
| Two adjacent automatic settlements | Two executions, at least one publication, all checks, no repair | No configured qualifying pair demonstrated | Acceptance ledger; simulations ineligible | Gap |

## Source and negative architecture

Followed release entry through explicit origin fetching, named creation,
exact-source classification, PR rebuild reset and cleanup into shared Git
operations. Ordinary removal acquires its lease and delegates to the same
removal implementation. Release cleanup independently reacquires and holds the
new lease through branch deletion. The deleted mismatch path no longer bypasses
that boundary or discards an unexpected branch. Ordinary placement alone retains
its upstream synchronization; named source creation has no `sync_main` path or
background publication. Existing source and branch classifiers remain shared.

Inspected CLI/Flow receipt attribution, accounting writers, history qualification
and the JSON fixture, publisher receipt/descriptor consumers, and release docs.
The repository Flow remains one mechanical release operation. `settle` owns
typed product success; `finish_process` cannot promote zero exit or overwrite
accepted success. Target exclusion, checkout protection and cron attribution
retain distinct scopes. Searches found no restored duplicate success-proof
wrappers, separate `record_verification` writer, separate Python candidate/publish
receipt classes or Swift consumers of the release-history/opportunity/public
receipt types. These scoped findings do not prove arbitrary external programs
retain descriptors or every materialization error is safe.

## Next implementation direction

1. Complete Task revocation/compensation capability propagation and its concrete
   reachability/proof. `clear_task_pr_merge_before_head_mutation`, merge-request
   replacement and stale-head compensation still call `disable_auto_merge` with
   no-op inheritance. Preserve Task settlement authority and independent scopes.
   Finish remaining materialization/interruption cases without restoring cleanup
   authority merely because the parent created a checkout.
2. Retain the telemetry prerequisite for every original covered due, including
   missing/failed historical evidence and linked current recovery. Implement the
   accepted once-per-wake bounded retry and dated repair ownership. Current lookup
   still uses the latest interval and two days of receipts. The observed missing
   `agent_turns` scorecard table and unaccepted Intelligence handoff remain; all
   36 observed failures, including the original 35, remain counterevidence.
3. Give closed unfinished obligations an explicit supported continuation or
   disposition without transferring old Home authority. Closure retains attempts
   while receipt-context validation rejects reuse; rows can remain Running.
4. Finish interruption/isolation proof before supported installation/sync and
   configured acceptance. Preserve 09:00 telemetry and 10:00 release, required
   UI-host/public exact-tag proof, and two adjacent automatic executions with at
   least one publication and no manual repair. Collapsed misses cannot supply
   additional settlements. No independent live publication blocker or sibling
   Task was identified here.

## Validation

- Twelve source survival scenarios: passed, 46.98s, before the mismatch repair.
- `cargo test -p loopflow --test release_lock_tests source_creation_mismatch_preserves_surviving_hook_and_its_work -- --nocapture`: failed before repair (5.46s), passed afterward (5.77s).
- `cargo fmt --check` and `git diff --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed, 19.83s.

The changed error path has the focused regression as its final proof. The
unchanged source-inheritance demonstration was not rerun for ceremony. No
full affected-suite gate, hosted matrix, installation, cron trigger, production
release, PM handoff, PR publication, landing or Task completion occurred.
