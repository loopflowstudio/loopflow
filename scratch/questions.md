# Open questions and assumptions (LOO-383)

Recorded at kickoff, 2026-10-05. Each has a default the plan proceeds on.

1. **Is `task/session` a plain skill or a primary conversation?** Assumed a
   plain builtin skill launched with `lf --task <issue> skill task/session`.
   A primary Task conversation needs `session ensure` and Ctrl-C decisions that
   belong to LOO-364. Jack Heart's brief asks to "repair the intended public
   entry point if missing"; which entry point was intended is not recorded.
2. **Does the session load operate by command or by assembly?** Assumed
   `lf help <scope>/operate` at start and on each return, so an installed fix
   reaches an existing conversation. Assembly would guarantee delivery at launch
   at the cost of new include code and a frozen procedure.
3. **Does "every unfinished Task gets a disposition" fit "soften, dont harden"?**
   Assumed yes: it removes the numeric "one or two moves" and measures covered
   work. Jack's 2026-09-28 correction rejected a cap, not completeness.
4. **May an operator choose the next Flow for a Task whose Flow finished short
   of landing?** Assumed it may start `queue` and report readiness, and may land
   only where the Task's selected Flow or the person already chose landing.
   Whether a Wave operator may run `ship` on a published `pursue` PR without
   asking is Jack's call.
5. **Does `wave/operate`'s Task-brief section move to `capture-tasks`?** Only if
   `capture-tasks` already covers each rule; otherwise it stays in place.
6. **Default New Session skill.** Unchanged (`capture-tasks`, Jack's 2026-10-02
   selection). `task/session` appears in the picker without becoming the default.
7. **Schedules on Jack's Home.** The minute check is disabled and Product's
   declared `wave/operate` cron is not installed. This Task documents that
   honestly and installs nothing.
