# Shared skill command review

LOO-298 · 2026-09-30 · Review of `fd9cf980b` through `9cb7b4c86`, the
following compression, and the bounded review repair below. Jack Heart requested
publication during this review so hosted Rust and Swift run on the checkpoint.
Publication is not acceptance of the remaining item 1 or complete-design claims.

## Findings

1. **Managed review dispatch lost its owner after claim release — repaired.**
   `checkpoint_in` deliberately clears the worker claim when reaching review.
   The new shared executor used that claim to choose Task review preparation,
   so an autonomous Task step followed by review used ordinary Flow review
   preparation instead. It lost the Task review identity/title and skipped
   managed checkpoint and background preparation. Dispatch now compares the
   Task's selected active Flow with the reviewed Flow, including runtime passes.
   A separately attributed Flow still uses ordinary review preparation.

   The previously passing public Chapter fixture did not detect this. Its retained
   database has `session_50d743deb69a41b89b6d9cac1c5c083a` titled `chapter-review`,
   rather than the managed Task review identity/title. Rechecking those stored
   observations fails the new assertion (`managed-review-red.json`); that is a
   retained-result check, not a fresh execution. The fixture now checks both facts.

2. **Task managed-account policy is enforced only at entry — unresolved.**
   Source inspection finds `preflight_task_execution` at creation/worker startup,
   but the deleted `TaskLauncher::account_for` also ran it when a later skill
   selected another agent. Startup skips provider preflight for an op boundary.
   The child enforces `task_execution_boundary`, which still restricts providers
   and supplies writable roots, but ordinary account selection permits no managed
   route. Therefore the existing note that the managed-account restriction is
   preserved throughout execution is too strong. An op-first Flow or later agent
   change can reach the direct path without that check. This is source evidence;
   no real credential fallback was attempted. Preserve the boundary input and
   resolve this retained policy exception with Jack before claiming parity closed;
   do not restore a Task launcher or silently weaken the stated policy.

3. **Two retained fixtures assumed claims still reserve inputs — repaired.**
   The first full review matrix found Chapter transfer expecting Started after
   a claim, and the live-unblock projection fixture unwrapping a nonexistent
   capture after claiming. Both now explicitly reserve the step input before
   their preservation/projection scenarios. Chapter transfer also proves the
   claim alone leaves Started unset. Their behavioral assertions remain intact.

## The named local failure

`controller::task::planning_tests::shared_driver_parks_after_releasing_its_claim_at_a_review`
was **deleted in `9cb7b4c86`**, not repaired under the same name. In
`agent-command-full.log`, its pinned executable was the Rust test binary, which
rejected `--batch` when the mechanical child tried to execute it. The complete
run recorded 2,032 passes, one failure and 16 skips; retain that failed result.

The remaining store test
`claimed_autonomous_boundary_settles_once_at_the_human_node` proves atomic claim
release and stale-settlement rejection. The public Chapter fixture proves the
actual command reaches review; its earlier assertions covered pending Session,
absent claim/failure and successful worker exit, but missed finding 1. Deleted
fake-launcher tests count as neither repaired tests nor replacement evidence.

## Architecture and evidence

Local logs, source fingerprints and fixture receipts are under `.lf/tmp/cut-i/`.

| Claim | Reviewed path and proof boundary |
| --- | --- |
| Skill and router execution use the direct command | `execute_child` → explicit `lf skill` → saved definition selection → `run_flow_skill` → shared prompt/capture/provider loop. No runtime definition lookup for a captured boundary. |
| Capture belongs to the actual step | Child reserves under version/claim and its Exec; publication checks the same owner. Public surviving-child test rejects a second child and consumes the original native completion once. |
| Removed authorities stay removed | Source/test search finds no `TaskLauncher`, `SavedLauncher`, `StepLauncher`, `CreateHarness`, old Task prompt preparation, Task account pin, or duplicate skill journal/checkpoint wrapper. |
| Driver death preserves work | Public Rust fixture retains child Exec, one native turn, exact completion consumption, immutable causal parent and unknown killed-driver outcome. Linux fixture covers managed worker death with a surviving scripted provider. |
| Task input uses the common provider loop | Task seed plus shared `TaskInput` supplies durable steers, interrupts, attachment and comment refresh; common retry loop retains its cursors. Account lease/failover and live control continuity across retry remain unproved. |
| Review does not settle the Flow | Source-free ordinary review fixture retains the captured skill and current Flow; managed review now checks Task identity after claim release. |
| Direct and agent-issued ancestry remains fenced | Existing Exec ownership tests cover current-driver/provider generations and shared engines. This cut does not simplify their owners yet. |

The first isolated, materialized full matrix (`skill-command-review-full.log`)
completed without fail-fast: **2,020 passed, two failed, 15 skipped**. It exposed
finding 3 and predates the review-owner repair. Its passing results do not prove
the final tree.

The final materialized snapshot ran with `--all --no-fail-fast` and four test
threads. The 900-second outer limit interrupted it under heavy shared host load:
1,875 passed, four running tests received SIGTERM, and 143 had not run. No
assertion failed in that run. The exact 147 unfinished tests then all passed
from the same snapshot. The inventory/coverage ledger verifies **2,022 unique
eligible tests passed, 15 ignored**, including both repaired fixtures. This is
a complete composite result, not an uninterrupted green matrix. Evidence:
`skill-command-review-final.log`, `skill-command-review-remaining.log`, and
`skill-command-review-coverage.json`. All-target Clippy with warnings denied,
formatting, architecture and diff checks passed.

The repaired public Linux command proof also passed
(`skill-command-review-managed.log` and its results directory): managed review
identity/title, attached steer, worker death with surviving native history and
no second turn, Project default/config precedence, Wave context, Chapter rotation,
and second-private-Home adoption through public sync. It uses real lf/tmux with
scripted Linear/Codex; it is not configured-provider acceptance. The final matrix
and public-proof source receipts match all 463 relevant Rust/Cargo/DTO/Chapter
fixture hashes in this checkout. Scratch review notes changed afterward.

## Next useful action

Close the [remaining managed-command proofs](exec-per-step.md#alternate-launcher-test-audit):
repeated decisions, explicit retry after uncertain native death, failure → one
unblock → feedback reassessment, provider/pre-publication failure release, and
account/control continuity. Keep the Task preflight exception visible for Jack.
Then follow attribution reduction, the separate naming decision, executed populated
import and canonical proof, and final docs in [remaining work](remaining-work.md).
This review supplies no navigation verdict, configured-provider/Desktop acceptance,
installed migration, promotion, merge or Task completion.
