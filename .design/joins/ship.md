# Join Summary: Infra Roadmap Items

Synthesized two roadmap proposals from forked worktrees:

## Merged Items

1. **merge-queue.md** — Enable GitHub merge queue for rebased-CI verification. Updates `lfops land` to use `--merge-queue` flag. Table stakes for Orchestra/teams support.

2. **lfd-health-checks.md** — Add health state file and supervised run mode for `lfd`. Enables observability (healthy/degraded/offline) and self-recovery with exponential backoff restarts.

Both items are complementary infrastructure improvements with no conflicts.
