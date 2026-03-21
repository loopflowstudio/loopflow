---
asana_id: '1213718096104955'
linear_id: c41ad0f6-255a-42b6-8aae-43a49ce99263
notion_id: 32af8f99-3d81-81c8-a089-d47185436add
---
# 05: Window Composition — Polish

**Finish line:** The multiplexer panes are rich enough to stay open. Markdown has a file picker, diff shows unified hunks, splitting offers a type choice, and directional focus works across the whole layout.

## Context

The multiplexer core shipped: recursive binary split tree, per-wave layout persistence (`MultiplexerStore`), tmux session management (`TmuxSession`), Ghostty terminal embedding, and first-party workspace panes for roadmap, README, runs, launcher, markdown, diff, terminal, and launchpad. Focus-aware keyboard routing dispatches splits and closes to SwiftUI or tmux based on whether a `GhosttyMetalView` is the first responder. All of this is in `MultiplexerView`, `MultiplexerLayout`, `MultiplexerStore`, `TmuxSession`, `KeyboardRouter`, `ShortcutAction`, and `ContentView`.

Current pane state:
- **Terminal:** Fully functional — Ghostty embedding, tmux-backed splits, session lifecycle
- **Roadmap / README / Runs / Launcher:** Shipped and useful, but still simple projections over existing stores rather than deeply interactive tools
- **Markdown:** Shows file contents with fallback search (`scratch/`, `wave/`, `README.md`). Plain `Typography.code()` rendering, no file picker, no file watching
- **Diff:** Shows `git diff --stat main...HEAD`. No per-file unified diff, no file list sidebar
- **Launchpad:** Cursor, Finder, PR buttons. Missing Codex and OpenCode

Current shortcuts: `Ctrl+Shift+5` split vertical, `Ctrl+Shift+'` split horizontal, `Cmd+W` close pane, `Cmd+Shift+Return` new shell, `Cmd+Option+←/→` focus next/previous pane.

## What to build

### 1. Pane type picker

When splitting, a popover lets you choose the new pane type. Currently splits auto-cycle through pane types (terminal splits default to launchpad; non-terminal splits cycle through roadmap → runs → readme → launcher → diff → launchpad).

- Small popover at the split point instead of auto-cycling
- Options: Terminal (grayed if one exists), Markdown, Diff, Launchpad
- Default: terminal if none exists, otherwise markdown

### 2. Markdown file picker

Add a sidebar or top bar to the markdown pane for browsing files.

- Tree of markdown files in `wave/<wave-id>/` and `scratch/`
- Click to open. Current file highlighted
- FSEvents watcher to re-render on external save

### 3. Unified diff viewer

Replace `--stat` with a proper diff display.

- File list sidebar (from `git diff --name-only main...HEAD`)
- Click file to see unified diff with syntax highlighting
- Truncate large hunks (>500 lines) with "show more"
- Refresh on focus or manual trigger

### 4. Directional focus

Replace next/previous with spatial navigation.

- `Cmd+←/→/↑/↓` for directional focus across outer panes
- When terminal is focused, directional focus routes to `tmux select-pane`
- Requires knowing which pane is spatially adjacent (the layout tree has this info)

### 5. Resize and zoom

- `Cmd+Option+←/→/↑/↓` resize the split boundary
- `Cmd+Shift+Z` zoom focused pane (toggle full-size)
- When terminal is focused, resize/zoom routes to tmux

### 6. Snap hotkeys and named layouts

- Add `snapHalf`, `snapThird`, `snapQuarter` actions to `ShortcutAction`
- `Cmd+1/2/3/4` for quarter/third/half/full on focused outer pane
- Named layouts per workflow: "build" (terminal + markdown), "review" (terminal + diff), "full" (terminal only)
- Layout presets stored per wave alongside the current split tree

### 7. Visual polish

- Drag-to-resize split boundaries
- Focus ring that clearly shows which layer is active (outer pane border vs terminal focus)
- Cross-pane interaction: click a file path in terminal → open in markdown viewer

### 8. Layout persistence safety

- Version `MultiplexerLayout` persistence so future schema changes can migrate cleanly
- Add a reset path for invalid or stale layouts instead of stranding a wave in a broken workspace

### 9. IME input source validation

The direct-key typing path bypasses `interpretKeyEvents` for ordinary printable input but defers to AppKit text input when `selectedKeyboardInputSource` contains `inputmethod` or when Option is held. Validate with Japanese (Kotoeri), Korean, Chinese (Pinyin), and third-party input methods (e.g. RIME, Google Japanese Input) to confirm composition still starts correctly.

## Done when

- Splitting offers a pane type choice
- Markdown pane has a file picker and watches for changes
- Diff pane shows unified hunks per file
- Directional focus works across outer and terminal panes
- At least one named layout preset works
