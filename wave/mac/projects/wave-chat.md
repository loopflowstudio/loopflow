# Wave chat

Driving: the core experience of working WITH a wave from the app. The chat
view is the steward thread (datamodel/flowloop bet) — one warm executive
mind owns the human conversation.

## KRs

- The chat view renders the steward thread; composer verbs (send / steer /
  interrupt) target the steward's session, never the engine.
- The session spine holds: tmux-externalized process + endpoint rediscovery
  + replay survives app restart 5/5 dogfood trials; no Swift-owned parallel
  session lifecycle (delete the iOS-legacy SessionState machinery).
- From the wave list, launching or reattaching is one action; a new wave can
  be created, started, and conversed with without leaving the app
  (regression bar — this works today).
