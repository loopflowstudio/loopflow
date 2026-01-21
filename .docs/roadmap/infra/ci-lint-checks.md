---
status: proposed
area: infra
---

# Add linting to CI

CI runs pytest but not ruff. Code can merge with linting/formatting violations. Currently 3 files would be reformatted and 2 lines exceed the configured limit.

## Scope

- Add ruff check and ruff format --check to CI workflow
- Fix existing lint violations so CI passes
- Not included: pre-commit hooks (separate item), editor integrations

## Approach

Add a `lint` job to `.github/workflows/ci.yml` that runs:

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

Fix existing violations:
- `src/loopflow/lfd/__init__.py`: 2 lines over 100 chars
- `src/loopflow/lf/run.py`: formatting
- `src/loopflow/lfd/models.py`: formatting
