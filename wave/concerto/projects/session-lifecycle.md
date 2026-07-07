# Session lifecycle

The spine. A wave session must outlive the app and be one action away.

## KRs

- A running wave session survives app restart and reattaches cleanly in 5/5
  dogfood trials.
- From the wave list, launching or attaching the right vendor session takes one
  action.
- No Swift-owned parallel tmux/session lifecycle when an `lf` or `lfd` session
  record can own it.
