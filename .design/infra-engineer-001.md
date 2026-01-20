# Add Ruff Linting Infrastructure

Add automated linting and formatting with ruff. This catches code quality issues in CI and provides consistent formatting.

## What We're Adding

1. **Ruff configuration** in pyproject.toml
2. **Ruff as dev dependency**
3. **CI job** that runs ruff check and ruff format --check

## Configuration

```toml
[tool.ruff]
line-length = 100
target-version = "py310"

[tool.ruff.lint]
select = [
    "E",   # pycodestyle errors
    "F",   # pyflakes
    "W",   # pycodestyle warnings
    "I",   # isort (import sorting)
]

[tool.ruff.lint.per-file-ignores]
"tests/*" = ["F841"]  # unused variables OK in test fixtures
```

## CI Changes

Add a `lint` job that runs before tests:

```yaml
lint:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: astral-sh/setup-uv@v4
    - run: uv sync
    - run: uv run ruff check src/ tests/
    - run: uv run ruff format --check src/ tests/
```

## Rollback

If ruff causes issues:
1. Remove `[tool.ruff]` section from pyproject.toml
2. Remove ruff from dependency-groups
3. Remove lint job from ci.yml

## Done When

- [x] `uv run ruff check src/ tests/` passes with no errors
- [x] `uv run ruff format --check src/ tests/` passes
- [x] CI lint job passes
- [x] `uv run pytest tests/` still passes
