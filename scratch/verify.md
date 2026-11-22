# Verify e166ff — PR #2017, merge c4de2038fd

Outcome: **Verified.** `git rev-parse HEAD^{tree} origin/main^{tree}` printed the
same tree (35231eafb2) for the gated head 233315ff95 and main, so the simulator
qa on that head (iPhone SE 3rd gen, light, dark, accessibility-large) is a look
at main's bytes.

On main's tree:
- `grep -rnE '\.font\(\.system\(size|\.font\(\.(title|body|…)|size: *[0-9]' swift/LoopflowiOS` → nothing.
- literal padding/radius grep → only four `spacing: 0` stacks.
- `buttonStyle` → every one `LoopflowButtonStyle`, plus one `.plain` on the Task row; no `ContentUnavailableView` in code.
- `TaskRow(` in TaskListView, `WorkflowLine(` in TaskWorkspaceView, "What do you want?" `.serif(.display)` in MobileNewTaskView, `StateView` in every empty/failed state; `MobileType.swift` gone.
- GhosttySurface: `font-family = \(Typography.monoFamily)`.
- DESIGN.md: ramp table has an iPhone column (34/26/20/17/13, mono 14, terminal 12), 12 pt floor, 44 pt `HitTarget.control`; no iPhone pointer left under "We do these now".
- `swift test --package-path swift --filter 'TypographyDisciplineTests|TaskActionsTests|TaskStatusTests|TaskRowTests|WorkflowLineTests|OwnerStreamTests'` → 40 tests in 6 suites passed.

CI on c4de2038fd: merge_group run 45040331287 success; push `CI` 45041115484
success; push `macOS on main` 45041115414 still in progress (`macos / swift-test`,
`macos / loopflow-ui-test`) when read — not counted as passed.

Not run by anyone: physical iPhone + QR scan, landscape rotation, the Mac's
"Auto-Ship requested" Notice in the running app. Already filed: 3e95bc, d5be39, 8b92c6.
