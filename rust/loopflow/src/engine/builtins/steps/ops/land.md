---
fast-path: lf ops land
produces: landed PR + rotated worktree
---
Land the current PR: rebase, lint, enable auto-merge, rotate worktree.

## API

```
lf ops land [--local] [--create-pr] [--no-lint]
lf ops wt move <worktree> <new-path>
lf ops wt create <name> [--base BRANCH]
lf ops wt list [--format json]
```

## Workflow

1. Read the error output from the failed fast-path attempt.
2. Diagnose the issue — common failures:
   - Merge conflicts: resolve them, then retry `lf ops land`
   - No PR: run `lf ops land --create-pr`
   - Lint failures: fix lint issues, then retry
   - CI failures: investigate and fix
3. After the underlying issue is resolved, run `lf ops land` to complete.
