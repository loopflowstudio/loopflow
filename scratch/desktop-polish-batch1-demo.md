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
| 6 | "Cmd` is supposed to cycle through repos/windows" | Terminal returns ⌘`/⌘⇧` to AppKit (f36018ec0) | **Not in the installed dev build** (built from e8bf969ef); Jack asked for no further dev app launches |
| 7 | Codex copies with Ctrl-C, not ⌘C (also in cmux) | Split → LOO-677 | — |

Automated proof: `swift test --filter "DashboardWorkspaceTests|WorkNavigationTests|DesktopPolishTests|WaveStandupTests|WorkSelectionTests"` → 72 passed.
Item 4's test reproduces Jack's exact banner text without the fix.

## Observed behavior and feedback

Run 2026-10-28 on Loopflow Dev built from b1f29930c, after `lf install` moved the
machine to v0.15.0 (the first launch failed: the shared store was behind the branch's
release frontier).

- Jack Heart: Infrastructure "still shows 66 tasks. That seems wrong. Are you still
  including the backlog now?" Yes — the count was every non-terminal Task, backlog included.
  Jack: "I just want it working and autonomous basically." Agreed change: Wave rows count
  the board's Working + Autonomous columns (idle/active/moving + shipping). Infrastructure
  breakdown at the time: 4 active, 3 moving, 3 shipping, 4 deferred, 103 unstarted → 10.
- Jack Heart: "The rest works. I think once you fix the task count you can ship this."
  Accepted: items 2–6 and the hidden Tasks strip.

## Known limits (interpretation, not agreed)

- Reorder still works only in the repository's Working Tasks list; Wave-scoped lists
  don't drag (one shared repository order exists).
- Autonomous rows come from the roadmap, not Rust's Working Tasks selection, which
  still excludes shipping (`rust/loopflow/src/work/selection.rs:383`). ⌘N skips them.

## Next action

Count fixed (`WorkModel.startedTaskCount`, test `waveRowCountsStartedTasks`); land #1762
per Jack's approval. LOO-673 stays open as the standing bucket.
