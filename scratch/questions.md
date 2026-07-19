# Open architecture questions

## Deferred to the server follow-up

- Which process is long-lived: Home, Work actor, Wave cadence, or Run?
- Does Feedback end a Run in typed Wait or retain an idle presentation process?
- Which live deltas must the Mac app stream, and which durable projections are
  sufficient after reconnect?
- How does a remote Home receive best-effort nudges while Ready scans preserve
  correctness?
- Can the Wave journal disappear after generic Steer/Turn/Trace conversion?
- Does `lf chat` remain a human alias over generic Work steer/follow?

## Explicitly not answered early

- Project/Task provider route, resume token, abandonment intent, and handoff
  remain until Run/Launch ownership is designed.
- Launch attention and its Swift projection remain until `WorkSnapshot.feedback`
  has a delivery surface.
- Specialized Project/Task controls remain until Home can wake stopped Work.

## PR #1052 carry-forward

The closed `make-task-gate-approval-the` branch should not be revived: its
Session/Review authority model is obsolete. Preserve these independent safety
invariants without reviving that model:

- represent one shipping choice as mode + exact head + disposition, and clear
  all of it before later supported work;
- refuse the shipping choice while an explicitly authored Feedback checkpoint
  is open;
- make mutation guards agree with the projected legal action;
- settle Task completion from durable merged-PR evidence without requiring a
  live executor or surviving worktree.

The first three belong to this redesign. The fourth remains a server-follow-up
requirement: current reconciliation still consults local branch/worktree
evidence before it records a completing merge. GitHub auto-merge is likewise
only fenced at the arming mutation; exact settlement across an external
maintainer push still needs one durable server owner.

Do not preserve its blanket ban on managed-Task `submit` or recreate approval
state. `submit` is an explicitly chosen User merge gate; bare `land` means
continue the Task, and `land -c` means complete it.
