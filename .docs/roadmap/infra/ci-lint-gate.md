---
status: proposed
area: infra
---

# CI Lint Gate

Add ruff lint and format checks to CI so violations fail the build instead of accumulating silently.

## Scope

**Included:**
- Add lint step to CI workflow
- Add format check step to CI workflow
- Fix existing violations before enabling

**Not included:**
- Pre-commit hooks (separate item)
- Additional lint rules beyond current ruff config
- Type checking (mypy/pyright)

## Approach

1. Fix the 2 line-too-long errors in `src/loopflow/lfd/__init__.py`
2. Run `ruff format` on the 3 files that need reformatting
3. Add CI steps after tests:

```yaml
- name: Lint
  run: uv run ruff check src/ tests/

- name: Format check
  run: uv run ruff format --check src/ tests/
```

Fast, no extra dependencies (ruff already in dev deps), prevents drift.
