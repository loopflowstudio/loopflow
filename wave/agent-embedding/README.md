# Agent Embedding

Concerto becomes the conductor, not the chat client. The agent runs in a real terminal. Concerto provides everything outside the coding session: what's stuck, what needs you, where things stand.

Claude Code, Cursor, Windsurf, OpenCode — all ahead on chat polish, all iterating faster. Concerto embeds their terminals and builds the UX *around* coding sessions. The conductor view, not the chat view.

## Concerto's job

- Block queue — what's stuck, what needs you
- Terminal embedding for coding sessions (Ghostty)
- Portfolio view (multi-repo, multi-wave status at a glance)
- Worktree/PR lifecycle management
- Wave configuration and monitoring
- Calibration view for tend flow trajectory review
- Window composition — native Swift alternative to tmux, mixing terminals with native diff viewers, chat views, wave editors
- Chord-wave graph view — hierarchical visualization of waves-over-waves relationships, derived from area paths

When OpenCode ships a desktop app or native components, evaluate adopting them for the chat view. Until then, Ghostty terminal embedding.

## Strategy

Block queue view first — it's the new primary screen and the UX that proves the conductor concept. Terminal embedding second — makes Concerto the place you actually work. Portfolio and lifecycle views build on the foundation once the core interaction model is proven.

The calibration view (trajectory review across waves) comes last because it requires tend flow output to be meaningful.

## Goals

- Primary Concerto screen is a block queue, not a chat view
- Coding sessions happen in embedded Ghostty terminals
- Multi-repo, multi-wave status visible at a glance
- Wave lifecycle (create, configure, start, stop) managed from Concerto
- Human calibration moments have a dedicated UX
- Chord-wave graph derived from area relationships, not a separate data model

## Risks

- Ghostty terminal embedding is unproven in SwiftUI — may need fallback plan
- Block queue is only useful once signals/blocks are being generated — chicken-and-egg with signals wave
- Portfolio view scope could expand unboundedly

## Metrics

- Clicks from "I see a problem" to "I'm acting on it" (target: <=2)
- Time to assess all-waves status (target: <10 seconds glance)
- Percentage of coding sessions that happen inside Concerto vs external terminal
