# LOO-386 design review

October 7, 2026. Jack Heart accepted retaining Workflow and Flow with Workflow
commands scoped to their owner. Jack then clarified that Projects and Tasks both
have workflows, proposing `lf project workflow set` and
`lf task workflow restart`, with no standalone `lf workflow` commands.
See [the design](align-the-data-model-and-names-after-workflows-one-more-pass.md).

Settled constraints:

- Keep `lf task run` traversing the Workflow and `lf run` executing independently.
- Keep nodes/edges and outgoing edges named by the unique Flow or skill they run.
- LOO-400 supplies Process/FlowProcess and LFID/PID. Its landed `e467ea995`
  is integrated at `7f3169596`, superseding the requested `e530eb780` stack.
  No competing Exec rename remains.
- Jack accepted `customize` copying a builtin only if needed and printing its
  local path. Its placement follows the implementation choice below.
- Projects select a definition; Tasks retain their captured Workflow and position.
  A Project setting change does not silently reset existing Tasks.
- Jack specified that `restart` resets position. It moves the captured Workflow
  to `start`, preserves history and leaves execution to `lf task run`.

Implementation choices and remaining uncertainties:

- Reusable definition list/customize lives under `lf project workflow`; Task
  workflow commands act on its captured instance. This placement is the agent's
  implementation choice within Jack's direction.
- Reuse stable Project resolution for show/set. LOO-397 has no execution or
  checkout as of October 7; its later CLI map consumes these owner paths.
- Use process vocabulary for Flow inspection flags and preserve historical
  captures while cutting Rust/Swift projections over together.

Jack Heart requested “ok. cool. move it forward.” on October 7 after settling
restart. Implement through pursue and return at demo review. Earlier synonyms
are research evidence, not targets; this grants no approval of the finished demo
or permission to skip it and land.

Implementation review, October 7: Project show/set accepts stable IDs and unique
names/slugs; catalog commands occupy Project's Workflow group. The obsolete
`wave update-plan --workflow` shortcut is removed. A full plan still replaces
its Workflow through `wave update-plan --plan`. Directory read failures remain
errors, and saved Task projections disable run controls until refreshed.
These are implementation choices, not additional approvals from Jack Heart.
