---
status: proposed
area: infra
---

# Add linting to CI

Ruff is configured locally but CI only runs pytest. Failed lints get caught in local dev (if you remember to run them) but can slip through PRs.

## Scope

- Add `ruff check src/` to CI workflow
- Add `ruff format --check src/` to CI workflow
- Keep it as a separate job so test failures and lint failures are distinguishable

## Not included

- Expanding ruff rules beyond current E/F/W/I selection
- Pre-commit hooks (different item)
- Fixing existing lint issues on stricter rules

## Approach

Add a `lint` job to `.github/workflows/ci.yml`:

```yaml
lint:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: astral-sh/setup-uv@v4
    - run: uv sync
    - run: uv run ruff check src/
    - run: uv run ruff format --check src/
```

This catches:
- Import sorting issues
- Syntax errors
- Formatting drift

Fast (<10s), no false positives with current rules.
