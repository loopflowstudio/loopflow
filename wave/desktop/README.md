# Desktop

Concerto for macOS. Native workspace for conductors running agents.

## Vision

Replace the external-terminal build flow with a first-class embedded workspace. The conductor opens Concerto, sees attention across waves, drives build work inside the app, and only drops to a full Ghostty window when the task actually wants one. The embedded terminal and the multiplexer workspace carry the day; Concerto makes agents legible and steerable without window-juggling.

Native chat UX and the conductor surfaces (runboard, portfolio, calibration, beat composition) are the rest of the daily experience — when to talk to agents, and how to watch them across waves.

### Not here

- Replacing the external terminal as an option — long interactive sessions will still prefer a real Ghostty window
- Replacing the CLI — the CLI is the source of truth; Concerto composes the work
- Chat-shell wrapper — the agent runs in a terminal, not a chat input box

## Tasks

1. **`embedded-terminal-build-driver`** (p1) — Concerto's embedded terminal replaces external Ghostty for daily build. Demo: morning, open Concerto, all build work stays in the app
2. **`native-chat-ux`** (p2) — rich chat: markdown / code / diffs / history / composer. Demo: think with an agent in polished native chat
3. **`conductor-surfaces`** (p2) — runboard + portfolio + calibration + beat composition. Demo: 10-second morning scan across all waves

## Risks

- Embedded terminal parity with Ghostty is bounded — tabs, splits, config, GPU optimizations will always lag
- Scope creep on polish — each task needs a clear "done when"
- Chat UX and embedded terminal compete for workspace real estate — layout needs both
