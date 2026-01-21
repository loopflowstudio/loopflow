# Open Questions

## Pre-commit hooks (infra)

- Should `lf init` automatically run `pre-commit install`, or just prompt the user?
- The codebase already has ruff 0.9.0 in dev dependencies. Should we match that version exactly in the pre-commit config, or allow them to drift?
