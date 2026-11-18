# LOO-966 open questions

- Should `lf sync <another Task's branch>` on an unstacked Task record the stack
  (and retarget the PR), rather than only hinting? It matches what the person
  did, but syncing onto a sibling to borrow a fix would then restack the PR.
  Plan: hint only.
- Should `lf pr publish` adopt a PR base that names another open Task's branch
  into the record? Would have healed LOO-959 by itself; lets GitHub edits rewrite
  lf's record. Plan: report, don't adopt.
- The brief says LOO-959 "was stacked on LOO-953 from creation". The store and
  process history show it was created unstacked and stacked by hand after a
  steer ("you can stack onto 953 i guess"). Plan treats the hand-stack as the cause.
