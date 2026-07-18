# Let reconciliation trust the settled PR set plus actual Linear completion

## Problem

`lf task reconcile` (shipped in #1087) settles a merged, Linear-complete Task
whose final directive was applied but never acknowledged — an out-of-band
Wave/Operator attestation, no provider turn, no new PR. Its guard, the
supervisor's Reconcile suppression, and the action recommendation all key on
**one predicate**: `latest_pr_completes_task` — the *newest PR by sequence* must
be a merged `CompleteTask`.

That predicate is correct for *automatic* completion (an older `CompleteTask`
behind a later `Review` merge must never auto-complete the Task). But it is too
strict for *reconciliation*. The live ENG-29 shape, exposed once ENG-30 merged,
is:

- an earlier merged `CompleteTask` PR,
- a **later merged `Review`** PR (now the newest *material* merge),
- an **abandoned, unpublished successor row** (a rotated branch that never became
  a real PR — the newest row by sequence),
- an applied-but-unincorporated final directive,
- the owning-team Linear issue actually **complete**.

`latest_pr_completes_task` reads the newest row (the abandoned successor, or the
merged `Review`) — neither is a merged `CompleteTask` — so reconciliation
*refuses*, the supervisor won't suppress a relaunch, and the action model
recommends `StartNextPr`. The Task strands forever, even though a human can
prove it shipped and Linear agrees.

This is a Developer-Efficiency KR: *"zero durable commands are left orphaned
'uncertain' against a dead generation."* This exact orphan is the one that
strands.

## The demo

```bash
lf task reconcile ENG-29 \
  --directive 6 \
  --summary "final direction shipped in the merged head; Review landed after the CompleteTask"
```

A Task whose newest merge is a `Review` sitting ahead of an older `CompleteTask`,
trailed by an abandoned unpublished successor, settles to `Completed`, its
orphaned recovery command clears in place — where before the same command
refused with *"its newest pull request is not a merged CompleteTask."*
`lf task status ENG-29` recommends `reconcile`, not `start_next_pr`.

## Approach

Split the one predicate into two, keyed by consumer:

- **Automatic completion stays strict.** `latest_pr_completes_task` is unchanged
  and remains the sole input to `merged_completing_pr` (and thus
  `advance_completion_after_gate`). An older `CompleteTask` behind a later merge
  still never auto-completes.

- **Reconciliation trusts the settled PR set.** A new predicate,
  `reconcilable_pr_set`, expresses the directive's three clauses:
  1. *Ignore abandoned unpublished successor rows* — select the **latest
     non-abandoned PR** by sequence, so a trailing abandoned successor never
     hides the real settled merge.
  2. *Require no active or unmerged non-abandoned PR* — nothing still in flight.
  3. *Allow the latest non-abandoned PR to be any merged disposition* —
     `Review` or `CompleteTask`.

  ```rust
  pub(crate) fn reconcilable_pr_set(prs: &[TaskPr]) -> bool {
      let none_in_flight = !prs
          .iter()
          .any(|pr| pr.abandoned_at.is_none() && pr.phase() != PrPhase::Merged);
      let latest_non_abandoned_merged = prs
          .iter()
          .filter(|pr| pr.abandoned_at.is_none())
          .max_by_key(|pr| pr.sequence)
          .is_some_and(|pr| pr.phase() == PrPhase::Merged);
      none_in_flight && latest_non_abandoned_merged
  }
  ```

Align the three reconciliation consumers on `reconcilable_pr_set`:

- **Command guard** (`task_reconcile`): replace the `latest_pr_completes_task`
  gate with `reconcilable_pr_set`; the refusal names "no non-abandoned PR has
  merged" / "a non-abandoned PR is still in flight." The Linear-complete proof
  and the existing `active_task_pr` / clean-tree / applied-delivery guards are
  untouched — reconciliation is still fail-closed on the actual Linear state.
- **Supervisor suppression** (`reconcile_project_tasks`): swap the suppression
  gate to `reconcilable_pr_set`, so the merged-`Review`-plus-abandoned-successor
  shape leaves the `Reconcile` recommendation standing rather than relaunching a
  body. Suppression only `continue`s — it never auto-completes, so automatic
  completion stays strict.
- **Action recommendation** (`derive_task_actions`): add a `reconcilable: bool`
  to `TaskActionEvidence` (computed as `reconcilable_pr_set(&prs)` in
  `task_snapshot`) and hoist a single Reconcile branch above the per-latest-PR
  phase match: `pending_directive && directive_applied && reconcilable →
  Reconcile`. This subsumes the old `merged_pr_model` reconcile block (which keyed
  on `latest_pr_after_merge == CompleteTask`), so Reconcile now lives in exactly
  one place and fires for the merged-`Review` shape too.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Does loosening reconcile weaken the sabotage guard #1087 added? | No. The sabotage the directive preserves is *a later non-abandoned open or unmerged Review* — `reconcilable_pr_set` returns false for it (that PR is non-abandoned and not merged → in-flight). The #1087 test's *merged*-Review case intentionally flips to success now; its automatic-completion assertions stay. | Rewrite `an_older_completetask_behind_a_later_review_never_reconciles` into two tests: the merged-Review ENG-29 success and the open-Review sabotage refusal. Keep `advance_completion_never_completes_from_an_older_disposition` and `latest_pr_completes_task_keys_on_the_newest_pr` unchanged. |
| Can the hoisted action-model Reconcile branch fire while a PR is genuinely active? | No. `reconcilable_pr_set` is false whenever any non-abandoned PR is unmerged (Working/Publishing/Open). So the branch only fires when nothing is in flight — safe to hoist above the phase match. | Place it after the review-gate block (review gate keeps precedence), before the `latest_pr_phase` match. |
| Does automatic completion change at all? | No. `merged_completing_pr` keeps calling `latest_pr_completes_task`. The only behavioral change is which *reconciliation* shapes are permitted. | `latest_pr_completes_task` keeps its body; only its doc comment (which claimed the reconcile guard shares it) is corrected. |
| Is "material" abandoned-published vs abandoned-unpublished a real distinction here? | The directive names the concrete blocker (abandoned *unpublished* successor). Selecting the *latest non-abandoned* PR ignores every abandoned row uniformly — simpler and correct; the only shape the directive's sabotage boundary cares about is non-abandoned + unmerged, which still refuses. | Single "latest non-abandoned merged" rule; no abandoned/published sub-case. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Add a `reconcilable` flag / disposition field to the Task or PR row | New persisted state, migration, another mirror to keep in sync | Directive forbids "another Task field or disposition shim"; the settled PR set already carries the truth. |
| Loosen `latest_pr_completes_task` itself so both paths share it | Would let automatic completion fire from an older `CompleteTask` behind a later merge — the exact #1087 sabotage | Automatic completion must stay strict; only reconciliation (attestation-gated + Linear-verified) can trust the broader set. |
| Have the action model call `reconcilable_pr_set` directly | `derive_task_actions` is a pure function over evidence; reaching into the store breaks that | Add one evidence field, computed once in `task_snapshot`. |

## Key decisions

- **Two predicates, one per consumer class.** Automatic completion is machine
  truth (strict, newest = merged CompleteTask). Reconciliation is
  attestation-gated human/Wave truth backed by a live Linear read, so it can
  trust the whole settled set. The names carry the distinction:
  `latest_pr_completes_task` vs `reconcilable_pr_set`.
- **Reconcile lives in one place in the action model.** Hoisting subsumes the
  old CompleteTask-only block; no duplicated recommendation logic.
- **No new persisted state.** Everything is derived from the existing `TaskPr`
  rows plus the existing directive/Linear guards.

## Scope

- In scope: `reconcilable_pr_set` predicate; rewire the reconcile command guard,
  supervisor suppression, and action recommendation; the ENG-29 regression; the
  open/unmerged-Review sabotage; doc-comment corrections.
- Out of scope: any change to automatic completion, `latest_pr_completes_task`,
  the Linear-complete proof, or the applied-delivery / clean-tree / active-PR
  guards. No new Task or PR field.

## Done when

- `cargo test -p loopflow` green, including a new regression that builds the exact
  ENG-29 shape (merged `CompleteTask`, later merged `Review`, abandoned
  unpublished successor, applied unincorporated directive, Linear complete) and
  asserts `task_reconcile` succeeds, the Session reaches `Completed`, and the
  orphaned recovery command clears.
- A preserved sabotage test where a later **non-abandoned open/unmerged** Review
  makes `task_reconcile` refuse.
- `snapshot.actions.recommended == Reconcile` for the merged-`Review` shape.
- `cargo fmt` and `cargo clippy -- -D warnings` clean.

## Measure

Not a quantitative change. The observable outcome is the demo: a previously
un-settleable Task settles, and its orphaned recovery command clears — moving the
Developer-Efficiency KR "zero durable commands left orphaned against a dead
generation" toward green.
