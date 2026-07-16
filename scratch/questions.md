# Open questions — W2-252 legal Task actions

Assumptions made under headless resume; flag for review on landing.

1. **"Review" spans two review concepts.** The directive names `review` as an
   action and "review gate combination" as a test dimension. There are two
   review things in the codebase: (a) PR review — a human reviews the open
   GitHub PR (`NextMoveOwner::Review`, the `dead_authored_commits` case); (b)
   interaction-review gate — the `lf task review` exercise with
   requested/active/completed + approved/changes_requested. **Assumption:** the
   `Review` action covers both, since both reduce to "a human must review
   something before this Task advances," and the reason string distinguishes
   them. The review-gate test dimension covers (b)'s states. If the reviewer
   wants two distinct actions, the six-action enum grows — but the directive
   fixes the set at six, so folding is the call.

2. **`Attach` moves to `BodyControl`.** Removing `TaskAttentionControl` orphans
   `Attach` (live-terminal attach). **Assumption:** add `Attach` to `BodyControl`
   and emit it for live bodies, since it's a body control. Alternative rejected:
   a separate `live: TaskLiveControls` field. If the reviewer prefers the
   separate field to avoid touching `BodyControl`/`observe()`/its fixtures, the
   six-action core is unaffected — only the live-control home changes.

3. **Share `TaskActionModel`, not full `TaskAttentionSnapshot`, on `lf task
   status`.** "The same DTO drives lf task status text+JSON and shared
   status/roadmap consumers" is read as: the *action* DTO (`TaskActionModel`) is
   identical across all three. **Assumption:** add only `actions: TaskActionModel`
   to `TaskSessionSnapshot`, not the whole `TaskAttentionSnapshot` (process/
   local_progress/observed_at). `TaskSessionSnapshot` stays `Serialize`-only and
   not Swift-mirrored. If the reviewer wants the full attention snapshot on `lf
   task status --json`, that's a larger follow-up (would make
   `TaskSessionSnapshot` a real cross-language DTO with `Deserialize` + fixture +
   Swift mirror).

4. **Predecessor evidence from builders, not `PrSnapshot`.** `PrSnapshot` drops
   `parent_pr_id` today. **Assumption:** build predecessor evidence in
   `snapshot_task_detail`/`task_snapshot` (which hold the durable `TaskPr`), not
   by adding `parent_pr_id` to the wire `PrSnapshot`. If a surface later needs to
   show the stack parent, add it then.

5. **CI with no fresh reading on an open PR → `Review`.** Matches
   `next_move_for_task:1688` (no CI → Review owner). **Assumption:** same for the
   action model. Could instead be `NoAction` ("CI status unknown"), but that
   diverges from the existing next-move semantics.

6. **`NoAction` for terminal/abandoning, not a seventh "Abandon" action.**
   Abandon is `lf task abandon`, not in the directive's six. **Assumption:**
   abandon-intent-set → `NoAction` recommended, all six blocked. The operator
   path to cancel abandon (`lf task resume`) is a `Resume` that the model would
   mark blocked — acceptable since abandon is terminal-bound by intent.
