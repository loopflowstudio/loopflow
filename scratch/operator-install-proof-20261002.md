# Unbound installation and schedule proof

Jack requested shepherding LOO-292 to its next human decision. The coordinating
conversation performed these supported operations outside Task-bound authority
on 2026-10-02, after the Task contribution verified the installed app signature,
artifact hashes, bundled CLI version and readable history.

- `lf install` exited successfully: published v0.12.29 was already installed.
- `lf install schedule` activated login plus weekly installation, Monday 09:00
  local time, through `~/Library/LaunchAgents/com.loopflow.refresh.plist`.
- Repeating `lf install schedule` reported already scheduled; the plist's SHA-256
  was identical before and after the repeat.
- `launchctl print gui/$(id -u)/com.loopflow.refresh` showed the stable installed
  CLI as its program, one run, and last exit code 0. The configured refresh log
  reported that v0.12.29 was already installed.

The initial launchd execution is observed. A later natural login, scheduled
Monday execution, and sleep/wake catch-up are not yet observed. Interactive app
acceptance remains with Jack. Do not infer these results from the successful
initial load or mark the Task complete.

The separate daemon role is retired in published v0.12.29; an old `lfd` command
is not a completeness requirement. The saved managed Flow still requires
restart; that restart remains blocked by the installed CLI's unrelated
foreign-Team planning refresh defect. No Flow review was completed by these
operator actions. See [the corrected demo](demo-published-install-20261002.md).
