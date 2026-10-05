# Reconciliation assumptions — October 4, 2026

No unresolved product judgment blocks implementation. Jack Heart approved the
workflow design; its [implementation defaults](focus-on-your-own-work.md) remain
reversible: 120-second Waiting fallback, most-recent interactive Task primary,
optional occurrence names, backward-target loops and source-only workflow YAML.
These are implementation choices, not Jack's exact spelling/timing preferences.

The saved-capture bridge preserves the existing invocation; live migration and
configured acceptance remain separate from implementation.

Superseded questions and proposals: `5090f672e:scratch/questions.md`.

Implementation interpretation: the existing per-Task automation on/off field also
holds CI repair. Preserve that setting and its delivery UI while deleting Flow
scheduling and its counters. This preserves unrelated CI policy; it provides no
Flow restart authority.
