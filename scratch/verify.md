# Verify: Task 8b92c6, PR #2018 on main

Merge SHA `43ee952fc7` (2026-11-22T17:50:53Z). `git diff --stat HEAD origin/main`
prints nothing: this checkout's tree is main's tree, so the checks below ran
against main.

## Claims

- **No font size, radius or padding passed as a number in `swift/LoopflowMac`
  outside `Design/`.** Holds.
  `grep -rnE '\.padding\((\.[a-zA-Z]+, *)?[1-9]|cornerRadius: *[1-9]|\.cornerRadius\([1-9]|spacing: *[1-9]|\.system\(size: *[0-9]' swift/LoopflowMac --include='*.swift' | grep -v '/Design/'`
  prints nothing. `spacing: 0` remains, and three `.frame(width: 10)` gutters
  (`TaskFilesView.swift:268`, `:391`, `TaskWorkView.swift:534`); frame widths
  are outside the claim.
- **DESIGN.md's Desktop pointers are gone.** Two remain under "Two of the same
  thing": New Task's `WorkflowSteps`, and hand-drawn empty and failed states.
  Both belong to Task 923677 ("New Task and every empty or failed state in
  Loopflow for Mac match the design prototype"), which is in deliver. Every
  pointer this change fixed is removed. No follow-up filed: one exists.
- **Existing view tests pass.**
  `scripts/test_desktop.sh --filter 'DesignDisciplineTests|TaskRowStatusTests|NowProjectionTests|BacklogViewTests|TaskBriefTests|RoadmapViewTests|DashboardWorkspaceTests|ThemeWorkflowTests|CreateWaveSheetTests'`
  printed `Test run with 60 tests in 9 suites passed after 9.754 seconds.`

## Post-merge CI for `43ee952fc7`

- `CI` (push, run 45051725157): success.
- `CI` (merge_group, run 45050866066): success.
- `macOS on main` (push, run 45051725148): success. `macos / swift-test` and
  `macos / loopflow-ui-test` succeeded; `macos / macos-process-test` skipped.

## Not checked here

- The Dev app on an unlocked display in light, dark and an imported theme
  (PR "Try it" step 5): no display in a headless run. A person runs
  `uv run python scripts/loopflow-dev.py run`.
- Row status written once in Rust: Task d5be39, unstarted.
