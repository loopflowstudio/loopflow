# Workspace frame — LOO-608

## Current direction — 2026-10-27

Jack Heart reviewed the extra workspace edge and said, “i just dont think we
need the extra border”. Remove the outer workspace decoration entirely.
Keep both top corners square and both lower corners rounded. Individual pane
focus styling, layout, terminal sizing and input behavior remain unchanged.

The workspace now uses only `PaneGeometry.workspaceShape` for clipping; its
border/shadow overlay is removed. The geometry test retains its Light/Dark ×
split-axis coverage of corner shapes and lower-corner clearance. Assertions
about the removed shadow overlay are deleted. The README matches this direction.

## Superseded implementation

The October 26 brief requested a fading edge. The first implementation used a
two-point shadow-colored stroke at 30% opacity, blurred by three points. Its
Desktop build and four focused geometry cases passed before publication as
PR #1728. The October 27 concept review accepted that implementation for visual
review; Jack Heart's later feedback supersedes its shadow and tuning guidance.

## Remaining work

Update PR #1728 and retain visual review in this Task conversation. No merge
approval has been given.

Check: `scripts/test_desktop.sh --filter TerminalPaneLayoutTests -Xswiftc -gnone` — PASS on the border-free revision: Desktop build and four Light/Dark × split-axis cases.

Review: the workspace now clips without drawing an outer edge; pane outlines
and lower-corner clearance remain unchanged. No new state or input handling.
