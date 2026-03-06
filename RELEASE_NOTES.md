# v0.9.9

Loopflow 0.9.9 lets agents write their own commit messages and catches release collisions before they waste a CI cycle.

Changes since `v0.9.8`.

## Commits without `-m`

- **`lf commit` auto-generates messages** — when no `-m` is provided, the staged diff is sent to a lightweight agent (claude:haiku) that produces a formatted `lf <task>: <title>` message. Explicit `-m` still works. Falls back to a prefix-only message if generation fails
- **Internal commits use the same path** — `auto_commit_if_dirty` and `post_step_sync` now go through `commit_workflow` instead of calling raw git helpers, so auto-commits get meaningful messages too

## Release safety

- **`lf ops land` fails fast on existing tags** — if a release tag already exists on origin, land aborts immediately instead of proceeding to a merge queue that will fail later
