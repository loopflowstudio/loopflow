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

## File-only memory curation blocker

`wave/intelligence/MEMORY.md` still describes the deleted live-memory stream,
write routes, and `lf memory add|update` commands as current architecture. This
repair leaves it untouched because the operating contract says Wave memory is
server-owned and explicitly forbids a direct file edit.

There is no compliant write path left: `lf memory --help` exposes only `show`,
and the Wave server has no memory route. The `update-wave` skill still instructs
an agent to edit `MEMORY.md` through the ordinary repository workflow, which is
the direct write this repair is not authorized to perform. Either restore an
owned curation command or explicitly make reviewed repository edits the
authority; then curate the stale Intelligence memory through that chosen path.

## PR #1052 carry-forward

The closed `make-task-gate-approval-the` branch should not be revived: its
Session/Review authority model is obsolete. Preserve these independent safety
invariants in the current model:

- pin GitHub auto-merge to the exact prepared head SHA;
- refuse landing while an explicitly authored Feedback checkpoint is open;
- settle Task completion from durable merged-PR evidence without requiring a
  live executor or surviving worktree;
- make mutation guards agree with the projected legal action.

Do not preserve its blanket ban on managed-Task `submit` or recreate approval
state. `submit` is an explicitly chosen User merge gate; bare `land` means
continue the Task, and `land -c` means complete it.
