# LOO-673 · Desktop polish batch 1 demo

Date: 2026-10-28. Task LOO-673 (standing polish bucket). PR #1762 at e8bf969ef.
Design and intake: `scratch/jack-loo-673-desktop-polish-fix-jacks-running-feedback.md`.
Runbook page: `scratch/demo.html`.

## Experience reviewed

The branch build installed as `~/Applications/Loopflow Dev.app`
(`uv run python scripts/loopflow-dev.py install`), opened on the loopflow repository.

| # | Jack Heart's feedback | Change in #1762 | What to observe |
|---|---|---|---|
| 1 | "Task counts in wave bar should say number of open tasks, not Total" | Wave rows count non-terminal Tasks | A Wave with done Tasks shows a smaller number; hover help says "N open Tasks" |
| 2 | Reorder shows "a green +" instead of a reorder bar; "slow and clunky" | `onDrag` + `DropDelegate` proposing `.move`; accent insertion line; row moves on drop before `lf desktop move` returns | No + badge; a line at the landing edge; the row jumps immediately |
| 3 | Two sections: current list, then Autonomous below, greyed or with a header; Autonomous = the board's column, "stuff with no human blocker left" | Shipping Tasks under an "Autonomous" heading, muted rows, Swift-only from the board's rule | Shipping Tasks appear below Working Tasks; none on Dashboards or Available/Busy filters |
| 4 | "Layout not saved: … CancellationError" appears a lot | Debounced save checks cancellation after its timer | Rapid pane edits (open/close shells, split) never show the banner |
| 5 | Wave view tile hover icons nearly touch the rounded corner | Inset 4pt → `Spacing.sm` (8pt) | Hover the Session part; icons sit clear of the corner |

Automated proof: `swift test --filter "DashboardWorkspaceTests|WorkNavigationTests|DesktopPolishTests|WaveStandupTests|WorkSelectionTests"` → 72 passed.
Item 4's test reproduces Jack's exact banner text without the fix.

## Observed behavior and feedback

Pending: Jack Heart has not yet run the build. Record what he sees per item here.

## Known limits (interpretation, not agreed)

- Reorder still works only in the repository's Working Tasks list; Wave-scoped lists
  don't drag (one shared repository order exists).
- Autonomous rows come from the roadmap, not Rust's Working Tasks selection, which
  still excludes shipping (`rust/loopflow/src/work/selection.rs:383`). ⌘N skips them.

## Next action

Jack runs the five checks on `scratch/demo.html`. Accepted → merge #1762.
Anything wrong → reply here and it becomes batch 1 rework before merge.
