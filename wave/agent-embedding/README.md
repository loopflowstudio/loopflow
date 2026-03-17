# Agent Embedding

Concerto becomes the conductor, not the chat client. The agent runs in a real terminal. Concerto provides everything outside the coding session: what's stuck, what needs you, where things stand.

## Strategy

Block queue view first — it's the new primary screen and the UX that proves the conductor concept. Terminal embedding second — makes Concerto the place you actually work. Portfolio and lifecycle views build on the foundation once the core interaction model is proven.

The calibration view (trajectory review across waves) comes last because it requires tend flow output to be meaningful.

## Goals

- Primary Concerto screen is a block queue, not a chat view
- Coding sessions happen in embedded Ghostty terminals
- Multi-repo, multi-wave status visible at a glance
- Wave lifecycle (create, configure, start, stop) managed from Concerto
- Human calibration moments have a dedicated UX

## Risks

- Ghostty terminal embedding is unproven in SwiftUI — may need fallback plan
- Block queue is only useful once signals/blocks are being generated — chicken-and-egg with signals wave
- Portfolio view scope could expand unboundedly

## Metrics

- Clicks from "I see a problem" to "I'm acting on it" (target: ≤2)
- Time to assess all-waves status (target: <10 seconds glance)
- Percentage of coding sessions that happen inside Concerto vs external terminal
