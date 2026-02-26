# 03.6: Git Sync Hardening

Make wave execution safe when multiple runs push to the same remote wave branch.

## Status

Next.

## Why this is the next phase

Phase 03.5 shipped basic pre/post step sync (fetch+rebase before steps, commit+push after). This works for the happy path but lacks recovery when concurrent writers cause conflicts or non-fast-forward pushes.

## What shipped in 03.5 (baseline)

- Pre-step: `fetch + rebase` onto `origin/{wave-branch}`
- Post-step: auto-commit + push
- Rebase failures: step skipped with warning, run continues on stale state
- Push failures: fetch+rebase retry, then hard fail

## What to build (hardening)

### Dual rebase

Pre-step should rebase onto both:

1. `origin/{wave-branch}` — pick up auxiliary run pushes
2. `origin/{default-branch}` — pick up upstream changes

Requires `target_branch` on `WaveRun` and `PendingActivation` (migration 019 staged).

### Rebase conflict recovery

Use `rebase_with_recovery` so rebase conflicts go through the dedicated rebase agent path instead of skipping the step silently.

### Push failure escalation

After the existing fetch+rebase+retry cycle, escalate stubborn push failures through `lf debug` with command output instead of hard-failing.

### Worktree reuse hardening

When reusing an existing wave worktree, run an immediate fetch/rebase instead of relying on background best-effort sync.

## Done when

- Pre-step rebases onto both wave-branch and default-branch
- Rebase conflicts route through rebase agent recovery
- Push failures escalate through debug agent after retry exhaustion
- Worktree reuse includes eager sync
- Main runs reliably ingest CI-fix or other auxiliary commits on the next step
