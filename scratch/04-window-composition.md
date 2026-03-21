# 04: Window Composition — Polish

## Problem

The multiplexer shipped with the right structure (binary split tree, per-wave persistence, 8 pane types, tmux-backed terminals) but the interaction layer is still primitive. Splitting auto-cycles through pane types instead of letting you choose. The markdown pane reads one file with no way to browse or watch for changes. The diff pane shows `--stat` output when you need unified hunks. Focus cycles sequentially instead of moving spatially. There are no layout presets for common workflows.

These gaps mean the workspace isn't sticky — you split, get the wrong pane, close it, split again. You open diff, see stat output, switch to a terminal and run `git diff` manually. The workspace should be the thing you stay in, not the thing you work around.

This advances wave goals:
- **"Percentage of coding sessions inside Concerto vs external terminal (>70%)"** — a useful diff viewer and markdown browser eliminate two common reasons to leave
- **"Clicks from 'I see a problem' to 'I'm acting on it' (<=2)"** — directional focus and layout presets reduce navigation friction
- **"Every human checkpoint surfaces as an AttentionItem"** — richer pane content means checkpoints have better context when you're already looking at the right files

## Approach

Five coordinated changes that transform the multiplexer from "you can split" to "you can compose a workspace."

### 1. Pane type picker (replace auto-cycling)

When a split shortcut fires (`Ctrl+Shift+5` or `Ctrl+Shift+'`), show a small floating menu anchored at the focused pane instead of auto-picking via `splitPaneType(for:)`.

**Menu items:** Terminal (disabled if one exists in the layout), Markdown, Diff, Launchpad, Roadmap, Runs, Launcher. Each with SF Symbol icon. Default selection: terminal if none exists, otherwise markdown.

**Interaction:** Menu appears on split shortcut → user clicks type or presses number key (1-7) → split happens with chosen type. Escape cancels. The menu is a SwiftUI `popover` modifier on `PaneContainerView`, triggered by state in `MultiplexerView`.

**Implementation:** Add `@State var showPaneTypePicker: String?` (pane ID) and `@State var pendingSplitAxis: SplitAxis?` to `MultiplexerView`. `ContentView.handleMultiplexerSplit` sets these instead of calling `splitPane` directly. `PaneContainerView` shows the popover when its pane ID matches. On selection, `MultiplexerStore.splitPane` fires with the chosen type.

### 2. Markdown file picker and file watching

Replace the static `MarkdownPaneView` with one that browses and watches files.

**Top bar:** Horizontal strip showing the current file path as a clickable dropdown. Dropdown lists all `.md` files found in `scratch/`, `wave/<wave-id>/`, and the repo root (non-recursive). Files grouped by directory, sorted alphabetically within groups. Current file highlighted.

**File enumeration:** `FileManager.default.contentsOfDirectory` with `.md` filter. Called on appear and when the watcher fires.

**File watching:** `DispatchSource.makeFileSystemObjectSource` watching the parent directories (`scratch/`, `wave/<wave-id>/`). On event, re-enumerate files and re-read current file content. The source is owned by the view's lifecycle (`task` cancellation tears it down).

**Config integration:** Selected file stored in `pane.config.filePath`. Changing files calls `MultiplexerStore.updatePaneConfig` so the choice persists across app restarts.

### 3. Unified diff viewer

Replace `--stat` with a file list + unified diff display. Reuse `DiffLinesView` for rendering.

**Layout:** Horizontal split — file list sidebar (200pt, resizable) on the left, unified diff content on the right.

**File list:** Run `git diff --name-only main...HEAD` (or `main...<branch>`). Display as a flat list with file icons and change indicators (+/-/~). Click to select. First file auto-selected on load.

**Diff content:** Run `git diff --no-color main...HEAD -- <selected-file>`. Parse into `DiffLine` array using existing `DiffLinesView` parser. Render with existing syntax coloring (green additions, red deletions, gray context). For hunks > 500 lines, show first 100 lines + "Show N more lines" button that expands in-place.

**Refresh:** Re-run on `scenePhase` becoming `.active` (covers window focus), or on manual pull-to-refresh / Cmd+R shortcut. Show a subtle "Refreshing..." indicator during async load.

**Stat summary:** Show the `--stat` summary line at the bottom of the file list (e.g., "12 files changed, 450 insertions(+), 120 deletions(-)").

### 4. Directional focus

Replace sequential next/previous with spatial `Cmd+Arrow` navigation.

**New shortcut actions:** `focusLeft`, `focusRight`, `focusUp`, `focusDown` mapped to `Cmd+←/→/↑/↓`.

**Tree walk algorithm:** Given a focused pane and a direction, walk the binary tree to find the spatially adjacent pane:
- `focusLeft` from pane P: find the nearest ancestor split with `.horizontal` axis where P is in the `second` subtree. The target is the rightmost leaf of `first`.
- `focusRight`: opposite — find horizontal split where P is in `first`, target is leftmost leaf of `second`.
- `focusUp`/`focusDown`: same logic with `.vertical` axis.
- If no adjacent pane exists in the given direction, wrap to the opposite edge (same behavior as current next/previous cycling).

Add `func adjacentPane(from paneId: String, direction: Direction) -> PaneState?` to `LayoutNode`. This is a pure tree traversal with no layout geometry needed — the binary tree encodes spatial relationships directly.

**Terminal routing:** When the focused pane is a terminal, directional focus routes to `tmux select-pane -L/-R/-U/-D` via `TmuxSession`. If tmux reports there's no pane in that direction (exit code 1), fall through to the outer multiplexer's directional focus. This lets you navigate within tmux splits first, then escape to outer panes.

**Backward compatibility:** Keep `focusNextPane` (Cmd+Option+Right) and `focusPreviousPane` (Cmd+Option+Left) as sequential alternatives.

### 5. Named layout presets

Three preset layouts applied via Cmd+Shift+1/2/3.

| Preset | Name | Layout |
|--------|------|--------|
| Cmd+Shift+1 | Build | terminal (right, 0.58) + markdown (left) |
| Cmd+Shift+2 | Review | terminal (right, 0.58) + diff (left) |
| Cmd+Shift+3 | Full | terminal only |

**Implementation:** Add `LayoutPreset` enum to `MultiplexerLayout.swift` with a `func layout(for wave: WaveViewModel) -> LayoutNode` method that generates the tree. Add `applyPresetBuild`, `applyPresetReview`, `applyPresetFull` to `ShortcutAction`. In `MultiplexerStore`, `applyPreset` kills tmux sessions for removed terminal panes, creates sessions for new ones, and persists the result.

**Preservation:** Applying a preset replaces the current layout entirely. The previous layout is not saved — presets are starting points you customize with splits, not states you toggle between.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Type picker as command palette filter | Reuses Cmd+K infrastructure | Too heavyweight for a 7-item choice. A popover is faster and more discoverable at the split point. |
| Markdown pane with full tree sidebar | Rich file navigation | Eats horizontal space in a pane that's already width-constrained. A top-bar dropdown is compact and sufficient for the typical 5-15 markdown files. |
| Diff via libgit2 / swift-git bindings | Structured diff data, no shell calls | Adds a native dependency for something `git diff` already does well. Shell calls are fine for a refresh-on-focus viewer. |
| Directional focus via geometry (hit testing) | Works with arbitrary layouts | Overkill — the binary tree already encodes spatial adjacency perfectly. Geometry-based navigation would only matter with drag-to-rearrange, which isn't in scope. |
| Layout presets stored per wave alongside split tree | Restorable named states | Complexity without clear value. Presets are fast starting points. If you want to preserve a custom layout, it already persists automatically via `MultiplexerStore`. |
| Resize + zoom (item 5 from wave spec) | More complete window management | Significant interaction design for a feature that's less urgent than the core five. Resize requires drag handles, zoom requires toggle state. Defer to a follow-up. |

## Key decisions

**Popover, not command palette.** The pane picker appears at the split point as a small popover, not as a command palette filter. Seven items don't need search. Spatial proximity (the popover is where the split will happen) makes the choice feel direct.

**Top-bar dropdown, not sidebar tree.** The markdown file browser is a horizontal strip with a dropdown, not a persistent sidebar. The typical wave has 5-15 markdown files. A sidebar would consume 150-200px permanently for a list you glance at once then ignore.

**Shell `git diff`, not libgit2.** The diff viewer runs `git diff` as a subprocess. It's refresh-on-focus (not live-streaming), so subprocess overhead is irrelevant. This avoids a native dependency and works identically to what the user sees in their terminal.

**Tree walk for directional focus, not geometry.** The binary split tree already encodes "left of" and "above" relationships. Walking the tree is O(depth) and always correct. Hit-testing against rendered geometry would require layout measurement and break in edge cases.

**Three presets, hardcoded.** Build, Review, Full. Not configurable (yet). These cover the three most common workflows. If a fourth becomes obviously needed, add it then.

**Defer resize, zoom, drag-to-resize, cross-pane interaction, IME validation.** Items 5, 7, 9 from the wave spec are real but secondary. Landing the core five in one coherent PR is better than spreading thin across nine features.

## Scope

- **In scope:** Pane type picker, markdown file picker + watching, unified diff viewer, directional focus, three named layout presets, tests for layout tree traversal and preset generation
- **Out of scope:** Resize/zoom (item 5), drag-to-resize split boundaries (item 7), cross-pane interaction (item 7), layout persistence versioning (item 8 — already low-risk with Codable), IME validation (item 9 — manual testing, not code), snap hotkeys beyond the three presets (item 6 partial)

## Done when

- Split shortcut shows a pane type popover; choosing a type creates the split
- Markdown pane has a top-bar file dropdown and re-reads on external file changes
- Diff pane shows file list + unified hunks with syntax coloring; hunks > 500 lines are truncated
- `Cmd+←/→/↑/↓` moves focus directionally across outer panes; terminal focus falls through to tmux
- `Cmd+Shift+1/2/3` applies Build/Review/Full layout presets
- `swift test --package-path swift` passes with new tests for directional traversal and preset layouts
- `xcodebuild test` passes for Concerto UI
