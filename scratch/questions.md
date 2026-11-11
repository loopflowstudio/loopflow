# LOO-825 choices

The Task delegates these choices; the simpler options in `design.md` are:

- Limit discovery to known local live views; report known remote ownership.
- Open repository/Wave conversations with `lf open`; put layouts and overviews
  under `lf workspace`, including the existing batch selector `--available`.
- Fail unsupported sibling-tab requests with a manual command, never a mode or
  host fallback. Reject agent `--here` rather than nesting terminal attachment.

These are autonomous design choices, not a recorded human review.

Attachment waits up to 30 seconds. A timeout reports failure without cancelling
the destination; a retry checks its live Session before creating another view.

Desktop receipt revisions identify published results, not stored attempts. A
pending opening without a pane still suppresses another launch; focus reports
Not open until its view exists.
