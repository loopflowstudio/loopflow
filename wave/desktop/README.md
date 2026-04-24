# Desktop

Concerto for macOS. Native workspace for conductors running agents.

## Vision

Replace the external-terminal build flow with a first-class embedded workspace. The conductor opens Concerto, sees attention across waves, drives build work inside the app, and only drops to a full Ghostty window when the task actually wants one. The embedded terminal and the multiplexer workspace carry the day; Concerto makes agents legible and steerable without window-juggling.

Native chat UX is the second priority. Some users want a chat interface, full stop. Streaming, transcript, tool cards, voice input, and quote-reply are already in place — the remaining layer is markdown/diff rendering, history browse, and composer polish.

### Not here

- Replacing the external terminal as an option — long interactive sessions will still prefer a real Ghostty window.
- Replacing the CLI — the CLI is the source of truth; Concerto composes the work.
- Chat-shell wrapper — the agent runs in a terminal, not a chat input box.

## Priorities

1. **Embedded terminal as build driver.** Launch flows/steps inside the embedded workspace and make the experience match or exceed external Ghostty for day-to-day build work. Worktree-aware. Multi-agent ready.
2. **Native chat UX.** Markdown + syntax highlighting + diffs. Conversation history. Composer upgrades.

## Risks

- Embedded terminal parity with Ghostty is bounded — tabs, splits, config, GPU optimizations will always lag. The line between "embedded is great for this" and "open external Ghostty" needs to be explicit.
- Scope creep on polish — everything can be polished further. Each priority-1 item needs a clear "done when."
- Chat UX and embedded terminal compete for the same screen real estate. Workspace layout needs to handle both gracefully.
