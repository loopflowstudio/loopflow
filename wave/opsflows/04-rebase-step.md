# 04: `lf rebase` step with fast-path

**Finish line:** `lf rebase` runs `lf ops rebase` as fast-path. No agent on the happy path. Agent spins up only on conflicts.

## What to build

**`lf rebase` step:**

```yaml
---
fast-path: lf ops rebase
---
```

Step prompt for conflict resolution. Lists the API:

```
## API

lf ops rebase [--onto BRANCH]   # git rebase onto main, push
```

Agent reads conflicting files, resolves, continues rebase. If ambiguous, surfaces to user (interactive) or writes to `scratch/questions.md` (headless).

**No changes to `lf ops rebase`** — it's already mechanical and correct. This sprint just wraps it in a step with fast-path so it's composable in flows and resilient to conflicts.

## Done when

```bash
lf rebase   # no conflicts: ops speed, no agent
lf rebase   # conflicts: agent resolves them
lf rebase   # ambiguous conflict: agent asks user (interactive) or notes it (headless)
```
