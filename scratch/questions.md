# LOO-903 open choices (taken the simpler way; reversible)

- The board becomes the backlog only; Working/Autonomous/Done segments go. Working and Autonomous stay in the sidebar and tiles. Done has no board view after this. Restore a Done segment if it's missed.
- Available = not claimed by someone else (preview: All 46 · Mine 0 · Available 43 with three others'), so it includes yours.
- Repository scope disables reordering ("Order lives in each Wave") because rank is per Project.
- Start's menu lists described Graphs; undescribed custom Graphs sit under "More ›".
- Multi-row moves and multi-Start loop over single-Task writes in Swift rather than a new Rust batch op.
- Start's menu lists every described Graph, so composition pieces (deliver, pursue, refresh, queue) and optimize, task-design and vsm-operate appear under "Run without stopping" beside auto and incident. The docs example shows only auto and incident. Hide pieces with a catalog flag, or drop their descriptions, if the menu reads as noise.
- Rank sorts the list across Projects; ⌘↑↓ past a row of another Project writes nothing, since rank is per Project. A Wave whose backlog spans Projects may want grouping (LOO-996).
- Undo uses the window's `UndoManager`, registered when a write lands; a reorder undone before the Work update arrives restores from the ranks read at the time.
- The narrowed filter is bracketed in the capture text (`[Mine 1]`); All, the default, is not.

## From the concept review, November 19 (settled the simpler way; say so if wrong)

- Started Tasks nobody holds stay in the backlog list, because `TaskSection` puts them there, and are drawn as started with their own Continue. The other reading: the backlog shows only unstarted Tasks and unowned started work gets its own place.
- New's sheet gains File (create without running) beside Start. File is not the default button, even when the sheet opens from the backlog.
- Desktop's word for a Graph is "workflow", as the sheet, the Task view and the Wave menu already say; "flow" in the new strings and docs changes to match.
- The view part is renamed `board` → `backlog` (`lf desktop capture -p backlog`). `WorkNavigation.Board`, the Wave view's identity, keeps its name; renaming it touches tiles and dashboards.
- Changing the filter or search drops hidden rows from the selection, as Finder and Mail do, instead of keeping them selected out of sight.
- incident reads "never stops" and auto "merges without asking", though both approve Auto-Ship at the end. Left as shown in the Start bar page.
