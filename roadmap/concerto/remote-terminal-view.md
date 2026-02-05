---
status: deferred
phase: 4
---

# Remote Terminal View

Terminal view on mobile streaming from lfd.

## Status: Deferred

This was originally planned as the primary mobile interaction model. After reconsidering:

- Terminal UX on phones is poor
- We support multiple agents (Claude Code, Codex, Gemini)—each has different terminal UI
- Building an iOS terminal renderer is significant work
- Mobile users (conductor persona) want status and actions, not terminal sessions

## If we build it

Power-user escape hatch for those who really want raw terminal on mobile.

- Terminal rendering component for iOS
- Connects to WebSocket terminal stream
- Keyboard input handling
- Touch scrolling through history

## Alternative

Most users will use:
- Phase 2: Non-interactive actions (land PR, trigger step)
- Phase 3: Chat interface (discuss code, get suggestions)
- Laptop: Interactive terminal sessions via Ghostty

SSH/Tailscale remains an option for power users who need raw terminal access from mobile.
