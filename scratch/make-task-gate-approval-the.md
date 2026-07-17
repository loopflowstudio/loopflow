# Make managed Task land the sole shipping declaration

## Problem

Managed Tasks currently expose two incompatible human decisions: durable
provider-backed `InteractionReview` checkpoints and a later `submit`/GitHub
merge click. The duplicate gate can either let merge arm before the authored
lifecycle finishes or leave an approved Task waiting for an unexplained second
approval.

The product contract needs one judgment plane. Every authored required
checkpoint in Kickoff, Iterate, or Gate is resolved in its owning LLM session.
`lf pr land -c` or `lf pr land --next <slug>` then declares that the exact
reviewed outcome is ready to ship. GitHub remains authoritative for required
checks, branch protection, merge queue execution, and the observed merge.

This directly advances the Loopflow API KR that every dispatched Task either
lands unattended after its checkpoints clear or stops with an actionable
non-convergence record, with no GitHub rescue.

## The demo

Run a managed Task through publish, requested changes, approval, and restart.
`lf pr land -c` refuses during Iterate and while checks or reviews are pending;
after the final Gate clears it records one SHA-pinned declaration, survives a
retry without issuing a second merge request, and GitHub settles the PR without
opening a browser or asking anyone to click Merge.

## Approach

Persist one `TaskSettlementIntent` inside `TaskGateProposal` containing the Task
PR identity, reviewed head SHA, material worktree fingerprint, lifecycle-clear
time, complete-versus-next disposition, declaration time, and remote-arm time.
The runner creates the evidence when Iterate enters Gate, marks it lifecycle
approved only after the entire authored Gate finishes with the same material
fingerprint, and returns requested changes to the correct owning phase.

Treat managed PR operations as Task-authority operations:

1. `lf pr publish` commits and publishes evidence during work but records no
   shipping disposition and never arms merge.
2. `submit` and `pr open` refuse managed Task worktrees. Ordinary non-Task
   behavior remains unchanged.
3. `land -c/--next` requires the active leased Task provider session, Gate
   completion, every current required review approved, incorporated directives,
   an open PR, a settled stack parent, no known pending or failing checks, and
   exact PR/head/worktree evidence.
4. Persist the chosen disposition before any GitHub mutation. Arm with
   `gh pr merge --auto --match-head-commit <reviewed-sha>` so GitHub rejects a
   head race at the mutation boundary.
5. Reconcile remote `headRefOid` and `autoMergeRequest` together on retry. If
   the reviewed head is already armed, record `armed_at` without another merge
   request. If the remote head changed, disable any stale auto-merge request,
   return the same Task to Iterate, and preserve the stale declaration as audit
   evidence.
6. Complete or rotate only after GitHub reports the merge and the required
   review gate still holds. CI failure keeps the same PR open and invokes the
   existing repair lifecycle.

Status, recommended actions, Mac consumers, docs, and builtin prompts derive
from the same lifecycle and settlement records. No consumer infers approval
from GitHub review state.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Can GitHub enforce the reviewed head at the merge request boundary? | Installed `gh pr merge --help` exposes `--match-head-commit SHA`, including with `--auto` and merge queues. | Managed arming must pass the persisted reviewed SHA, not rely only on a preceding local comparison. |
| Can approval be inferred from all review rows currently present? | No. A later required Gate step may not have opened its review row yet. | Persist `lifecycle_approved_at` only after the complete authored Gate finishes; land requires it in addition to approved review rows. |
| Does changes-requested history permanently poison settlement? | It would if every historical required row were treated as current. A restarted phase creates a later review for the same phase/flow/step. | Keep rejected reviews immutable as audit evidence and select only the latest lifecycle entry per checkpoint for settlement. Kickoff and Iterate restart their owning phase; Gate returns to Iterate. |
| Can a crash between GitHub mutation and the local `armed_at` write duplicate landing? | Yes unless retry first reads remote auto-merge state. | Record intent before the network call, then reconcile remote head plus `autoMergeRequest` before deciding whether to arm or only finish the local receipt. |
| Do mixed require/require/require, require/defer/require, and defer/defer/defer rows require migration? | No. They are valid authored policies. Deferred exercises already use the Project provider review protocol and must finish before the lifecycle can clear. | Migration adds settlement state only and never rewrites phase policy or completed review history. |
| Does GitHub need to become lifecycle authority to retain branch protection and merge queues? | No. `gh pr merge --auto` delegates execution to required checks and the merge queue; the observed merge remains the settlement fact. | Keep human judgment in `InteractionReview`; keep execution and settlement evidence in GitHub observations. |
| Can a stacked child declare ready before its parent settles? | Status already identifies an unmerged predecessor, but a CLI command could bypass a derived recommendation. | Enforce parent `Merged` state in managed-land authorization, not only in presentation. |
| Is the migration ordinal stable across active branches? | PR #1053 also claimed `0.11.027`; this branch was rebased to `0.11.028`. The migration ledger is shared and order races are a known product-wave hazard. | Rebase and run migration provenance checks again after #1053 settles; do not land this migration against an ambiguous ordinal chain. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep `submit` and treat the GitHub merge click as final approval | Familiar repository workflow, but duplicates judgment without Task phase, transcript, evidence fingerprint, or recovery semantics. | It preserves the exact second approval this Task removes and strands headless Tasks. |
| Have the runner arm merge automatically as soon as Gate approves | Removes a command, but loses the explicit complete-versus-next declaration and makes crash ownership harder to audit. | `land -c/--next` is the durable managed-Task declaration and must carry the serial disposition. |
| Let Iterate call `land --auto` and trust GitHub checks to delay merge | Simple, but checks cannot know whether later required LLM checkpoints approved the product outcome. | GitHub execution evidence cannot substitute for lifecycle judgment. |
| Add a new Task-only landing daemon or outbox | Could centralize retries, but creates another process owner beside the existing leased Task Session and PR reconciler. | The active Task lease, durable settlement intent, and existing GitHub reconciliation already provide the required ownership boundary. |

## Key decisions

- There is exactly one human shipping decision: the applicable provider-backed
  lifecycle checkpoints. `land` declares their reviewed outcome; it does not
  ask for another human decision.
- The declaration is explicit and typed: `-c` means the observed merge may
  complete the Task; `--next <slug>` means the observed merge may rotate the
  same Task to one next serial PR.
- Gate completion is a durable fact separate from individual review rows.
  This prevents a partially traversed Gate from appearing approved.
- GitHub arming is SHA-pinned and replayed by observation. Local validation
  alone is insufficient across the network race.
- Known pending or failing checks block the declaration and preserve the
  existing repair lifecycle. Repositories with no required-check observation
  still rely on branch protection and merge queue execution.
- Missing registry authority, missing PR evidence, stale head/worktree state,
  changed directives, GitHub outage, and failed arming all stop with an
  actionable durable reason. None falls back to `submit`, `pr open`, or a
  browser.
- `require/require/require` is valid authored policy, not legacy drift.

Wild success is boring: a user reviews in one transcript, sees the Task wait on
CI, and later sees it merged or rotated without learning Loopflow internals.
Wild failure is a second implicit authority creeping back in through status,
GitHub review state, or a retry that arms a different SHA; the source-of-truth
and SHA-pinning rules are designed to make those regressions test failures.

## Scope

- In scope: Task lifecycle transitions; durable review and settlement evidence;
  managed authority for publish/open/submit/land; replay-safe GitHub arming;
  complete-versus-next settlement; status and recommended actions; builtin
  prompts, CLI docs, migration, and focused lifecycle/CLI tests.
- Out of scope: removing ordinary non-Task `submit`; weakening branch
  protection; adding per-phase policy UI; redesigning generic interactive
  handoffs; rewriting completed review history; changing GitHub merge-queue
  semantics.

## Done when

- `cargo test -p loopflow task::runner::tests::` proves required Task and Project
  reviews park and resume through the existing provider transcripts.
- Focused lifecycle tests prove changes-requested restarts Kickoff and Iterate,
  Gate changes return to Iterate, and only the latest lifecycle entry for a
  checkpoint gates settlement.
- `cargo test -p loopflow --test land_tests` proves managed `submit`/`pr open`,
  incomplete Gate, pending checks, stale evidence, and detached land are refused
  before GitHub mutation; approved `-c` persists its disposition and replay
  produces one SHA-pinned auto-merge request.
- A restart test proves: intent written but GitHub not armed retries once;
  GitHub armed but `armed_at` absent heals locally; changed remote head refuses
  or disables stale auto-merge and returns to Iterate.
- `-c` completes only after an observed merge and approved current reviews;
  `--next` rotates only after the observed merge.
- `cargo test -p loopflow --test golden_prompt`, `cargo fmt --all -- --check`,
  `cargo clippy -p loopflow --all-targets -- -D warnings`, and
  `uv run python scripts/check_migrations.py` pass after the final rebase.
- CLI, status, README, operations docs, LOOPFLOW.md, and Task skills all teach:
  publish evidence, review in the LLM session, declare with land, let GitHub
  execute and settle. No managed surface teaches a human merge click.

The observable KR movement is one uninterrupted week in which every dispatched
Task either lands after its LLM-session checkpoints or stops with a named
non-convergence reason, with zero manual GitHub merge rescues.
