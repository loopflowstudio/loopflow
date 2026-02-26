---
name: lint
description: Run lint and format checks. Fix failures. If everything passes, stop immediately.
loopflow: true
disable-model-invocation: true
---
Run lint and format checks. Fix failures. If everything passes, stop immediately.

## Workflow

### 1. Find the project's lint commands

Check `.lf/config.yaml` for a `lint:` field. If not configured, check `TESTING.md` or CI config (`.github/workflows/`).

### 2. Run them

Run all lint and format checks. If everything passes, you're done — stop here.

### 3. Fix failures

Auto-fix where possible (`--fix` flags, formatters). Fix remaining issues manually. Run checks again to confirm.
