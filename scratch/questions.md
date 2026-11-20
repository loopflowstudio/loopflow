# LOO-903 open choices (taken the simpler way; reversible)

- The board becomes the backlog only; Working/Autonomous/Done segments go. Working and Autonomous stay in the sidebar and tiles. Done has no board view after this. Restore a Done segment if it's missed.
- Available = not claimed by someone else (preview: All 46 · Mine 0 · Available 43 with three others'), so it includes yours.
- Repository scope disables reordering ("Order lives in each Wave") because rank is per Project.
- Start's menu lists described Graphs; undescribed custom Graphs sit under "More ›".
- Multi-row moves and multi-Start loop over single-Task writes in Swift rather than a new Rust batch op.
- Start's menu lists every described Graph, so composition pieces (deliver, pursue, refresh, queue) and optimize, task-design and vsm-operate appear under "Run without stopping" beside incident. Hide pieces with a catalog flag, or drop their descriptions, if the menu reads as noise.
- Rank sorts the list across Projects; ⌘↑↓ past a row of another Project writes nothing, since rank is per Project. A Wave whose backlog spans Projects may want grouping (LOO-996).
- Undo uses the window's `UndoManager`, registered when a write lands; a reorder undone before the Work update arrives restores from the ranks read at the time.
- The narrowed filter is bracketed in the capture text (`[Mine 1]`); All, the default, is not.

## From the concept review, November 19 (settled the simpler way; say so if wrong)

- Started Tasks nobody holds stay in the backlog list, because `TaskSection` puts them there, and are drawn as started (their action: see the second review below). The other reading: the backlog shows only unstarted Tasks and unowned started work gets its own place.
- New's sheet gains File (create without running) beside Start. File is not the default button, even when the sheet opens from the backlog.
- Desktop's word for a Graph is "workflow", as the sheet, the Task view and the Wave menu already say; "flow" in the new strings and docs changes to match.
- The view part is renamed `board` → `backlog` (`lf desktop capture -p backlog`). `WorkNavigation.Board`, the Wave view's identity, keeps its name; renaming it touches tiles and dashboards.
- Changing the filter or search drops hidden rows from the selection, as Finder and Mail do, instead of keeping them selected out of sight.
- incident reads "never stops" and auto "merges without asking", though both approve Auto-Ship at the end. Left as shown in the Start bar page.

## From the sync onto main, November 19

- Main's #1960 made Claim, Start, Move and Abandon show in the click and report refusals in the window Notice. The backlog's batch actions now loop over those, so the footer says "Claimed 3" and a refused Task posts its own Notice; the "Started 2 of 3 · reason" tally is gone. ⌘Z for Claim is registered in the click, not when the claims land. Reorder still waits for each rank write.
- The backlog's inline "Planning unavailable / Retry" line and the out-of-plan notice (`WavePlanningNotice`) are gone; main sends both to the window Notice.
- Builtin Graphs main added (plan, build, redesign, deep-clean) got one-line `description:`s from their header comments, since every builtin needs one.
- Start's menu after concept review item 4 (pieces under More ›): "Run without stopping" still holds deep-clean, optimize and task-design beside auto and incident. Taken the simpler way: they stay, and the docs example is redrawn to match. The other choice is a first menu of only auto and incident, which needs a way for a Graph to say it is not a first choice.

## From building the concept review's changes, November 19

- File keeps the sheet open until Rust answers, unlike Start, so a refusal can't lose the words. It then clears that backlog's filter and search so the new row shows. It does not navigate to the Wave.
- `runs: task | wave` was added to the catalog entry: nothing else on the wire says vsm-operate is a Wave's Graph. It comes from the builtin's directory, so a repository Graph is always `task`.
- `queue` shows under "Run without stopping" in a repository with no `finish-queue`, since nothing there includes it.
- New's chooser rows lost their step diagram for the menu's line (stops, first sentence); the sheet still draws the chosen workflow's steps.
- The Task view's "Choose workflow" menu reads the same list, so it no longer offers a Wave's Graph or lists pieces first.
- The suite is `BacklogViewTests` after the rename, not `BacklogBoardTests`.

## From the second concept review, November 19 (settled the safer way; say so if wrong)

- Built this way: with a started Task selected alone, ⌘↩ opens it, as the button does. A started Task Rust would refuse to run still opens. Row-menu Open is off for several rows rather than opening the first.

- A started Task nobody holds gets Open, not Continue, in the backlog. Continue from a last stop such as demo reaches `end` and approves Auto-Ship, and the design already says the backlog must not reproduce Continue/Ship. The other reading: keep a one-click Continue in the backlog and warn when it will merge, which needs the run action and its consequence on the wire (`TaskRunControl` carries only a refusal).
- "Run without stopping" still lists six Graphs (auto, deep-clean, incident, optimize, queue, task-design). Whether the first menu should hold only auto and incident is for the demo to show.

## From compress, November 19

- `lf repo people` (GitHub users behind Task owners, cached in `github_people`) has no Desktop reader now that the standup board is gone. Left in place: it is a documented command. Deleting it means a `-- leaving: github_people` marker and removing the served `repo.people` read.
- VoiceOver's Move up/down on a row outside the selection now selects that row first, as the row menu does.

## From the sync onto main, November 20

- Main's #1976 deleted the builtin `auto` Graph and the Autonomous section: auto is a Task setting (`lf task auto on`). Start's "Run without stopping" no longer offers auto, and the sidebar's Autonomous list is gone. The backlog has no Auto control; Start on a Task with auto on runs past its interactive steps. An "Auto" choice beside Start is the open question.
- The standup board stays deleted; main's Auto badge on its rows had nowhere to go, and the Task cards below keep theirs.
