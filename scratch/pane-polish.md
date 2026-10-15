# Pane polish

Jack Heart authorized items 1 and 8 of the Rex study on October 15, 2026:
rounded pane islands, visible copy success, and unobstructed terminal search.
Cell shaders, automatic contrast correction, animation/color policy and whole-app
theming belong to separate Tasks. Existing native clients and pane ownership stay.

## What to build

Give retained panes an 8pt radius, 6pt gutters and a modest neutral focus shadow
on the existing palette. Show a short copy confirmation only after a successful
clipboard write. Reserve a search row above the terminal viewport: matches can
never sit behind it, including at narrow widths and after resizing.

## Current system / Delete — do not maintain

`SessionsView` owns pane layout and its existing 6pt divider drag targets;
`GhosttySurfacePool` owns native terminal lifetimes. Replace the divider's line
and square focus outline with island edges; preserve drag targets and identities.
`GhosttyMetalView.copySelection` already handles text and command blocks; keep
that single selection reader and clipboard writer.

Discovery: Loopflow has no terminal search UI. Pinned GhosttyKit exposes
start/end, total and selected-result callbacks, plus search/navigation bindings.
Wire those APIs; do not infer match rectangles or create a second text index.
The search control travels with the retained native view; its host reserves space
outside the viewport. Closing Find restores terminal focus without sending input.

## Remaining acceptance

Native rendering, PTY match highlighting and visual judgment remain optional
demo checks; no live app restart or real-session input occurred. The added native
fixture covers search viewport reservation, resize, focus restoration and surface
identity, but is intentionally skipped by the headless runner.

Review fixes: copy feedback cannot intercept pointer input; closing Find and late
search callbacks cannot take focus from another context; native callbacks copy
values and defer work outside Ghostty's callback; CRLF line counts use UTF-8
because Swift treats CRLF as one Character. Search controls compress at narrow
widths, retaining the field, Close and keyboard next/previous.

Check: `scripts/test_desktop.sh --filter 'TerminalPolishTests|GhosttyShellBlockTests|DesktopPaneControlTests|GhosttyTerminalInputTests' -Xswiftc -gnone` — app/test build passed; 29 passed, 15 optional display diagnostics skipped; native visual/focus demonstration remains with demo/review (`/tmp/pane-polish-tests.log`).
