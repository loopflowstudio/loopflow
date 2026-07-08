# Wave conducting

Create, start, and observe a wave without leaving the app. Audited
2026-07-08: the loop is already in-app (CreateWaveSheet writes GOAL.md
file-first; Start launches detached-tmux `lf wave`; observe/steer rides one
SSE with a verb-aware composer) — and the old "never render a native
assistant chat; frame the vendor TUI" doctrine is dead in fact: WaveChatView
renders a fully native thread today and no vendor TUI is framed anywhere.
The steward design legitimizes this: the native thread IS the steward
conversation.

## KRs

- The chat view becomes the steward thread (goals/flowloop bet): one warm
  executive mind owns the human conversation.
- The agent bus gets its own unabridged audit window beside runs — curation
  is a reading aid, never a filter on the record.
- Create/start/observe stay one-action from the wave list as the steward
  split lands (regression bar for what already works).
