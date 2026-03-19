---
asana_id: '1213718096104955'
linear_id: c41ad0f6-255a-42b6-8aae-43a49ce99263
---
# 06: Window Composition

**Finish line:** Concerto is a native pane compositor where one terminal pane (backed by tmux) lives alongside native markdown, diff, and launchpad panes. One command vocabulary manages both layers.

## Context

The multiplexer milestone (this PR) shipped the core: recursive binary split tree, per-wave layout persistence, tmux session management, Ghostty embedding, and keyboard shortcuts. Shell panes work. Phase 1 is the layout engine plus shell panes.

The pane routing spec establishes the model going forward:

- **One terminal pane per wave**, backed by one tmux session
- tmux owns all inner shell splitting (panes, windows, tabs)
- Concerto owns the outer pane tree (terminal + native panes)
- One semantic command vocabulary dispatches to the right layer based on focus

This item covers the native pane types and the command dispatch system.

## What to build

### 1. Markdown viewer pane

Read-only file viewer for wave and scratch content.

- Scoped to `wave/<wave-id>/` and `scratch/` in the worktree
- File picker sidebar (tree of matching markdown files)
- Renders markdown with syntax highlighting via `Typography.code()` for code blocks
- Watches files for changes (FSEvents), re-renders on save
- No editing, no LSP, no git integration — pure viewer
- Opens from: file picker, or clicking a path reference in another pane (later)

This replaces the old `TerminalContextSidebar` content. Wave identity, current work items, and roadmap state are all viewable in `wave/<id>/README.md` and `scratch/` — no need for a dedicated sidebar chrome.

### 2. Diff viewer pane

Shows the branch diff against main.

- Runs `git diff main...HEAD` in the worktree via `Process`
- File list sidebar, unified diff view with syntax highlighting
- Refreshes on focus or manual trigger
- Read-only — for reviewing what changed, not editing
- Truncates large diffs (>500 lines per file) with "show more" expansion

### 3. Launchpad pane

Quick-launch buttons for external tools.

- Open in Cursor (existing `TerminalLauncher.openInIDE`)
- Open in Codex app
- Open in OpenCode GUI
- Reveal in Finder
- Open GitHub PR (if wave has one)
- Small pane — works well as a narrow sidebar or quarter-screen
- Also shows wave identity: name, branch, status, worktree path

### 4. Pane type picker

When splitting a pane, a picker lets you choose what type the new pane is.

- Appears as a small popover at the split point
- Options: Terminal (only if no terminal pane exists), Markdown, Diff, Launchpad
- Terminal option is grayed out if a terminal pane already exists in the wave's layout
- Default for Cmd-Shift-Enter: always terminal if possible, otherwise markdown

### 5. Semantic command dispatch

One command vocabulary, routed by focus context.

```swift
enum PaneCommand: String, Codable {
    case splitVertical
    case splitHorizontal
    case closeFocus
    case focusLeft, focusRight, focusUp, focusDown
    case resizeLeft, resizeRight, resizeUp, resizeDown
    case zoomFocus
    case newTab
}

func dispatch(_ command: PaneCommand, focus: FocusContext) {
    switch focus {
    case .outerPane(_, let kind) where kind != .terminal:
        outerPaneManager.handle(command)
    case .terminal:
        tmuxBridge.handle(command)
    default:
        outerPaneManager.handle(command)
    }
}
```

Terminal-focused translations:
- `splitVertical` → `tmux split-window -h`
- `splitHorizontal` → `tmux split-window -v`
- `focusLeft/Right/Up/Down` → `tmux select-pane -L/-R/-U/-D`
- `resizeLeft/Right/Up/Down` → `tmux resize-pane -L/-R/-U/-D <step>`
- `zoomFocus` → `tmux resize-pane -Z`
- `newTab` → `tmux new-window`
- `closeFocus` → `tmux kill-pane` or `tmux kill-window`

### 6. Terminal pane invariants

- Exactly one terminal pane per wave
- Terminal pane cannot be duplicated in the outer tree
- Closing the terminal pane either: disallows close, or replaces with a new terminal pane
- tmux session survives Concerto crashes; on relaunch, detect and reattach
- Concerto does not map outer leaves to tmux windows or panes — tmux owns that

### 7. Polish

- Drag-to-resize split boundaries in the outer tree
- Snap hotkeys: Cmd-1/2/3/4 for quarter/third/half/full on focused outer pane
- Named layouts per wave or per workflow (build, review, tend)
- Cross-pane interaction: click file path in terminal → opens in markdown viewer (later: editor)
- Visual focus indicators: which outer pane is active, whether terminal has keyboard focus

## What this replaces

The existing `07-window-composition.md` was written around lfd-owned terminal sessions and multiple terminal panes in the split tree. This rewrite reflects the new model:

- Terminal sessions are local tmux, not lfd-managed
- One terminal pane per wave, not N shell panes
- tmux owns inner splitting, Concerto owns outer splitting
- Command dispatch is focus-aware, not layer-specific

## Implementation order

1. Markdown viewer pane content (file picker + renderer)
2. Diff viewer pane content (git diff display)
3. Launchpad pane content (external tool buttons)
4. Pane type picker on split
5. Semantic command dispatch with tmux bridge
6. Directional focus (left/right/up/down instead of next/previous)
7. Resize and zoom
8. Named layouts and snap hotkeys

## Risks

- **Markdown rendering quality**: SwiftUI doesn't have a built-in markdown renderer with syntax highlighting. May need AttributedString parsing or a lightweight WebView. Start with basic AttributedString, upgrade if needed.
- **Diff parsing**: Raw `git diff` output needs parsing into file-level hunks. Can use `Process` + string parsing, or a library. Start simple.
- **Command dispatch timing**: When terminal has focus, Ghostty captures most key events. Need to intercept pane commands before Ghostty sees them. The existing `performKeyEquivalent` override in `GhosttyMetalView` already handles Cmd+C/V — extend it for pane commands.
- **Focus visual clarity**: If the user can't tell whether they're in outer-pane mode or terminal mode, shared shortcuts will feel random. The focus indicator design is load-bearing.
- **Focus detection drift**: Shortcut routing depends on accurate Ghostty responder detection. If Ghostty's `performKeyEquivalent` behavior changes, terminal-vs-outer dispatch could silently break. Pin focus detection with tests.

## Done when

- Markdown viewer shows wave/scratch content with file picker
- Diff viewer shows branch diff with file list
- Launchpad has working buttons for Cursor, Finder, and PR
- Splitting offers a pane type choice
- Same shortcut splits outer pane or tmux pane depending on focus
- Focus indicators clearly show which layer is active
