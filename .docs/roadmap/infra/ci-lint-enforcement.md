---
status: proposed
area: infra
---

# CI Lint Enforcement

Add ruff linting to CI so code style issues fail the build instead of silently passing.

## Scope

- Add `uv run ruff check src/ tests/` step to CI workflow
- Fix existing lint violations (2 line-length errors in `lfd/__init__.py`)

**Not included:**
- Type checking (mypy/pyright)—separate item if desired
- Ruff formatting enforcement—just checking for now

## Approach

1. Fix the 2 existing lint violations in `src/loopflow/lfd/__init__.py`:
   - Line 349: break long function call across lines
   - Line 369: break long typer.Option across lines

2. Add a `lint` job to `.github/workflows/ci.yml`:
   ```yaml
   lint:
     runs-on: ubuntu-latest
     steps:
       - uses: actions/checkout@v4
       - uses: astral-sh/setup-uv@v4
       - run: uv run ruff check src/ tests/
   ```

Small change, immediate value—prevents lint regressions from landing.
