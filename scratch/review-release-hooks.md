# Release hook slice review

## Verdict and next slice

**Iterate. Keep the serial PR unpublished and the Task incomplete.** Hook
inheritance advances the full design. This review reproduced and repaired a
separate branch-preservation defect in PR rebuild. Passing local process tests
does not close the remaining execution or configured acceptance obligations.

Continue in this Task:

1. Carry the existing target and checkout capabilities through shared commit,
   PR creation/arming/re-arming, notes, lockfile tools and source/cleanup mutation
   children. `prepare_release_in_worktree` reaches `commit_workflow` and
   `finish_arm_after_rebase` without those borrowed capabilities;
   `wait_for_pr_merge` reaches `enable_auto_merge` likewise. Worktree creation,
   rebuild reset and removal still launch ordinary Git children. Prove controller
   death and launcher failure at these boundaries, preserving independent target
   and checkout scopes. The hook shell proof does not cover arbitrary programs
   that close inherited descriptors before launching descendants.
2. Retain each original covered telemetry prerequisite and its missing/failed
   evidence, implement the approved bounded current retry, and record dated
   repair ownership. Current verification still searches two days for the latest
   due interval. The observed missing `agent_turns` scorecard table remains the
   named verification blocker; the Intelligence handoff remains unaccepted.
3. Give unfinished closed obligations a supported continuation or disposition
   without transferring Home authority. Retained rows can still remain Running
   after `receipt_context` refuses the closed segment.
4. Finish interruption/isolation proof, then supported installation/sync and
   required UI-host/public artifact smoke. Observe two adjacent automatic
   executions, at least one publishing, with all required verification and no
   manual repair. No fixture below supplies that acceptance.

No independent new live publication blocker was demonstrated or sibling Task
opened. Original 35 failed telemetry targets remain counterevidence; the retained
baseline has 36. No fresh production telemetry read was made in this review.

## Review boundary and demonstration

Starting head: `3d883ae4c08d42b572ebc9f97dd9d8df16d1b58d`.
Obtained the complete Task patch through `lf task diff LOO-285 --json`:
576,256 characters, `truncated: false`. Reviewed the directive, complete design,
Done when and forbidden outcomes, current hook slice, preceding evidence, and
the affected ownership paths. Inspected their accounting/history, CLI/Flow,
publisher, fixture and documentation consumers. The patch includes historical
reports and captured receipts; those are retained evidence, not fresh tests.

The built-CLI hook demonstration passed all four verification/preparation ×
controller-death/failed-launcher cases (14.64s). The fixture kills and reaps only
its own controller, or lets it fail into real cleanup while a child survives.
It observes target exclusion, blocked ordinary removal, usable source bytes,
recovery after child exit, and exact caller HEAD/branch/index plus staged,
unstaged and untracked preservation. Git, the bare origin, processes and OS locks
are real; GitHub is simulated. These are disposable repository proofs, not
launchd timing, actual publication or installed scheduled settlements.

## Reproduced and repaired

PR rebuild used `create_named_worktree`, which can prefer an existing local
branch when its remote branch exists. After materialization, rebuild compared
HEAD with the observed PR head inside a closure whose error still went through
unconditional cleanup. A divergent local release branch therefore failed the
comparison and was then deleted, including its reference to unpublished repair
commits.

The new integration regression publishes a release branch to a disposable bare
origin, adds an unpublished local repair, checks out main and requests release
recovery against a simulated open/dirty PR. Before repair, release returned the
head-mismatch error but `rev-parse jack/release-default-v0-9-2` failed: cleanup
had deleted the local branch. This is stronger evidence than the error message
claiming refusal.

Rebuild now calls the existing `materialize_exact_source_worktree` with the
observed PR head and its already-held checkout lease. Its classification occurs
before creation/reset or cleanup. Divergent branches and dirty existing trees
stay intact; a valid exact source can use existing recovery. Removed the later
duplicate HEAD comparison. No new classifier, lock, store, cleanup service or
compatibility path was added.

The regression now preserves the branch SHA, committed repair bytes and caller
HEAD/branch/index (2.32s). Normal reintegration with main advancing during
preparation also passed (6.86s), including repeated preparation on the refreshed
origin. The change neither completes subprocess inheritance nor alters product
settlement or prerequisite policy.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Hook survival | Target and checkout remain protected through child exit | Both borrowed descriptors reach the hook shell | Four-case built-CLI demonstration | pass locally |
| Launcher failure | Cleanup cannot delete surviving child's source | Drop controller lease, independently reacquire for removal | Same demonstration, failed-launcher cases | pass locally |
| Caller preservation | Keep local commits, branch/index and edited bytes | Selection uses origin; creation disables caller sync | Same demonstration | pass for exercised exits |
| Rebuild preserves local repair | Reject divergence without destroying work | Existing exact-source classifier precedes reset/cleanup | New red/green regression | pass for reproduced boundary |
| Rebuild remains useful | Refresh main and rerun preparation | Explicit origin fetch, reset only accepted owned checkout | Existing reintegration case passes | pass locally |
| Complete child exclusion | Every surviving mutator retains required capabilities | Shared PR/notes/tools/Git children remain reachable gaps | Call graph listed above | gap |
| Original dues and frozen catch-up | Every due retained; one execution/result; later dues wait | Atomic obligation document and saved covered keys | Accounting source and earlier focused/joined receipts | retained local evidence |
| Candidate continuity and late results | Preserve selection; reject obsolete or conflicting settlement | Exact attempt fencing, atomic outcome/proof, retained rejected input | `begin`, `select`, `settle`; previous regressions | retained local evidence |
| Process success | Zero exit cannot establish product success | `finish_process` leaves missing proof Unverified | Writer inspection and earlier joined proof | retained local evidence |
| Timing and intervention | Collapse cannot erase original timing or manual repair | History reads all covering attempts and collapsed provenance | History source and prior regressions | retained local evidence; live trigger race gap |
| Telemetry | Preserve every original prerequisite and current recovery | Latest interval gates mutation, historical associations/retry absent | `verify_scheduled_telemetry`; retained failed receipts | gap |
| Exact candidate/no-change | Exact source checks, empty range and complete baseline proof | Shared completion and public verification remain | Producer/source; earlier joined proof | retained local evidence |
| Public outcome and post-publication failure | All stages/hashes/UI/smoke; preserve effects when smoke fails | Publisher reads/repairs stages and persists effects before final proof | Publisher source and prior Python/CLI receipts | live proof gap |
| Removed/changed obligations | Retain denominator and actionable unfinished work | Retained segments lack complete continuation | `observe`, `close`, `receipt_context` | gap |
| Corruption, DST, historical unknowns | Preserve evidence and missingness | Strict reads, atomic replacement, shared calendar | Source and earlier focused receipts | retained local evidence |
| Independent scopes | Other targets/repos/checkouts remain usable | Separate target lock and exact checkout lease | Prior direct-child/publisher proofs; source | retained local evidence |
| Complete interruption matrix | No caller loss or competing mutation at any required stop | Covered hook and publisher stops; shared mutation stops incomplete | Current and preceding demonstrations | partial |
| Two adjacent configured settlements | Two autonomous executions, one publication, all proof | No qualifying configured pair demonstrated | Acceptance ledger remains open | gap |

## Negative architecture and validation

The repository flow remains one mechanical release operation. Cron attribution,
target mutation exclusion and checkout removal protection remain separate
capabilities. `settle` remains the sole typed product-success writer;
`finish_process` cannot promote a zero exit or overwrite accepted success.
Searches found no restored duplicate success-proof wrappers, `record_verification`
writer or separate Python candidate/publish receipt types. No Swift consumer of
the release-history/opportunity/public-receipt names was found. Historical
schema-1 physical receipts remain intact.

The remaining direct `create_named_worktree` calls in release source explicitly
disable default-branch sync; rebuild now uses the existing exact-source helper,
whose creation call also disables it. This is scoped source evidence, not the
earlier overly broad claim that every indirect release path lacks synchronization.
Shared child gaps remain explicit. The repair removes a second validation path
instead of introducing another ownership abstraction.

- `cargo test -p loopflow --test release_lock_tests surviving_release_hook_retains_target_and_checkout_ownership -- --nocapture`: passed, four scenarios, 14.64s.
- `cargo test -p loopflow --test release_tests release_pr_rebuild_preserves_divergent_local_branch -- --nocapture`: reproduced deletion before repair; passed afterward, 2.32s.
- `cargo test -p loopflow --test release_tests release_run_reintegrates_a_dirty_existing_pr -- --nocapture`: passed, 6.86s.
- `cargo fmt --check`: passed after formatting the regression.
- `cargo clippy --all-targets -- -D warnings`: passed (35.07s including build-lock wait).

The hook demonstration precedes the rebuild-classification repair. Its shell
inheritance/cleanup code is unchanged; the focused rebuild tests exercise the
changed behavior. No full affected-suite gate, hosted matrix, UI automation,
installation/sync, cron trigger, production release, PM handoff, PR publication,
landing or Task completion was performed.
