---
produces: passing lint checks
---
Run lint and format checks. Fix failures. If everything passes, stop immediately.

## Workflow

### 1. Find the project's lint commands

Use repo guidance first: `TESTING.md`, `README.md`, and relevant module docs. Then cross-check CI (`.github/workflows/`) so local checks match what CI enforces.

### 2. Run them

Run all lint and format checks. If everything passes, you're done — stop here.

### 3. Fix failures

Auto-fix where possible (`--fix` flags, formatters). Fix remaining issues manually. Run checks again to confirm.
