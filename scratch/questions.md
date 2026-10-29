# LOO-684 questions and assumptions

- Assumed (kickoff, 2026-10-29): when a Task opening hands off to another
  repository window, the handing-off window sets `taskOpening = nil` rather than
  `.usable`/`.failed`. It owns no opening anymore; `DesktopOpening`'s wire
  statuses stay unchanged. Reversible.
- Unresolved: which of the two silent paths Jack hit is unobservable until
  LOO-683 lands Desktop logging; the plan fixes both.
