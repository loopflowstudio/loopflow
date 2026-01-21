---
status: proposed
area: infra
created_at: 2026-01-21T02:35:00
---

# Pre-commit hooks for fast local feedback

Add pre-commit hooks that catch common issues before they reach CI. Developers get immediate feedback on lint errors, formatting issues, and basic sanity checks without waiting for GitHub Actions.

## Scope

**Included:**
- `.pre-commit-config.yaml` with ruff (lint + format)
- Basic file hygiene checks (trailing whitespace, end-of-file, no debug statements)
- `pyproject.toml` integration (hooks respect existing ruff config)
- Documentation in CLAUDE.md about running/skipping hooks

**Not included:**
- Type checking (mypy/pyright) - too slow for pre-commit
- Test running - that's CI's job
- macOS-specific hooks for Maestro (Swift has different tooling)
- Security scanning - separate roadmap item

## Approach

Use the `pre-commit` framework. Configure hooks that run in under 2 seconds for typical commits.

```yaml
# .pre-commit-config.yaml
repos:
  - repo: https://github.com/astral-sh/ruff-pre-commit
    rev: v0.9.0
    hooks:
      - id: ruff
        args: [--fix]
      - id: ruff-format

  - repo: https://github.com/pre-commit/pre-commit-hooks
    rev: v5.0.0
    hooks:
      - id: trailing-whitespace
      - id: end-of-file-fixer
      - id: check-yaml
      - id: check-added-large-files
```

Add `pre-commit` to dev dependencies:

```toml
# pyproject.toml
[project.optional-dependencies]
dev = ["pre-commit>=4.0.0"]
```

Install on clone:

```bash
uv sync --extra dev
uv run pre-commit install
```

The `lf init` command should offer to install pre-commit hooks during setup.

**Why ruff over black/isort:** Already using ruff for linting. Single tool means single version to track, faster execution.
