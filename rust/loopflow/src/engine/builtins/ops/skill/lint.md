---
produces: passing lint checks
---
Run lint and format checks. Fix failures. If everything passes, stop immediately.

Lint owns formatting and static analysis only. Do not run tests or builds that
are not required by the lint command itself; gate and CI own them.

## Workflow

### 1. Find the project's lint commands

Use repo guidance first: `TESTING.md`, `README.md`, and relevant module docs. Then cross-check CI (`.github/workflows/`) so local checks match what CI enforces.

### 2. Run them

Run all lint and format checks. If everything passes, you're done — stop here.

### 3. Fix failures

Auto-fix where possible (`--fix` flags, formatters). Fix remaining issues manually. Run checks again to confirm.
