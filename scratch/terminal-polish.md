# LOO-487 / PR #1559 — inherited integration contract

October 15, 2026. Full accepted decisions, rejected alternatives, failed probes,
artifact receipts, native demos and unproved cases are retained at
`d6743d844fbac89693e1e25ad3e9c2a98fe9fe02:scratch/terminal-polish.md`.
This parent is integrated into LOO-488; it selects no additional parent work.
Jack Heart accepted implementation for hands-on review. Demo 5's layout was
accepted, then shipping was held for navigation dismissal and resizing corrections.
The LOO-462 [Workflow-loading hold](workflow-loading.md) survives.

## Owners and accepted behavior

Screen owns terminal colors, selection, blocks and search. Ghostty composes shell
fills before glyph contrast; Swift owns gutter/header and native focus. Delete
Swift cell fills, GhosttyCommandBlockStyle, TerminalCopyNotice and their exclusive
tests. Preserve selection/status/geometry, hover, extraction, bounded reads and
retained surfaces. No second selection mirror or animation loop.

Copy reads bytes and a surface-local receipt under one lock. Successful pasteboard
writes confirm clear-on-copy and show only the header notice; no cell underline.
Replacement selection, reset, reflow, viewport changes, hiding and destruction
invalidate receipts. Incarnation-scoped notices cannot cross windows/replacements.
Find retains a finite 160 ms edge using absolute deadlines and existing scheduling.
Reduce Motion preserves static feedback; hidden panes never replay cues.
Exact Cmd+C/V/F belong to the host; extra modifiers reach the provider. TUI-native
copy and terminal copy remain separate. Shift-drag follows Ghostty/application
configuration, not an unconditional override; [provider evidence](tui-actions.md).

`minimum-contrast = 3` precedes personal defaults; `1` bypasses correction exactly.
Choose the feasible linear-RGB adjustment with least displacement, preserving
alpha/channel order and already-passing colors. Metal uses Display P3 luminance,
GLSL sRGB. Unattainable faint colors use the best endpoint; exact hue/arbitrary
color separation, transparent backdrops/images/postprocessing are not guaranteed.
Source colors, selection/search, graphics exemptions and copied bytes remain.

New Task windows start Session/Files at 61.8/38.2, superseding 70/30 Task overview.
Header menus switch retained Task/PR/Files/Shell/lf-process companion views. Shell
creation is explicit; hidden shells run and drafts survive; closing retires the
hidden shell. Main Session provider switch/reset uses existing owners and retains
history. Remove Workflow chrome/Details; one lifecycle control reveals actions.
Command-T adds/focuses a shell, vertical first then alternating orientation, using
61.8/38.2. All dividers constrain nested children (240pt width/140pt height,
adapted to available space), use screen coordinates and snap within 10pt of
38.2/61.8 without violating minimums. Existing layouts and deliberately closed
panes survive. LOO-416 owns database persistence; its proposed handoff was not
filed because the provider read disagreed. No second persistence implementation.

Navigation starts closed. A generous Session-header location control drops the
same rich WorkNavigator over unchanged panes; no flat palette replacement.
Cmd-K opens/focuses it; Ctrl-Cmd-S pins/unpins the resizable sidebar. Any repo,
Wave, Task or Session selection, including current, closes temporary navigation;
pinned remains. Escape/outside click restore prior focus. Preserve filters,
scroll, Needs you, adjacent Work, drafts and native clients.

## Evidence and remaining work

Parent pins immutable lf7; LOO-488 combines it with the fixed-RGB readback patch
under a new artifact name. No old artifact is overwritten. Relevant Zig tests,
324 offscreen Metal cases (168 classified unattainable), GLSL compilation,
universal framework and focused Swift suites passed on the parent's source.
Those checks do not prove full glyph composition, presented-pixel latency, native
popover geometry, live provider switching/input, failed clipboard writes or calm
appearance under imported themes. Paired native costs and hands-on review remain.
No numeric performance target or sustained-use KR is inferred.

Check: parent `scripts/test_desktop.sh --filter 'DesktopPaneControlTests|TaskFilesTests|PaneResizeTests' -Xswiftc -gnone --jobs 4` PASS (48 reported, three native cases skipped; `/tmp/loo487-files-default-tests.log`); merged-source acceptance belongs to LOO-488 gate.
