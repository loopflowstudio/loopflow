
## Phone Start racing a server's own start (from ci-fix on a425c8986d)

`session.continue` from the phone while mini's recovery starts the same prepared Task
conversation opens a second held terminal; its `lf session resume` finds the conversation
held elsewhere and ends, and `session.attach` without a terminal ID can hand the phone that
losing terminal (the registry's `session_terminal` takes whichever live one it finds first).
Deduping in `Registry::start` would fix it but breaks `restart_provider`, whose continue
arrives while the stopped provider's terminal is still live. The iOS fixture now attaches
as the app does instead of starting.
