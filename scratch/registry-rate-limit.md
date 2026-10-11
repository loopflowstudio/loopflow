# Installation registry recovery

Jack Heart requested autonomous publication for review on 2026-10-10; no landing
without his review. Reconciled 2026-10-11 for Task lf-b978097. The bounded-retry
approach is implemented at `243ed91a36`; review and hosted acceptance remain open.

## Evidence and implemented behavior

Job 114410351377 in run 38119214480 failed at 2026-10-11T06:48:21Z:
`docker pull public.ecr.aws/docker/library/rust:trixie` returned
`toomanyrequests: Rate exceeded`. The earlier investigation obtained the direct
job log while the enclosing run was unfinished. This identifies Amazon ECR Public
anonymous throttling, but neither a numeric quota nor the particular ECR quota.

`scripts/test_task_installation.py` now reuses local images, including explicit
`--image` values. Otherwise it retries recognized throttling after 15, 30 and
60 seconds within one 300-second download budget. Manifest/auth errors fail
immediately; exhausted acquisition reports infrastructure failure before container
creation. The unconditional pull is gone. Proof execution and cleanup stay outside
retry; OS-account isolation and the separate startup deadline are preserved.

Both CI proof steps use this path on the same runner. TESTING.md documents manual
pulls to refresh mutable local tags. No credentials, registry replacement or
workflow permissions were added. Release's September 28 cold-download failure
supports retaining separate transfer and startup deadlines; it proves no recovery
from this rate limit.

## Remaining acceptance

The intended outcome remains installation verification despite registry throttling,
with infrastructure failures clearly distinguished from code failures. The six
focused tests cover transient recovery, cache reuse, persistent throttling,
non-throttling failure, timeout and the shared deadline. They simulate Docker:
actual hosted recovery is unproved, and bounded retry cannot guarantee recovery
from sustained throttling. Persistent failure must remain visible, never skip proof.

Gate/CI owns both complete disposable paths: the normal installation proof and
`--native-titles`. Publication for review remains outstanding; Jack Heart's review
is required before landing. No additional product decision is needed for this pass.

Check: `uv run pytest python/tests/test_task_installation.py -q` — prior 6 passed, reused unchanged; complete installation and native-title proofs deferred to gate/CI.
