# Evals

Is loopflow worth it, provably? Loopflow-as-harness measurably beats the
alternatives — raw codex/claude loops, other orchestrators — on real work,
and every change to the system stays answerable to that comparison forever.

## KRs

- An eval set of matched real tasks (loopflow and Cadenza work, not toys)
  runs through loopflow flows vs bare vendor loops: completion, cost,
  wall-clock, human interventions — recorded and comparable.
- Prompt/skill/flow changes are gated by eval regressions, not vibes — the
  quality proxy question is answered: the proxy is the eval.
- The comparison is rerunnable on demand (one command) so a claim like
  "the tier skills made task completion better" is checkable the day it's
  made.
