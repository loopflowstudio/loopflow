---
status: proposed
area: maestro
created_at: 2026-01-20T15:33:00
---

# Maestro MVP: The Podium for Conducting Agents

Vision: "Maestro is where you conduct from—launch tasks, watch progress, ship when ready."

## The Problem

Today, agent orchestration requires:
- Multiple terminal windows
- Manual `lfd status` polling
- Switching between worktrees with shell commands
- No unified view of all running work

## What Maestro Does (MVP)

Native macOS app showing:

1. **Worktree list** - All worktrees in the repo, their branches, status
2. **Loop status** - Running loops with live indicator
3. **Quick actions** - Open in terminal, open in Cursor, ship (PR + land)
4. **Notifications** - When loops complete or need attention

## What Maestro Doesn't Do (MVP)

- No embedded terminal (launch in your terminal app)
- No streaming output (check terminal for details)
- No IDE integration (works alongside Cursor/VS Code)

## Implementation Path

1. **Start with SwiftUI** - Native macOS, menu bar presence
2. **IPC via lfd** - Maestro talks to daemon for status
3. **Launch protocol** - `maestro://open?worktree=feature-auth`
4. **Minimal state** - All truth in lfd database

## Protocol Between Maestro and lfd

```
lfd serve --maestro-socket /tmp/maestro.sock

Commands:
- list-worktrees
- list-loops
- loop-status <id>
- start-loop <id>
- stop-loop <id>
```

## Open Questions

1. Menu bar app vs full window app vs hybrid?
2. How to handle multi-repo workflows?
3. Should Maestro manage terminal app preferences?
