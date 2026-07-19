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
