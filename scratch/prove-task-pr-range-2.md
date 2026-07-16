# W2-138 PR2 — Reconcile #924, then prove the range end-to-end

## Reconciliation of merged PR #924 against the done-when

The done-when (from the PR1 design, `scratch/prove-task-pr-range.md`, now
cleared by land) has three build gaps plus a proof obligation. Status after
#924:

| Deliverable | Where | Merged in #924? |
|---|---|---|
| Gap A — `verify_task_pr_range` pre-publication parity proof (M==B, M<B refuse, B<M heal, divergent refuse) | `ops/task.rs`, gated in `ops/land.rs::prepare_pr` before `prepare_land` | ✅ |
| Gap B — placement guard: `resolve_upstream_base` origin-anchor + no-remote fallback, `refuse_if_canonical_ahead` | `ops/task.rs::task_run` | ✅ |
| Gap C — keep `request_task_pr_publication` as intent writer, gate runs earlier | `ops/land.rs` | ✅ |
| `heal_task_pr_base` dedicated store write | `store/{child_sessions,sqlite/child_sessions}.rs` | ✅ |
| **Unit proof** — 10 inline tests over all verdicts + placement | `ops/task.rs` tests mod | ✅ |
| **Integration proof** — bare-origin + real `submit`/`land` path asserting refusal happens **before any push / `gh pr`**, and that after healing the three views agree | design §"Proof tests (integration, `rust/loopflow/tests/`)" | ❌ **missing** |
| **Dogfood field proof** — a real serial Task PR whose GitHub range, `lf task changes`, and `base_commit` agree | this Task's PR2 | ❌ **this PR** |

The one gap #924 left: the design explicitly scoped integration tests in
`rust/loopflow/tests/` that drive the *real* publication path and assert the
observable acceptance property — **no push / no `gh pr` before refusal**, and
minimal aligned range after healing. #924 proved the verdict *logic* by calling
`verify_task_pr_range_with_authority` directly; it never proved the gate fires
before the first GitHub side effect through `submit`/`land`. That end-to-end
proof is the content of PR2.

## What PR2 builds

`rust/loopflow/tests/task_pr_range_tests.rs` — two integration tests over the
bare-origin fixture, driving the actual `submit`/`land` ops:

1. **`submit_refuses_a_contaminated_range_before_any_push`** — the #877/#882
   acceptance case. Recorded base carries a foreign unpushed commit (M<B).
   `submit` returns Err naming the foreign commit + `rebase --onto` recovery,
   **and the branch never reaches the remote and no `gh pr create` is issued** —
   proving refusal precedes the first side effect.

2. **`serial_pr_heals_stale_base_and_aligns_the_three_views`** — the serial /
   dogfood shape. A continuation PR's recorded base is behind current
   `origin/main` (a sibling landed). `land` rebases, heals `base_commit → M`, and
   publishes. Asserts the healed base equals the current origin tip and that the
   three views agree: `base_commit..HEAD` is exactly the one Task commit
   (`lf task changes` == GitHub range == recorded base), with the already-merged
   upstream commit dropped.

## Dogfood proof (completion gate)

PR2 is itself a serial Task PR (`sequence: 2`, base recorded behind current
origin). Publishing it runs `verify_task_pr_range` on PR2's own range — the
machinery proving itself. Completion requires, on the published PR2:

- GitHub range == `git rev-list <base>..HEAD` == `lf task changes W2-138`, and
- the recorded `base_commit` == `merge-base(origin/main, HEAD)` after healing.

All three aligned, zero manual commit dropping.
