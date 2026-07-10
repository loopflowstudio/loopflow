# Product Performance

Loopflow feels immediate under real work. Product surfaces stay glanceable and
steerable while waves, runs, chats, workers, and audit records accumulate.

## KRs

- Core user paths hold their p95 latency budgets for a month of real
  accumulated data: list waves, open a wave, send/steer chat, and inspect
  audit show useful state within 1 second; attaching to a run yields an
  interactive terminal within 2 seconds. Measure on the living workspace,
  never a demo state.
- Budgets are enforced by gates: a regression fails visibly before release,
  demonstrated by at least one caught regression or one full quarter of
  green measured releases.
- Slow paths preserve control under the worst week of data we have: state
  visible, interrupt available, recovery possible without waiting on full
  hydration.
