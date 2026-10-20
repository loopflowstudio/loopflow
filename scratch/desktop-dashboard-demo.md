# Desktop Dashboard demo notes

October 18–19 attempts and findings in full: `f492be7f19:scratch/desktop-dashboard-demo.md`.
Resolved there: the populated-clone failure (superseded by main's #1612 draft-copy
builds), malformed `process_input` JSON (`79bf0d415b`), stale Sessions hiding activity
and the raw unavailable-ID wall (one summary line since October 20), visit-derived
Working Tasks (agreed-ux.md §4). Still open: LOO-426's Start offer needs demo
confirmation; live `desktop state`/control target `com.loopflow.mac`, so a separately
identified preview cannot show CLI↔native round-trip parity. Not yet exercised:
jump-bar menus, Machine control, Start, Dashboard arrangement.

## Demo findings — October 20, 2026 (Loopflow Dev on the store copy, Jack Heart)

1. “if were using burgundy in title, bar text should be white no?” Trail and TaskHeading
   drew dark text on the burgundy titlebar. Fixed: `palette.titlebarText`/
   `titlebarSecondaryText` follow the titlebar scheme; TaskHeading uses the container's
   secondary style, so tiles keep their palette.
2. “why did i get an unstarted task if i clicked on it in main sidebar” / “it doesnt look
   unstarted”. Cause: TaskHeading fell through to Linear's `planning_state`
   (“unstarted”) once durable evidence said started. Fixed: the heading shows
   Started/Unstarted from runtime `started`; Rust `task_began_in` (recorded work or a
   published PR) is now the one definition for both the runtime and query evaluation.
3. Trail “loopflow > hi / LOO-521” over Dashboard “hi” showing “No visible Work” in a
   band. Cause, from code (not reproduced live): entering a Dashboard kept the previous
   Task's `selectedSessionId`, and the breadcrumb derived a Task from it. Fixed: Dashboard
   destinations clear it and head their own trail; a sidebar Task then shows the Task
   (`workingTasksDashboardYieldsToSidebarTask`). Empty states fill the pane.
   Demo should confirm.

Also on this branch: f492be7f19 (draft builds stop handing off to the installed lf; the
same bug is on main — keep it in the PR notes).
