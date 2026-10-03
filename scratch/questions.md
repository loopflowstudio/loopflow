# Remaining implementation choices

Jack Heart resolved naming as **New Session**, accepted the Linear-style split-row
prototype, and requested preserving the existing opening prompt. Capture scope and
Task sizing are accepted; no product decision blocks the current implementation.
The reconciled contract and remaining acceptance are in [capture-task.md](capture-task.md).

These reversible choices implement that direction without separate approval:

- A selected Project uses its Wave; an unresolved Wave falls back to repository
  scope. The explicit Wave menu action appears on Wave rows only.
- The Wave menu uses the repository's selected skill, matching New Session.
  A disappeared saved skill stays selected and exposes the CLI's launch error.
- Discovery refreshes on picker opening. Native preferences use canonical repo
  identity. No Edit skill action was added.
- The existing navigation owner carries retained-workspace presentation while
  Task selection stays intact; ordinary selection clears that presentation.
- Wave/session reads capture-tasks instead of duplicating its former filing
  guidance. Existing authorization governs filing and operating authority stays
  unchanged.

Native appearance and keyboard/provider behavior remain demo judgments, not new
scope questions. Historical naming discussion and superseded implementation
assumptions are retained at
`25f183992969f59b42f9765d7594638cc91652dc:scratch/questions.md`.
