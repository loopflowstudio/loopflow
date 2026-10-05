# Open questions and assumptions (LOO-383)

Recorded at kickoff, 2026-10-05. Each has a default the plan proceeds on.
Jack Heart resolved 1, 2, 3, 4, 8 and 9 in design review on 2026-10-05; see
"Decisions" in [the design](make-session-and-operate-prompts.md).

1. Resolved: plain skill, launched with `lf --task <issue> skill task/session`. Jack: a primary Task session is "just a smaller wrapper around this that saves that id in a field"; that wrapper stays with LOO-364.
2. Resolved: inline. The session's builtin content is composed from its session file and its operate body; no run-time read.
3. Resolved: the rule covers started Tasks only.
4. Resolved: no. A defined Flow with steps left proceeds; a Task whose Flow ended before landing waits on Jack. Letting a Task change its Flow is wanted and undesigned.
5. Assumed at implementation: `wave/operate`'s Task-brief section stays.
   `capture-tasks` does not cover its description-versus-comment rules.
8. Resolved: started Tasks only; unstarted work is not started "for now at least."
9. Resolved with 2.
6. **Default New Session skill.** Unchanged (`capture-tasks`, Jack's 2026-10-02
   selection). `task/session` appears in the picker without becoming the default.
7. **Schedules on Jack's Home.** The minute check is disabled and Product's
   declared `wave/operate` cron is not installed. This Task documents that
   honestly and installs nothing.

Still open after review:

- How a Task changes its Flow. Jack wants it; no design exists. Out of this Task.

Assumptions made at implementation, 2026-10-05:

- `task/operate` still selects a Flow for a Task that has never had one, since
  invoking it on that Task is the person's selection. It no longer selects a
  further Flow after one finished unlanded.
- The composed session text places the operate procedure after the session's
  workspace section, under a generated heading; the session opens by pointing
  to it.
- LOOPFLOW.md dropped "selected Wave context belongs to that Work" to stay at
  114 lines; the `<lf:wave>` block already scopes that context.
