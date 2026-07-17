# Do not complete a Task over committed follow-up work

Review status: directive v2 is incorporated and the reduced design is ready for
approval. Do not rebase or continue into implementation until approval is
recorded.

## Problem

W2-293/#1042: GitHub merged head `b00405e`; the Task worktree had acknowledged
directive v3 and committed follow-up `4d8d82e` on top. Reconciliation saw a
merged `complete_task` PR with no pending directive and marked the Task
`Completed`. The commit survived locally but fell outside Task ownership — no PR
carries it, no gate names it.

The completion decision reads `after_merge == CompleteTask` and
`has_pending_directive`, and neither can see committed work. An *acknowledged*
directive — the honest case — is exactly what slips through.

## The demo

A Task whose completing PR merged, with one commit past the merged tip: the gate
and every reconcile withhold completion. `lf pr next` rotates that exact commit
onto sequence 2. The Task completes only once nothing is left behind.

## Reduced v2

Keep the production change to two behaviors, both driven by the existing
`committed_follow_up_range` calculation:

1. **Completion guard.** A merged `CompleteTask` PR does not complete the Task
   while its branch has commits past GitHub's recorded `head_sha`.
   `reconcile_task_pr_with_authority` applies the rule to the PR being settled
   in flight; `task_completion_gate` applies the same rule to the newest durable
   PR on later reconcile and repair paths. These are the two entry points for
   one invariant, not separate mechanisms.
2. **Rotation exception.** `ensure_working_pr_with_authority` keeps the existing
   rule that a settled completing PR does not rotate, except when that same
   range is non-empty. The successor then carries the already-computed range.

The expected production footprint is 30–50 lines: one condition on the
in-flight completion decision, one newest-PR blocker in the durable completion
gate, and one condition on the existing rotation return. Do not add a helper or
explanatory layer, change `committed_follow_up_range`, introduce state, broaden
rotation, or add SHA/branch recovery instructions. The commits are the state.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Does `lf pr next` work on a completing PR? | No — the rotation bar returns `Ok(None)`, reported as "no settled PR to rotate from" (wave memory, ENG-19). | Blocking without relaxing that bar strands the Task. Same range gates both sides. |
| Does the blocker clear after rotation? | Only if read from the newest PR. The cherry-pick leaves the settled branch's commits in place, so a per-PR scan would block forever. | Gate reads `prs.last()`, matching what rotation calls `settled`. |
| Does this break normal completion? | No. The range is `None` when `head_sha` is unrecorded, the tip equals `head_sha`, or `head_sha` is not an ancestor (rewrite). | Existing gate tests (fixture `head_sha: None`) stay green untouched. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| New `follow_up_pending` column | Explicit, queryable | New schema, and a derived flag can disagree with git. |
| Block on a dirty worktree too | Catches uncommitted work | Would refuse completion for stray build artifacts. Out of scope. |
| Auto-rotate inside reconcile | No human step | Reconcile is read-side; minting a branch there is a large hidden mutation. Withhold and let `lf pr next` act. |

## Key decisions

- **The range is the state.** If git says there are commits past the merged tip,
  completion is not owed yet.
- **Newest PR only** — stated in the code, since it is the invariant that keeps
  the block clearable.
- **One guard, two representations** — merge observation checks the in-flight
  PR because it is not stored yet; subsequent completion attempts check the
  newest stored PR. Omitting either check leaves a completion race.

## Scope

- In scope: the committed-follow-up completion invariant and the completing-PR
  rotation exception.
- Out of scope: uncommitted follow-up, new state, compatibility handling,
  missing-worktree behavior, new helper predicates, and new user-facing
  diagnostics.

## Done when

`cargo test -p loopflow --lib ops::task` green, retaining only two focused
behavior tests:

- `completion_is_withheld_over_work_committed_past_the_merged_tip` reproduces
  the merged-PR observation with one descendant commit and proves both the
  immediate decision and a later durable reconcile withhold completion.
- `a_completing_pr_rotates_only_to_carry_committed_follow_up` proves the settled
  completing PR rotates to sequence 2 and the descendant commit is the work
  carried forward. Existing tests continue to own the ordinary no-follow-up
  completion and rotation cases.

No helper-level tests, schema tests, or broad lifecycle matrix are added.

Pre-existing, not owned here: `recover_refuses_a_non_abandoned_task` and
`recover_abandoned_task_adopts_existing_worktree_pr_and_direction` fail
identically on the clean base (ambient `LF_WAVE_ID` leak; wave memory).

## Measure

Developer Efficiency advances two proof thresholds: "No Task strands on a dead
body" and "A week of normal development ... requires zero manual git surgery."
Before: W2-293 completed over one owned follow-up commit, forcing out-of-band
recovery. After: the Task remains live and rotates that exact commit through its
normal serial-PR path.
