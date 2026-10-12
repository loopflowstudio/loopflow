# Decisions and remaining assumptions

2026-10-11 — Instructions at the right level, Jack Heart.

- Jack accepted a work-guidance skill, preloaded for Work entry points.
  `lf help skill work` supplies it to an existing conversation; command JSON is unchanged.
- Jack required `lf wt create`, `lf pr open` and taskless Flows to remain independent
  of Task creation and Workflow enrollment.
- Existing-Task detection uses the current checkout or resumed Session. A Task
  elsewhere in the repository does not opt this work in. This is the narrow
  interpretation proposed in conversation; no repository-wide scan was added.
- Launcher guidance follows the user's tools and repo policy; Task-managed
  operations use lf to preserve recorded state.
- No Wave placement was supplied or needed for the local implementation.

- Jack identified the taskless-operation prohibition as internal policy, not
  user-facing copy. Docs now teach the available commands; agent guidance names
  the trigger for loading Work context. The internal boundary remains in tests
  and the skills that would otherwise enroll work.

- Jack required `wave/` writes to belong to selected Loopflow Flows, not merely
  Work API usage. Standalone memory procedures retain proposals in scratch.
- 2026-10-12 — Jack rejected `scratch.managed`, reserved directories and ownership
  manifests. Use the Git diff: delete added scratch files at cleanup, retain
  existing files and edits. Skills keep writing to their existing paths without
  configuration. `lf pr submit` and `lf pr land` invoke
  `lf scratch clear` automatically; the command is also available independently.
  PR open and publish do not run general scratch cleanup.
  Writing and automatic context inclusion remain defaults. This supersedes
  the earlier repository-opt-in design.
- Jack asked that the same lesson apply across the branch: infer useful behavior
  from existing context, preserve defaults, and avoid enrollment questions or
  setup requirements that make ordinary use harder.

- Jack requested continuation into instruction explanations and decision
  evaluations. Existing capture decisions own explanations; no user-level mode
  or new schema. Nine fixtures and a scorer are provider-neutral. A live model,
  version and evaluation environment have not been selected; rendered inputs and
  scorer tests do not constitute model evaluation results.
