# 03.6: Git Sync Hardening

Make wave execution safe when multiple runs push to the same remote wave branch.

## Status

Next.

## Why this is the next phase

Phase 03.5 removed CI-fix special casing and lets auxiliary runs operate as normal wave runs. That unifies activation behavior, but branch sync behavior still assumes a single writer in many paths.

## What to build

### Pre-step sync for every flow step

Before each step:

1. Rebase onto `origin/{wave-branch}` to pick up auxiliary run pushes
2. Rebase onto `origin/{default-branch}` to pick up upstream changes

Use `rebase_with_recovery` so rebase conflicts go through the dedicated rebase agent path.

### Post-step sync for every flow step

After each step:

1. Auto-commit dirty changes
2. Push with recovery:
   - Try push
   - On non-fast-forward: fetch + rebase-with-recovery + push retry
   - If still failing: escalate through `lf debug` with command output

### Worktree reuse hardening

When reusing an existing wave worktree, run an immediate fetch/rebase instead of relying on background best-effort sync.

## Done when

- Step execution loop wraps every step with pre-step and post-step sync
- Pushes no longer fail permanently on non-fast-forward due to concurrent auxiliary writes
- Main runs reliably ingest CI-fix or other auxiliary commits on the next step
- Failure handling is unified: rebase conflicts via rebase agent, stubborn push failures via debug agent
