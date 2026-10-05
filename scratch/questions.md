# Open questions and assumptions (LOO-383)

Jack Heart resolved the kickoff questions on 2026-10-05; see "Decisions" in
[the design](make-session-and-operate-prompts.md).

Still open:

- How a Task changes its Flow. Jack wants it; no design exists. Out of this Task.
- Whether an operator should arm a merge that status recommends. The branch
  leaves unarmed merges to the person.

- If PR #1439 lands, no command continues a stopped Flow. Does "a defined Flow
  proceeds without asking" then mean the operator launches fresh work for the
  remaining steps on its own judgment, or does a stopped Flow wait on Jack?

Assumptions the branch proceeds on:

- The default New Session skill stays `capture-tasks` (Jack's 2026-10-02
  selection); `task/session` appears in the picker without becoming the default.
- Nothing is installed on Jack's Home: the minute check is disabled and
  Product's declared `wave/operate` cron is not installed.
- `task/operate` still selects a Flow for a Task that has never had one, since
  invoking it on that Task is the person's selection. It no longer selects a
  further Flow after one finished unlanded.
- LOOPFLOW.md dropped "selected Wave context belongs to that Work" to stay at
  114 lines; the `<lf:wave>` block already scopes that context.
