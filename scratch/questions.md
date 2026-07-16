# Open questions — W2-252 legal Task actions

All six assumptions made during the prior design session are **validated by the
existing Rust implementation**. No changes to the design are needed; these are
recorded for audit.

1. **"Review" spans two review concepts.** ✅ Validated. `ReviewGateState`
   maps from `interaction_reviews` (`ops/task.rs:review_gate_from`), and the
   `Review` action is used for both PR review and interaction-review gate.
   Reason strings distinguish them ("checks passed; awaiting review" vs
   "awaiting review disposition" vs "merged; answer the post-merge review").

2. **`Attach` moves to `BodyControl`.** ✅ Validated. `BodyControl::Attach`
   exists (`child_session.rs:643`) and is emitted in `observe()` for live bodies
   (`child_session.rs:792,806`). Swift `BodyControl` (`WaveWorkMap.swift:87`)
   still needs `attach` added.

3. **Share `TaskActionModel`, not full `TaskAttentionSnapshot`, on `lf task
   status`.** ✅ Validated. `TaskSessionSnapshot` has only
   `actions: TaskActionModel` (`ops/task.rs:144`), stays `Serialize`-only, no
   Swift mirror.

4. **Predecessor evidence from builders, not `PrSnapshot`.** ✅ Validated.
   `task_snapshot` reads `parent_pr_id` from `TaskPr` and looks up the parent
   PR's phase from the store (`ops/task.rs:2459-2466`). `PrSnapshot` is
   unchanged.

5. **CI with no fresh reading on an open PR → `Review`.** ✅ Validated.
   `actions.rs:242`: `Some(CiState::Passing) | None => Review`.

6. **`NoAction` for terminal/abandoning, not a seventh "Abandon" action.** ✅
   Validated. `actions.rs:122-137`: terminal → `NoAction` ("Task is
   completed/abandoned"); `abandon_intent` → `NoAction` ("Task is being
   abandoned"), all others blocked.
