# LOO-369 assumptions · 2026-10-02

- Proposed: Show completed includes canceled/duplicate history with explicit
  outcome labels and a clarifying caption; use cancellation timestamps for those
  outcomes, never label cancellation successful. Jack requested history access
  but did not specify a separate cancellation control.
- Proposed: days are rolling 24-hour UTC intervals, inclusive at both ends;
  unknown-date history is available only under All time. Preserve the last valid
  positive day count while input is invalid.
- Proposed: filter choices persist per Wave within the current window, starting
  hidden in a fresh window. Preserve open planning and unresolved execution or
  Session access regardless of local historical abandonment.

These are reversible implementation choices for authored design review, not
additional decisions attributed to Jack.
