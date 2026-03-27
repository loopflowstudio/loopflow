# 00: Open in Terminal

A button on the wave workspace that opens a shell at the repo path — externally or embedded. Always backed by a tmux session so you can switch between the two.

## Done when

- "Open Terminal" button on workspace creates a tmux session and opens it in external Warp (or configured terminal)
- "Open Internally" option attaches the same tmux session in Concerto's embedded Ghostty view
- Closing either window doesn't kill the session — you can reopen in either mode
- If the tmux session already exists, attach to it instead of creating a new one

## What to build

- Create a named tmux session per wave workspace (e.g. `lf-<wave-id>-shell`)
- "Open Terminal" button: create tmux session if needed, then launch external Ghostty attached to it
- "Open Internally" button/menu option: attach Concerto's embedded `GhosttyTerminalView` to the same tmux session
- Register session in `TmuxSessionRegistry` so cleanup and reattach work
- Ghostty only — no fallback to other terminals needed

### Why Ghostty as default

Ghostty has a proper CLI — no AppleScript UI scripting, no `delay 0.5`, no keystroke injection:

```bash
/Applications/Ghostty.app/Contents/MacOS/ghostty \
  --working-directory=/path \
  --command="tmux attach -t session-name"
```

Warp's launcher uses AppleScript `keystroke` commands which are fragile and have historically failed to auto-run agent commands. Ghostty is also what Concerto already embeds via libghostty — one terminal, two modes (embedded and standalone).

Add a `launchGhostty` case to `TerminalLauncher` alongside the existing Warp/iTerm/Kitty launchers.

## What exists

- `TmuxSession.swift` — create/attach/kill tmux sessions
- `TmuxSessionRegistry.swift` — track active sessions
- `TerminalLauncher.swift` — open external terminals (Warp, iTerm, Terminal.app, Kitty — no Ghostty yet)
- `GhosttyTerminalView.swift` — embedded terminal via libghostty
- Ghostty installed at `/Applications/Ghostty.app/Contents/MacOS/ghostty`

The pieces exist. Wire them together behind one button with two modes. Add Ghostty as a launcher target.
