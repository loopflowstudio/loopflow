# Workspace verification disk decision

2026-09-30 · LOO-303 · Current participant: Jack Heart

The caller requested permission to remove two recent inactive build caches after
supported recovery retained them and reported a busy uv cache. No approval from
Jack Heart has been received in this review. No cache was removed here.

Fresh read-only `uv run python scripts/resource_envelope.py --json` reports
32,595,652,608 bytes free (30.4 GiB), below TESTING.md's 32 GiB reserve. It reports
both proposed worktrees inactive. `du -sh` confirms their target directories:

- `/Users/jack/src/loopflow.keep-account-status-live-and/target`: 3.9 GiB.
- `/Users/jack/src/loopflow.make-the-deepest-cuts-first/target`: 2.7 GiB.

TESTING.md's automatic cleanup retains builds changed within 24 hours. The
pending decision is an exception for only these two reproducible target
directories. Source, worktree metadata, scratch evidence and other caches are
outside the proposed deletion. Existing staged and unstaged work is untouched.

If Jack approves, recheck activity immediately before cleanup, remove only the
approved inactive target directories, and rerun resource preflight. Success is
measured free space above the reserve; approximately 37 GiB is an estimate,
not a reservation against concurrent builds. The caller can then resume its
requested Rust and Swift checks. No test result or merge readiness is established
by this inspection. If approval is withheld, retain the caches and disk blocker.
