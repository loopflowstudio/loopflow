# Session owns Runs and a current Run

2026-09-26 · Interactive review · Current participant name unresolved

The participant confirmed that Session owns Runs, added a current Run, accepted
Flow / Invocation naming, and approved proceeding with those changes.
This supersedes the earlier one-Session-per-Run model in the
[handoff](from-loo291/demo-native-workspace.md). The
[current design](data-model-one-table-per.md) and
[review feedback](data-model-review-feedback.md) carry the updated contract.

```text
Session
  id                stable conversation identity
  title, source     retained across Run replacement
  state, feedback   conversation lifecycle
  current_run_id    FK to one of this Session's Runs
  runs              query runs WHERE session_id = this Session

Run
  id                execution attempt identity
  session_id?       FK to Session; null for a Run without a conversation
  invocation_id?    execution membership, distinct from conversation membership
  task_id?, wave_id?
  provider, outcome, usage, exact process evidence
```

Create the initial Run and Session in one transaction. Replacement appends a
Run under the existing Session and changes its current pointer atomically with
any waiting boundary's exact Run binding. Compare the previous current Run so
late writers cannot overwrite a newer attempt. The pointer must reference a
member of that Session. Closed Sessions retain their pointer and history.

Keep old Run outcomes, provider/native identities, receipts and usage intact.
Session name and feedback stay on the same row. Delete `carry_session_name`;
do not replace it with a SQL copy. Desktop panes key on Session ID, while
process control and Flow settlement continue to require the exact Run and
invocation claim. Stable conversation identity grants no new execution authority.

Migration preserves recorded conversational identity and replacement links.
It must not merge unrelated conversations by title, directory or provider.
Map old desktop keys to stable Session IDs once. Prove repeated replacement,
history retention, stale-result rejection and one completion of saved feedback.

## Assumption requiring care during implementation

The participant did not decide Session-wide Task binding across history.
For now retain Run-owned ancestry: display through current Run, bind that Run,
and inherit its attribution on replacement. Earlier Runs keep their own
attribution and usage. This is a working assumption, not participant approval
of that consequence. Resolve any need for Session-wide history reassignment
before implementing it; do not invent a competing Session Task field or alter
invocation membership to satisfy a bind.

## Next useful action

Implement against the amended complete design after this review is completed
and the Flow chooses its next step. Taskless Invocations use the same owner and
driver as Task-owned ones. Naming stays Flow / Invocation. No runtime or real
Home was changed during this review, and this note chooses no navigation edge.
