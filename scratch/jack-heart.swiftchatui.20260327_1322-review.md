# Review: streaming transcript state + Ghostty workspace shells

## What was implemented

- Moved transcript-derived session state into `SessionState` so streaming updates maintain grouped tool runs, the latest assistant message ID, and timestamp labels incrementally instead of recomputing them in `WaveSessionView` on every token.
- Added a small assistant-segment cache in `MessageRow` so markdown/code segment parsing only invalidates when streamed content length changes.
- Made Ghostty the default external terminal app and wired local workspace shell actions around one tmux session per wave (`lf-<waveId>-shell`) for both external and embedded terminal surfaces.
- Preserved sidebar wave ordering across server refreshes, preloaded local wave content after wave refreshes, and aligned the README shortcut docs with the removed rename shortcut.

## Key choices

- **Cache derived transcript state at the mutation sites.** `SessionState` now owns transcript grouping, timestamp labels, and transcript indexing so the view layer stays read-only and cheap during streaming.
- **Use one workspace shell identity per wave.** Both "Open Terminal" and "Open Internally" attach to the same tmux session, which keeps external and embedded shells in sync without extra session-discovery UI.
- **Route default terminal launches through `TerminalApp.defaultExternal`.** This avoids a mix of hard-coded `.warp` and `.ghostty` call sites.
- **Only preload local wave content.** Remote targets keep their existing SSH/IDE path and skip local README/roadmap preloads.

## How it fits together

`SessionState` now updates transcript rows and derived transcript metadata together, and `WaveSessionView` simply renders `state.groupedTranscript`, `state.timestampLabels`, and `state.latestAssistantMessageId`. On the workspace side, `TerminalContextSidebar` builds a `WorkspaceShell`, ensures the tmux base session exists, and then either launches Ghostty externally or upserts a matching embedded terminal session.

## Risks and bottlenecks

- The `MessageSegmentCache` invalidates by content length, which is correct for append-only assistant streaming but would be stale if assistant content were replaced in-place with the same length.
- External shell launch depends on Ghostty being installed at `/Applications/Ghostty.app` and supporting the current CLI flags.
- Preloading wave content adds local README/roadmap reads after wave refreshes; it should stay cheap, but large repos could make that work more noticeable.
- Full `xcodebuild test -scheme Concerto` still fails in this environment because `ConcertoUITests-Runner` exits before establishing the UI-test connection.

## What's not included

- No remote Ghostty/tmux workflow.
- No detached shell reattach management beyond the shared per-wave tmux session.
- No Instruments capture in this gate pass.
- No markdown-rendering or conversation-history feature work beyond the streaming hot-path cache.
