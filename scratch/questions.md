# Open questions

## Existing-repository migration

- Linear currently refuses a second team with “limit of teams allowed in your
  current plan.” No binding changed during the failed PRD attempt. A distinct
  Task tag requires a distinct Linear team, and keys are workspace-global, so
  the production migration needs enough team capacity for every deliberately
  distinct Wave identity. Do not collapse domain Waves onto shared archetype
  teams merely to work around the provider limit.

- Loopflow already has Product, Infrastructure, and Intelligence. Do not author
  Operations until a concrete recurring-operation Project exists; an empty
  fourth Wave would contradict the lazy bootstrap rule.
- Manabot already expresses the archetypes in domain language: Game owns the
  Product role, Rules owns Infrastructure, and Intelligence names itself.
  Study remains a durable product subdivision. Give the actual Waves optimized
  stable tags (for example Game/GAM and Rules/RUL), then add real hierarchy
  without renaming them or creating generic shells beside them. Operations
  stays lazy until a concrete operational Project exists.
- Cadenza is now Kata at `/Users/jack/src/kata`. Ignore the broken legacy
  `cadenza.*` worktrees. Kata currently has Core, Ear, Feedback, Release,
  retired Scores, and Theory Waves; use the four archetypes to test their live
  ownership without reviving Scores, flattening domain names, or adding empty
  roots.
