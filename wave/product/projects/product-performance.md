# Product Performance

Loopflow feels immediate under real work. Product surfaces stay glanceable and
steerable while waves, runs, chats, workers, and audit records accumulate.

## KRs

- The five core paths — list waves, open a wave, acknowledge send/steer chat,
  open audit, attach to a run — each have an explicit p95 latency budget before
  the proof window starts and hold it for a month of real accumulated data,
  measured on the living workspace rather than a demo state.
- Budgets are enforced by gates: a regression fails visibly before release,
  demonstrated by at least one caught regression or one full quarter of
  green measured releases.
- Slow paths preserve control under the worst week of data we have: state
  visible, interrupt available, recovery possible without waiting on full
  hydration.
