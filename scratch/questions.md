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
- Jack permits writing scratch and wants cleanup optional. Chosen mechanism:
  repo-only `scratch.managed: true`, default false, covering deletion and
  removal from published history. Directory existence never determines ownership.
