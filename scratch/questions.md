# LOO-966 open questions

- `lf sync <another open Task>` now stacks (records, retargets, syncs);
  concept review reversed kickoff's "hint only". Reversible: if borrowing a
  sibling's fix without a stack matters, put the hint back.
- Should publish notice a branch that holds another open Task's unmerged head
  (a raw `git merge`) and name `lf sync <Task>`? Plan: no; Git is not a stack
  record.
- Should `lf pr publish` adopt a PR base that names another open Task's branch
  into the record? Would have healed LOO-959 by itself; lets GitHub edits rewrite
  lf's record. Plan: report, don't adopt.
- The brief says LOO-959 "was stacked on LOO-953 from creation". The store and
  process history show it was created unstacked and stacked by hand after a
  steer ("you can stack onto 953 i guess"). Plan treats the hand-stack as the cause.
- A stacked Task running `lf sync <plain branch>` is still refused. Merging it
  would make the stacked sync record that branch's tip as the fork
  (`record_control_task_state`). Docs say it merges without changing the
  stack; either build that (merge without recording) or narrow the doc line.
- `lf sync <Task> --plan` plans against the literal argument and records
  nothing; resolving the Task's branch for the preview is not built.
