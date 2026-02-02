---
produces: passing lint checks
---
Run linting and fix any issues.

## Goal

Get the codebase passing lint and format checks. Fix issues directly rather than just reporting them.

## Workflow

### 1. Run lint check
```bash
ruff check src/ tests/
```

If errors are found, fix them. Common fixes:
- Unused imports: remove them
- Unused variables: remove or use them
- Import sorting: ruff will auto-fix with `--fix`

For auto-fixable issues:
```bash
ruff check src/ tests/ --fix
```

### 2. Run format check
```bash
ruff format --check src/ tests/
```

If formatting issues are found:
```bash
ruff format src/ tests/
```

### 3. Verify
Run both checks again to confirm everything passes:
```bash
ruff check src/ tests/
ruff format --check src/ tests/
```

## Output

Fix all lint and format issues. When both commands pass with no errors, you're done.
