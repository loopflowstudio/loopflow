# flows

Flow definitions live in `.lf/flows/*.py` and support choose/fork/join. Loops track area, goals, and flow with updated docs/tests.

## Status

**Verdict:** Ready

- Fork join now captures committed fork changes when the worktree is clean.
- Maestro loop loading handles legacy goal columns and non-JSON goal strings.

## Design notes

- Join summaries remain optional (`.design/joins/<flow>.md`).
- Open questions: should join require an explicit output artifact, and how should the CLI surface required `--flow` usage?
