# LOO-418 design review

2026-10-07 · draft mechanisms, not additional decisions attributed to Jack Heart.

- Proposed: `ship` uses `land --wait`, then follow-through, before Workflow end.
  Bare land returns immediately and exposes pending follow-through until an
  existing operator finishes it. This avoids another mandatory review node.
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

The plan is in `scratch/make-a-task-up-to.md`. All implementation remains;
the first showable slice is the sibling-checkout Flow launch/status repair.
