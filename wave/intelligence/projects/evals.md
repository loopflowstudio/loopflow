# Evals

Is loopflow worth it, provably? Loopflow-as-harness measurably beats the
alternatives — raw codex/claude loops, other orchestrators — on real work,
and every change to the system stays answerable to that comparison forever.

## KRs

- An eval set of matched real tasks (loopflow and Cadenza work, not toys)
  runs loopflow flows against bare vendor loops: completion, cost,
  wall-clock, human interventions — and loopflow wins on the suite in
  three consecutive monthly runs.
- The suite runs on cadence for a quarter — weekly and on every release —
  without manual repair; a broken eval is a stop-the-line event.
- Prompt/skill/flow changes are gated by eval regressions for a month:
  zero ungated changes land, and the quality-proxy question stays answered
  (the proxy is the eval).
- Any capability claim made in chat or docs is rerunnable on demand (one
  command) the day it's made.
