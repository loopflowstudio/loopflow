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

- "now when i click a wave my scroll bar stays, but thats weird because i want to
  go to Prouct, whichis now at the top" → the scope's scroll-to-top ran in the
  same update that swapped the rows and was lost; it now runs one turn later.
- "I also think Waves needs a permanent, expandable section in the left par[t] …
  so that there is scrolling with the working tasks … Starts Rather small, but
  like 3 waves can show in that region" → **Built, awaiting Jack's look:** a WAVES
  section pinned under the Task list with its own scroll, about three rows tall;
  its chevron expands it to every Wave (state on `WorkNavigation.sidebarWavesExpanded`).
  Proposals made in building it, not yet confirmed by Jack: it sits at the bottom;
  it always lists the repository's Waves, the selected one highlighted, so a
  Wave's label no longer slides into the header (Dashboards still do); the
  in-list SUBWAVES and Dashboard Wave-member rows are gone from the Task list.

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
- 18:06 retry (load ~12, `trace4/`): no hitches, one 415 ms microhang at 2:26 and
  no recorded SwiftUI updates, so Jack likely did not click during it. Still owed.
- Traces: scratchpad `trace2/` (before), `trace3/` and `trace4/` (after), not retained in the repo.

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
