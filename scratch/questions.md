# LOO-418 implementation assumptions

Reconciled 2026-10-08. Jack Heart required the full lifecycle on PR #1499 before
demo on October 7. The launch/alias-only milestone was the agent's scope reduction. Accepted decisions live in [the design](make-a-task-up-to.md).

Jack Heart revised completion on October 8: end triggers `task complete`; it does
not define Task status. Linear completion changes status without moving Workflow
or stopping Processes. This supersedes the earlier alias. Implementation must
preserve durable failed-trigger retries, newer reopen decisions, and unresolved
delivery facts. The prior gate verifies the earlier model, not this revision.

Implemented defaults below remain unreviewed by Jack:

- Waited landing polls every 15 seconds for 30 minutes; the real waiting loop
  now has timeout-expiry proof. Retry and interruption have focused evidence.
- Redundant `-c` and serial `--next` are removed with the PR-chain implementation.
- Surface due follow-ups on the owning Wave's next operation. Filing installs no
  timer; an unattended check must already be authorized in its brief. Exact-time
  wakeups are outside scope. Present this behavior in the complete demo.
- `--design PATH` transfers the child-specific plan without replacing newer
  child work. Preserve scratch isolation and stacked sync.
- LOO-385 overlaps. Recheck before any parallel implementation or external
  disposition; no closure or transfer is authorized by this work.

The original four slices and landing corrections are present; the independent
completion/trigger revision remains to build. Earlier checks pass after repairs. Unified lifecycle proof, LOO-406 integration and
the complete demo remain. No landing or installed-Home
migration is authorized.

Implementation choice: `task follow-up --key` identifies an obligation across retries; the default is `follow-up`, and additional obligations use distinct keys. The receipt pins the initial Team/state as well as Project and content.

Wait interruption uses the existing global exit 130 handler (a stopped Flow, without retry); timeout remains held exit 3. The public CLI test exposed a conflict with a second Tokio Ctrl-C handler, so that duplicate handler was removed.

Review corrections (2026-10-08, findings from `cdde9f6a2`):

- Jack Heart asked how automatic CI repair interacts with waited landing, then
  confirmed it should repair its own PR with Desktop closed.
  The initial wait disabled repairs and the existing guard rejected the live
  ship driver. Waited landing now shares repair admission, exempting only its
  recorded waiting Flow/Task controllers while retaining other-work exclusions.
  A watcher reports busy work without marking delivery failed. The combined
  headless Task/Flow/wait/repair/merge/follow-through fixture passes.
- Jack asked whether already-merged `land` should succeed with a clear message.
  Previously only the waited path checked before preparation. Jack approved the
  correction: bare land shares that success path, confirms
  the same PR, preserves pending follow-through, and performs no publication,
  sync, scratch cleanup or new merge request. Keep failed/unknown observation
  distinct from verified merge. Pending/completed Tasks, retained scratch and
  explicit checkout selection have passing public CLI coverage.

Jack requested retry from the failed Flow step; filed Infrastructure LOO-435 on
October 8, linked to LOO-418/PR #1499. This branch only corrects propagation of
held exit 3, which the Flow previously turned into failure and retried. Bare land
waiting by default remains an unaccepted suggestion. Historical finite landing
was for continuation after caller exit (#1382), then watcher-owned repair
(#1384); the inspected record does not establish a particular crash as the cause.

Jack Heart (October 8) requested no duplicate Task data types and explicit
`--wait-and-fix` naming. The design records the LOO-406 integration gap: shared
optional placement/identity and local follow-up filing/completion, not DTO merging
alone. Existing LOO-406 types are reused; its active checkout is unchanged.
