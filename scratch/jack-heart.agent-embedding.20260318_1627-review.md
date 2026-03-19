# Review: daemon-backed terminal workspaces and wave multiplexer

## What was implemented

- Added daemon-backed `terminal_sessions` storage, types, migrations, events, and HTTP routes so waiting waves can create, attach, start, complete, and cancel terminal sessions through `lfd`.
- Reworked Concerto's wave detail/workspace flow so each wave keeps its own tracked terminal tabs, run history, and attention state instead of sharing a repo-wide terminal surface.
- Added a native outer multiplexer model in Swift (`LayoutNode`, `MultiplexerStore`, `MultiplexerView`) with exactly one terminal pane plus markdown, diff, and launchpad companion panes.
- Collapsed attention kinds to the `interactive` / `algedonic` model across Rust, Python, Swift models, stores, and UI.
- Updated macOS keyboard routing so the same multiplexer shortcuts dispatch either to the outer pane tree or tmux depending on focus.
- Polished app startup for automation by treating UI-test and snapshot launches as test contexts, which skips eager daemon/notification/voice warmup side effects.

## Key choices

- Kept one terminal pane per wave and delegated inner shell splitting/tabbing to tmux instead of building a native terminal multiplexer.
- Persisted multiplexer layout and terminal-tab selection per repo/wave in local state so switching waves feels workspace-like instead of stateless.
- Preserved the existing shortcut surface (`splitVertical`, `splitHorizontal`, `closePane`, `newShellPane`, focus cycling) and made routing context-sensitive rather than renaming the whole command vocabulary.
- Used the collapsed attention model everywhere instead of carrying compatibility layers for older kind names.
- Chose to skip eager app bootstrapping during UI/screenshot automation because deterministic launches matter more than background warmup in those contexts.

## How it fits together

`lfd` now treats interactive waits as terminal-session records with lifecycle events, persistence, and attach/start/complete/cancel APIs. Concerto consumes those records through `RepoState`, keeps per-wave terminal workspaces and multiplexer layouts in local stores, and routes terminal-pane keyboard actions into tmux while native panes stay in SwiftUI.

## Risks and bottlenecks

- tmux is a hard runtime dependency for the outer multiplexer terminal flow; missing tmux degrades the terminal pane to an error state.
- The native companion panes are intentionally simple first-cut viewers (static markdown/diff/launchpad), so richer editing/navigation still lives outside this milestone.
- `xcodebuild test -scheme Concerto` still failed locally on March 19, 2026 because `ConcertoUITests-Runner` was killed before establishing its UI-test connection. The app and non-UI Swift suites completed first; this looks host-automation-specific, but that is still an inference and should be rechecked in CI.
- Shortcut routing depends on accurate focus detection; if Ghostty responder detection changes, terminal-vs-outer dispatch could drift.

## What's not included

- No native inner terminal pane graph beyond tmux.
- No directional native pane navigation/resizing UI beyond the current shortcut set and focus cycling.
- No richer markdown editor, diff interaction model, or pane-type picker; new native panes still follow the fixed markdown → diff → launchpad cycle.
