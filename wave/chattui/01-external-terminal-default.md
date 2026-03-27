# 01: External Terminal as Default

When a user runs a step or flow, Concerto creates a tmux session, starts the agent command in it, and opens the user's preferred external terminal attached to that session.

## Done when

Running a step from Concerto opens an external Ghostty (or configured terminal) window with the agent running. Concerto's sidebar shows the session as "running." Closing the terminal window does not kill the session.

## What to build

- Wire flow/step launch to create a tmux session and run the agent command
- Open external terminal attached to that tmux session (use TerminalLauncher)
- Terminal preference setting (Ghostty default, Warp, iTerm as options)
- Session registry tracks active sessions, survives app restart
- Sidebar shows session status: running, waiting, completed, detached

## What exists

- `TmuxSession.swift` — creates/kills tmux sessions
- `TmuxSessionRegistry.swift` — tracks active sessions
- `TerminalLauncher.swift` — opens external terminals
- `TerminalWorkspaceView.swift` — terminal tab UI with session status

The infrastructure is there. The gap is making it the default launch path instead of an alternative.
