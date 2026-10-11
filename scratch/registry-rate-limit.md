# Installation registry recovery

Jack Heart requested autonomous publication for review on 2026-10-10 and later
approved PR #1529 for landing (steer `ff9c03dd-7587-4c14-afd8-447d99252aaf`).
Reconciled 2026-10-11 for Task lf-b978097. The bounded-retry approach is
implemented at `243ed91a36`; hosted acceptance remains open.

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

Both disposable paths passed locally: nine normal installation checks and the
`--native-titles` promotion/retry/preservation proof. Both reused the cached Rust
1.99.0 image and removed their containers. PR #1529 is published and approved for
landing. Delivery remains outside this gate step.

Gate review found no code changes needed: cache hits avoid registry traffic,
download attempts and backoff share one deadline, and only image acquisition is
retried. The stale review-only boundary above and in Infrastructure memory was
corrected from the supplied approval steer. A published checkpoint still defers
hosted tests while scratch remains; candidate/merge-queue CI owns the full matrix.

Check: `uv run python scripts/test.py --base 21dae527b4 --reuse-passing` — architecture passed; Python 394 passed (including all 6 registry cases), 19 skipped, 1 nested-macOS-sandbox refusal; `uv run pytest python/tests/test_desktop_performance.py::test_checkout_observation_preserves_read_boundary_and_detects_changes -q` — passed through its own sandbox; `uv run ruff check scripts/test_task_installation.py python/tests/test_task_installation.py` and `uv run ruff format --check scripts/test_task_installation.py python/tests/test_task_installation.py` — passed; `uv run python scripts/test_task_installation.py` and the same command with `--native-titles` — passed; full hosted matrix and real throttling recovery remain with CI.
