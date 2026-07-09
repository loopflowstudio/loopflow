# Context

Everything the model sees is one information system. The authored surface —
prompts, skills, flows, directions, config — and the dynamic surface — the
agent bus, wave memory summaries, and per-pass context generation — should be
deliberate, minimal, bounded, and evidence-justified. Quality comes as much
from the knobs we refuse to add as from the text we write.

## KRs

- Evidence-cited edits: every prompt/skill/flow change lands with a cited
  local run failure or cost trace and a follow-up run showing the intended
  behavior. The nine tier skills (wave/project/task × clarify/pursue/
  mutate) are the current targets — shipped unmeasured.
- Median tokens per comparable run trends down without first-pass gate
  regression (re-baselined once trace feeds usage).
- Zero-config excellence: a fresh repo works superbly with nothing
  configured; the config-surface audit removes knobs and writes down the
  non-configurability doctrine.
- The agent bus carries reports worth folding: a child's report is complete
  enough to act on without reading its transcript.
- Context generation earns its place: what rides into each pass seed and
  each child's prompt (<lf:wave-memory>, <lf:wave-chat-recent>, folds) is
  measured, bounded, and shown to help.
