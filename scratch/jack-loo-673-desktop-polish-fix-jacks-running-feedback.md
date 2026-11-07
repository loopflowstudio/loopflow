
## Batch 2 intake (Oct 30)
13. Sidebar drag: dragged row lingers ~0.5s after release before the Task moves. Fixed in-branch: gesture drag, row follows pointer, lands on release.
14. Abandon leaves the left pane in a weird state ("Open Task is outside this list"): abandonTask keeps the abandoned Task selected and never removes its row early.
15. Unassign leaves the Task in the left pane (LOO-312): Working list (selection.rs) ignores ownership while the Wave board moves unassigned Tasks to Backlog. Jack's Desktop unassign of LOO-312 recorded no entry; CLI unassign at 11:55 Oct 30 succeeded.
Relation to LOO-675 (#1773, open, CI red, conflicting): it moves abandon/unassign writes in-process; list semantics and selection are unchanged.
Batch 2 status: 13 fixed (gesture drag). 14 fixed (abandon hides row + moves selection at once; refusal restores row, error in sidebar). 15 fixed at the root per Jack: Rust `TaskSection::of` is the one bucketing (Backlog/Working/Autonomous/Done/Abandoned); selection Working = section Working, board columns and row counts read `RoadmapTask.section`, `lf wave show` groups by it. Replaced RoadmapSection (now/waiting/not_started/later).
Audit leftovers (to split): Task page header shows Linear state vs TaskHeading runtime; awaitsStart/palette "Not started" = no owner; Busy/Available three ways; board/palette sort by rank vs Rust task_orders; terminal = planning OR runtime in TaskWorkView; taskSessions vs WorkProjection session matching; shipping via task.shipping vs runtime.status; NowProjection groups.
Checks: swift sidebar/board/DTO suites 155 pass + abandon test; cargo selection/durable/waves/lifecycle lib tests, dto_fixtures, session_lifecycle work_first pass; clippy clean.

## Batch 3 intake (Nov 6)
16. Wave view Session tile: zoom/hide hover controls nearly touch the rounded corner (Jack Heart, Oct 28, comment 6edb585b). Cause: strip `.padding(.leading, 12).padding(.trailing, 4)` in `WorkspacePanesView.swift`, 4pt inside an 8pt corner radius. Fixed with explore option B (headless choice, see `scratch/questions.md`): `PaneGeometry.inset` (= corner radius, 8pt) now sets strip leading/trailing, the terminal reserve and stripless tab rows; Wave tiles reuse this strip. Still to do: reply on comment 6edb585b with the PR.
Check: `scripts/test_desktop.sh --filter 'TerminalPaneLayoutTests|WorkspacePanesTests'` 7 pass (strip inset asserted light and dark); Jack's eyes on the Dev build remain.
Compress: `cargo test durable::tests` + `swift test --filter 'WorkNavigationTests|MockWaveFixtureTests|TerminalPaneLayoutTests'` 52 pass; gate owns the rest.
Realign (Nov 6): code matches item 16 ("Not running here" tile is a `WorkspacePanesView` pane, strip at `.padding(.horizontal, PaneGeometry.inset)`); PR #1791 open, mergeable, no failing checks. Wave memory `.lf/wave/product/MEMORY.md` is 16051/16000 tokens; left untouched (no memory-curating Graph step), retire ~60 tokens of stale guidance when one runs.
