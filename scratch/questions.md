# LOO-418 design review

2026-10-07 · unresolved proposals. Accepted decisions and their attribution live
in [the design](make-a-task-up-to.md).

- Proposed: polling limits and the follow-up filing CLI. Jack Heart accepted
  waited landing plus follow-through and recovery by the next Task/Wave operation.
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

The launch/status repair and `task complete` alias are implemented locally;
Jack's demo review remains. Later lifecycle work retains the open choices above.
No landing is authorized.
