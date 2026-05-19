---
status: in-progress
claimed_by: f6684ab5-6f24-4bec-97a5-52dcc912fd38
claimed_at: 2026-05-19T02:01:39.027621Z
asana_id: '1214270115439574'
---
# Native chat UX

**Finish line:** Concerto's chat view is a polished native conversational surface — rich rendering (markdown, syntax, diffs), smooth streaming (no jank, no layout jumps), browsable history, and a composer that supports file drop, slash commands, and context awareness.

## Context

Streaming infrastructure shipped: `SessionState` owns derived transcript state, `MessageRow` caches segment parsing, animations route through `DesignAnimation`. Base chat works. Missing for it to feel like a product:

- **Rich rendering** — markdown renders correctly; code blocks get syntax highlighting; diffs show as unified-diff blocks; tool calls collapse/expand smoothly
- **Conversation history** — browse past sessions in a list, resume any, filter by wave or time
- **Composer upgrades** — drag-drop files into the composer; slash commands (`/code`, `/image`, `/search`); context awareness (current wave, file, selection)

## Daily experience

Conversations are where you think with an agent. Ask "how should I structure this?" — the answer comes in formatted markdown with syntax-highlighted code. Drag a file in, the agent picks it up. Scroll back a week, resume a conversation about a prior decision. The chat isn't a fallback; it's where you go when the work is exploratory.

## Done when

- Markdown / syntax highlighting / diffs render at production quality
- Browse and resume past sessions without dropping to terminal
- Composer supports file drop and slash commands
- Streaming stays smooth — target: 0 dropped frames at 30 tok/s, 100-entry transcript
