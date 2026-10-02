# LOO-369 implementation choices · 2026-10-02

Coordinator guidance resolves the history scope: Show completed contains only
successful completions. Canceled/duplicate history stays in the full shared
inventory and Linear, with workspace/Session access and unresolved work preserved.
This guidance is not recorded as a direct new statement from Jack Heart.

Routine choices: rolling 24-hour UTC intervals, inclusive boundaries; unknown-date
successful completions appear only under All time; invalid day input preserves
the last valid filter. Choices persist per Wave within the current window and
start hidden in a fresh window. No cancellation timestamps or second history
control are needed for this slice.

No design question blocks implementation. See the
[current design](keep-current-tasks-visible-and.md) and
[implementation handoff](task-history-handoff.md). Final Desktop judgment remains
at the authored demo review.
