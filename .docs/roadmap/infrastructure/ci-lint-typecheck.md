# CI: Lint and Typecheck

**Status:** proposed
**Area:** infrastructure
**Priority:** high

## Problem

There's no CI pipeline. PRs can be merged without passing lint or typecheck. This contradicts the "craft over vibes" philosophy—we say we care about quality, but we don't enforce it.

Recent commits show lint improvements (`lf lint: add built-in lint step`), but nothing runs these checks automatically on PRs.

## Proposal

Add GitHub Actions workflow that runs on every PR:

1. **Lint** — `ruff check src/`
2. **Typecheck** — `pyright src/`
3. **Tests** — `pytest tests/`

### Configuration

```yaml
# .github/workflows/ci.yml
name: CI
on:
  pull_request:
    branches: [main]
  push:
    branches: [main]

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: astral-sh/setup-uv@v4
      - run: uv sync
      - run: uv run ruff check src/
      - run: uv run pyright src/
      - run: uv run pytest tests/
```

### Branch protection

After CI is working, enable branch protection on `main`:
- Require CI to pass before merge
- Require up-to-date branch

## Success criteria

- PRs show check status
- Can't merge to main if lint/typecheck/tests fail
- `lf lint` output matches CI output

## Dependencies

- ruff (already in dev deps)
- pyright (add to dev deps if missing)
- GitHub Actions access

## Effort

Tiny: one session to write the workflow and test it.
