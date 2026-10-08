# LOO-418 implementation assumptions

Reconciled 2026-10-08. Jack Heart required the full lifecycle on PR #1499 before
demo on October 7. The launch/alias-only milestone was the agent's scope reduction. Accepted decisions live in [the design](make-a-task-up-to.md).

Implemented defaults below remain unreviewed by Jack:

- Waited landing polls every 15 seconds for 30 minutes; timeout-expiry proof
  remains with gate. Retry and interruption have focused evidence.
- Redundant `-c` and serial `--next` are removed with the PR-chain implementation.
- Surface due follow-ups on the owning Wave's next operation. Filing installs no
  timer; an unattended check must already be authorized in its brief. Exact-time
  wakeups are outside scope. Present this behavior in the complete demo.
- `--design PATH` transfers the child-specific plan without replacing newer
  child work. Preserve scratch isolation and stacked sync.
- LOO-385 overlaps. Recheck before any parallel implementation or external
  disposition; no closure or transfer is authorized by this work.

All four slices are implemented locally; focused checks pass. Gate and the
complete demo remain. No landing or installed-Home migration is authorized.

Implementation choice: `task follow-up --key` identifies an obligation across retries; the default is `follow-up`, and additional obligations use distinct keys. The receipt pins the initial Team/state as well as Project and content.

Wait interruption uses the existing global exit 130 handler (a stopped Flow, without retry); timeout remains held exit 3. The public CLI test exposed a conflict with a second Tokio Ctrl-C handler, so that duplicate handler was removed.
