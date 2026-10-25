# LOO-576 demo: sidebar scope (October 25, 2026)

Experience reviewed: the Desktop sidebar following the selected Wave or Dashboard,
in `Loopflow Dev` built from this branch (PR #1669 head plus a main merge) on
Jack Heart's Mac. Design: `scratch/loo-576-show-the-selected-wave-or-dashboards-tasks-in-the-desktop.md`.

## What Jack said

- "go back to Working Tasks, not Sidebar Tasks … in the UX at least … in code
  SidebarTasks is fine." → **Agreed and done:** the View menu and Keyboard
  Shortcuts list say "Working Tasks" / "Working Task 1–9"; Swift keeps
  `selectSidebarTask` / `cycleSidebarTask`.
- "the animation opening a row is fine but going back is slow." Observed from
  a Dashboard back to Working Tasks (screenshot: `loopflow › Working Tasks`).
- "can we test for any extra re-rendering with any of this stuff"

## Measured on real data (Instruments SwiftUI trace, attached to Loopflow Dev)

Jack Heart doubted the fixture harness ("might be better to use the 'real' data"), so
the harness scenarios were reverted and the live app was traced while Jack clicked
Dashboard ↔ ‹ loopflow, Wave ↔ back and plain Task selections. Load average ~240.

- Before: 15 hangs of 270–530 ms plus one of 1.34 s (a back to Working Tasks);
  40+ frame hitches over 50 ms. Bodies took only 5–53 ms per hang, so **re-rendering
  was not the cost.**
- `ForEachChild<Array<(Int, String)>, …>` (the sidebar Task rows) took 1.74 s
  across the hangs. Main-thread samples: `WorkModel.task(id:)` → `visibleTasks`
  46%, `workingTaskWorkID(row:)` 15%. Each row lookup refiltered and copied the
  whole roadmap; `workingTaskWorkID` did that per Working Tasks member. Pre-existing
  on main; back is worst because it inserts every Working Task row at once.
- Fix (this branch): `WorkModel.TaskIndex` caches the repository's Tasks with an
  ID/Work ID/identifier → position map and Waves by ID, rebuilt when the roadmap,
  repository or origin revision changes. 140 focused tests pass.
- After: 0 hitches over 50 ms and three 140–230 ms stalls, plus one 7.15 s hang at
  0:32 (likely attach under load). Unconfirmed whether Jack clicked during this
  recording; a repeat with clicks and Jack's feel are still owed.
- Traces: scratchpad `trace2/` (before) and `trace3/` (after), not retained in the repo.

## Unrelated rough edges seen in the screenshot

- An invalid Working Tasks query (`state:opena asdf`) shows its error twice,
  once in the sidebar and once over the board.
- The Dev build's "working on the copy in ~/.lf/copies/…" notice repeats with it.

## Open

- Filtered ⌘N slots (PR "Open choices") not yet judged.
- Header placement (sidebar header vs titlebar) not yet judged.

## Next

Jack re-tries back in the rebuilt Dev app. If it still feels slow, trace again;
remaining candidates are `DynamicContainerInfo` (row insertion) and the
Working Tasks board remount, then limiting the animation to the scope title.
