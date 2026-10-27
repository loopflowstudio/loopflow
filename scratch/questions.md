# LOO-634 questions

- 2026-10-27: Moving Graphs at a step boundary (Task outcome) needs either a Graph
  handoff/resume design (architecture says a stopped Graph is never resumed) or
  letting a Graph launch its next step on the selected lf instead of its pinned
  `LF_BIN` (relaxes LOO-107). Settled in concept review: the Task's own "pinning stays" plus the
  no-resume rule leave no in-scope mechanism, so Graphs stay listed; choosing which
  constraint to relax belongs to a follow-up Task.
