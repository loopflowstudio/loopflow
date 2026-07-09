# Context

Everything the model sees is one information system. The authored surface —
prompts, skills, flows, directions, config — and the dynamic surface — the
agent bus, wave memory summaries, and per-pass context generation — should be
deliberate, minimal, bounded, and evidence-justified. Quality comes as much
from the knobs we refuse to add as from the text we write.

## KRs

- Evidence-cited editing is the standing practice: for a month, every
  prompt/skill/flow change lands with a cited run failure or cost trace and
  a follow-up run showing the intended behavior — the nine tier skills get
  their citations first.
- Median tokens per comparable run trends down across a month of real runs
  without first-pass gate regression.
- Zero-config excellence, repeated: 3/3 fresh repos reach a landed PR with
  nothing configured; the config-surface audit removes knobs and the
  non-configurability doctrine holds for a month without a new knob.
- Bus reports stay foldable: a week in which the wave acts on every child
  report without opening a single child transcript.
- Context generation earns its place continuously: what rides into each
  pass seed and child prompt is measured and bounded, and stays inside its
  budget as history accumulates.
