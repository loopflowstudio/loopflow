# CI: Enforce Lint & Format

**Status:** done

## Problem

Ruff is configured in `pyproject.toml` but not enforced in CI. Code can be merged without passing lint or format checks. This creates drift between what developers run locally and what CI enforces.

## Proposal

Add lint and format checking to the CI pipeline:

```yaml
lint:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: astral-sh/setup-uv@v4
    - run: uv sync
    - run: uv run ruff check .
    - run: uv run ruff format --check .
```

## Why This Matters

- Ruff is already configured and rules are defined
- Fast execution (~1-2 seconds)
- Catches import ordering, unused variables, style issues before merge
- Prevents accumulation of lint debt

## Open Questions

None. Ruff config already exists; this just enforces it.
