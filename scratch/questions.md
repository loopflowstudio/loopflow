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
  (`record_control_task_state`). `docs/ops.md` now says only unstacked Tasks
  merge a plain branch; building "merge without recording" is open.
- `lf sync <Task> --plan` plans against the literal argument and records
  nothing; resolving the Task's branch for the preview is not built.
- QA found that `lf sync main` (detach) keeps the old parent's commits and
  reverts their changes. Restacking now replays from the fork
  (`git::stack_needs_fork_comparison`). But if that parent lands with a merge
  commit and the detached child syncs main without restacking, Git's merge base
  is the parent's tip, and the child's PR deletes the parent's work. Detaching by
  rewriting the child's commits onto main, or by recording a fork that the main
  merge compares from, is open; squash and rebase landings are unaffected.
- A parent landed and deleted within 60 s of lf's last PR read: the child's sync
  and publish fail on `git fetch` of the deleted branch until the cache expires.
  Treating a missing parent ref as "go read GitHub fresh" would close it.
