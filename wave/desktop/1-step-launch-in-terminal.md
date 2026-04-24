# 01: Step Launch in External Terminal

When a user runs a step or flow from Concerto, open it in an external Ghostty window backed by tmux.

## Done when

Running a step from Concerto opens an external Ghostty window with the agent running. Concerto's sidebar shows the session as "running." Closing the terminal window does not kill the session.

## Already done

- `TerminalApp.defaultExternal = .ghostty` — Ghostty is the default terminal
- `TerminalLauncher.launchGhostty` — launches Ghostty with working directory and command args
- Command palette "Open Terminal" uses `defaultExternal`
- Workspace-level "Open Terminal" / "Open Internally" buttons create tmux sessions and open in external/embedded Ghostty

## Remaining

- Wire flow/step launch to create a tmux session and run the agent command in it. `TerminalLauncher.launchStep()` exists but currently only user-initiated — step execution via `repoState.runWave()` doesn't trigger external terminal launch automatically.
- Open external terminal attached to that tmux session on step start
- Session registry persistence across app restart — `TerminalWorkspaceStore` persists selection/ordering via UserDefaults, but `TmuxSessionRegistry` is in-memory only and calls `killAllSynchronously()` on app quit. Surviving restarts means discovering pre-existing tmux sessions on launch.
- Sidebar detached state detection — `TerminalWorkspaceView` shows session status (pending/attached/running/succeeded/failed/canceled) but doesn't detect whether a tmux session is externally attached, embedded, or detached.

## What exists

- `TmuxSession.swift` — creates/kills tmux sessions
- `TmuxSessionRegistry.swift` — tracks active sessions
- `TerminalLauncher.swift` — opens external terminals (including Ghostty)
- `TerminalWorkspaceView.swift` — terminal tab UI with session list and workspace shell buttons

The infrastructure is there. The gap is making external terminal the default launch path for steps/flows, not just for workspace shell access.
