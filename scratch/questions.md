# LOO-418 design review

2026-10-07 · accepted decisions and remaining proposals.

- Accepted by Jack Heart: `PR merges → file follow-ups or record “none needed”
  → Task completes`. Every merged Task gets this check, including manual GitHub
  merges. Without a finishing Flow/operator, the Task remains visibly pending.
- Accepted by Jack Heart: end means completed; restore `lf task complete ISSUE`
  as an alias for `lf task move ISSUE end`, with one operation and identical
  checks. Follow-through calls it; Task operator guidance continues past merge
  until completion or a concrete blocker. No separate completion state.
- Accepted approach: `ship` uses `land --wait`, then follow-through.
  Bare land returns immediately and exposes pending follow-through until an
  existing operator finishes it. No new watcher; without an installed schedule,
  recovery waits for the next operation. Polling limits and filing CLI remain
  proposals.
- Proposed: remove redundant `-c` alongside `--next`; no command keeps the
  source Task open to await production evidence.
- Proposed: the owning Wave surfaces due follow-ups on its next pass; filing
  alone does not enable a timer or authorize an arbitrary future run. An explicit
  unattended check in the brief can be run by that pass. Exact-time wakeups are
  outside this slice. Jack's judgment on this return path remains necessary.
- Reversible assumption: `--design PATH` hands a prepared child-specific plan
  into checkout. Keep current scratch isolation and stack sync. No automatic
  interpretation of a multi-step Markdown plan or overwriting newer child notes.
- LOO-385 overlaps the no-PR/Workflow work. Its inspected planning is unstarted;
  no cancellation, transfer or claim of its acceptance has been made.

Jack Heart requested continued advancement. The first implementation milestone
is the sibling-checkout Flow launch/status repair and exact `task complete`
alias, through demo. The full plan remains in `scratch/make-a-task-up-to.md`;
later lifecycle work and its open choices remain. No landing is authorized.
