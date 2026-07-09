# Mac Surface UX

The Mac app is the daily conductor surface for loopflow: glanceable wave state,
fast navigation, launch, reattach, steering, and audit views around the shared
product API.

## KRs

- The app is the only surface needed to drive waves for one full week of
  real work — every drop to the terminal for a wave operation is recorded
  as a failure of this bet.
- Launching, reattaching, steering, and inspecting stay one action from the
  main surface as the wave count and history grow — measured on the real
  accumulated workspace, not a fresh one.
- The wave list makes the next required attention obvious: in a week of
  dogfood, the top-ranked wave is the right one to open >= 9 times in 10.
- The surface model is settled and stays settled: primary list, detail,
  chat, audit, and terminal affordances each keep one clear home for a
  month without a relayout.
- No Swift-owned parallel session lifecycle, verified continuously by the
  reattach trials in wave-chat.
