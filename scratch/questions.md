# Open questions and assumptions (LOO-383)

Recorded at kickoff, 2026-10-05. Each has a default the plan proceeds on.
Jack Heart resolved 2, 3, 4, 8 and 9 in design review on 2026-10-05; see
"Decisions" in [the design](make-session-and-operate-prompts.md).

1. **Is `task/session` a plain skill or a primary conversation?** Assumed a
   plain builtin skill launched with `lf --task <issue> skill task/session`.
   A primary Task conversation needs `session ensure` and Ctrl-C decisions that
   belong to LOO-364. Jack Heart's brief asks to "repair the intended public
   entry point if missing"; which entry point was intended is not recorded.
2. Resolved: inline. The session's builtin content is composed from its session file and its operate body; no run-time read.
3. Resolved: the rule covers started Tasks only.
4. Resolved: no. A defined Flow with steps left proceeds; a Task whose Flow ended before landing waits on Jack. Letting a Task change its Flow is wanted and undesigned.
5. **Does `wave/operate`'s Task-brief section move to `capture-tasks`?** Only if
   `capture-tasks` already covers each rule; otherwise it stays in place.
8. Resolved: started Tasks only; unstarted work is not started "for now at least."
9. Resolved with 2.
6. **Default New Session skill.** Unchanged (`capture-tasks`, Jack's 2026-10-02
   selection). `task/session` appears in the picker without becoming the default.
7. **Schedules on Jack's Home.** The minute check is disabled and Product's
   declared `wave/operate` cron is not installed. This Task documents that
   honestly and installs nothing.

Still open after review:

- Whether the plain `task/session` skill is the entry point Jack's brief meant
  (question 1); not discussed.
- Where `wave/session`'s persistent-workspace and publication section goes once
  the session file holds only continuity.
- How a Task changes its Flow. Jack wants it; no design exists. Out of this Task.
