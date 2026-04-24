---
asana_id: '1214270114156178'
---
# Embedded terminal build driver

**Finish line:** Concerto's embedded terminal replaces external Ghostty for daily build work. Flows launch from the command palette into embedded tmux sessions, persist across app restarts, and the workspace layout (tabs/splits per wave) survives too. Dropping to an external Ghostty window becomes a deliberate choice for long interactive sessions, not the default.

## Context

The workspace multiplexer shipped: binary split tree per wave; runs / roadmap / terminal / markdown / diff / launcher panes; tmux-backed terminal pane; focus-aware keyboard routing. `TerminalSession` lives in lfd with persistence. What's missing to make this the daily driver:

- **Flow launch from palette** — command palette runs `lf <step-or-flow>` inside the embedded terminal with the right worktree, not via a separate Terminal.app window
- **Reattach across restarts** — close Concerto, reopen, terminal sessions reattach; tmux is source of truth, embedded view is a client
- **Multi-agent dispatch** — pick Claude / Codex / OpenCode per step; the terminal session header shows which provider is running
- **Workspace layout** — multiple terminals per wave (split, tab), layout serialized, next launch opens to the same arrangement
- **Polish enough to feel first-class** — window composition, focus rings, padding, scroll handling

## Daily experience

Morning: open Concerto. Pick a wave. `⌘K`, type "ship," Enter. Flow runs in the embedded terminal pane. Close your laptop. Evening: open, session is still there, output has progressed. When you want a full Ghostty window for a long interactive session, one click pops it out — but 90% of daily build work stays in the app.

## Done when

- Flow launch from command palette runs in the embedded terminal, not Terminal.app
- Sessions survive Concerto restart and reattach cleanly
- Multi-agent dispatch visible in the session header
- Layout persists per wave across launches
- The embedded terminal no longer feels second-class compared to real Ghostty for build work
