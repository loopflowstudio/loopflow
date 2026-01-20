---
status: proposed
area: infra
---

# Add lint and type checking to CI

CI runs tests but doesn't enforce code quality. Ruff is configured in pyproject.toml but never runs in the pipeline. Type hints are everywhere but never validated.

## Scope

Included:
- Add `ruff check` job to CI
- Add `ruff format --check` job to CI
- Add type checking job (mypy or pyright)
- Fail PR if any check fails

Not included:
- Pre-commit hooks (local dev tooling)
- Stricter lint rules beyond what's already configured
- Coverage thresholds (separate roadmap item)

## Approach

Add a new `lint` job to `.github/workflows/ci.yml`:

```yaml
lint:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: astral-sh/setup-uv@v4
    - run: uv sync
    - run: uv run ruff check src/ tests/
    - run: uv run ruff format --check src/ tests/
    - run: uv run mypy src/loopflow/
```

Mypy will need a `[tool.mypy]` section in pyproject.toml with reasonable defaults (e.g., `strict = false` initially, explicit module list).

This catches style drift and type errors before merge. The lint config already exists—this just enforces it.
