# Open Questions

## Resolved

The branch name `newrepos` was ambiguous. After reviewing the existing design doc, the interpretation is **"improve new repository experience"** — better onboarding when loopflow is run in an uninitialized repo.

## Still Open

1. **Dependency checking granularity** — Should missing deps block task execution, or just warn? Current design warns but doesn't block.

2. **Error message format** — The design uses plain text. Should we use color/formatting consistent with Typer's style (e.g., `typer.style()`)? Keeping it simple for now.
