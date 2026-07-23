# v0.12.6

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.12.6 makes the boundaries around verification, release recovery, Task authority, and installation explicit. Local gates stop rather than mistake host pressure for a product failure, while an interrupted release can resume from repaired control code without changing the immutable tag being published. Tasks and candidate binaries now fail closed when the durable evidence says they no longer own—or cannot execute—the work in front of them.

## Verification stays inside a measured host budget

Local verification now accounts for the machine it runs on. The gate attributes disk and CPU use, applies repository-owned limits, and leaves a product result unproven when resource pressure makes that result unreliable (#1168).

- `uv run python scripts/resource_envelope.py` reports free space and attributes worktree builds, gate artifacts, traces, and shared caches to their owners.
- `scripts/test.py` preflights the resource envelope, limits build and test concurrency to four low-priority workers, and monitors free disk plus macOS security-process pressure throughout each phase.
- `--recover` removes only allowlisted disposable data: builds from inactive worktrees, expired gate output, and entries accepted by `uv cache prune`. Source, worktrees, traces, receipts, and SQLite state are preserved.
- Schema-3 gate evidence records child CPU, minimum free disk, build bytes, and attributed growth. `lf performance` exposes build-disk and pre-land CPU measurements alongside the existing scorecard.

## Release recovery preserves the exact tag

Publishing can now use repaired release control from current main while every artifact and product source remains pinned to the original tag. This lets `lf release run patch` resume an incomplete release instead of either changing its contents or cutting a newer tag around it (#1170, #1172).

- The tagged publisher worktree remains leased until its subprocess exits; concurrent cleanup cannot remove a checkout that is still publishing.
- Publisher commands may use `{repo}` for the synchronized control repository. `LF_RELEASE_SOURCE_REPO` identifies the leased exact-tag source tree used for packaging and deployment.
- An ambiguous Fly deploy failure triggers exact-tag production health checks before rollback. If `/healthz` reports the intended release and the root page is healthy, publication continues.
- Release health probes carry an explicit identity accepted by the production edge, avoiding the HTTP 403 returned to Python's default `urllib` client.

## Tasks act only under current direction

Historical routing and stale Project turns no longer retain authority over Task automation. Task creation and Run reservation are fenced by the immediate parent's current durable direction, and later side effects re-check the Task's current Linear Project ownership (#1167).

- Creating a Task now atomically records the Task, initial Steer, PR state, and reserved Run. A concurrent Project steer rolls the entire operation back rather than leaving partial Work.
- Automated commit, push, publication, merge, abandonment, and completion stop before side effects when a newer direction or Linear Project move supersedes the Run's authority.
- `lf task run` refuses terminal Work. A person may explicitly restart abandoned Work with `lf task recover`; completed Work requires a new Linear task.
- Failed authority checks preserve the existing Work, Run, Steer, and PR history for inspection and remediation.

## Upgrades prove they can execute durable Work

Schema compatibility alone no longer permits a candidate to become the Home launcher. `lf install preflight --json` now proves that the candidate can expand every executable lifecycle reachable from placed, open Work before installed binaries move (#1174).

- Preflight copies the shared SQLite store, applies candidate migrations to that isolated snapshot, and leaves live control state untouched.
- Wave and Project lifecycles resolve from their Wave repository; each Task phase resolves from its Task worktree.
- Validation follows nested flows, XOR routers and paths, skills, and directions through the effective builtin and repository-local catalogs.
- A missing catalog or unresolved reference rejects promotion with the exact Work, flow, catalog root, and reason. Migrations may still repair a stored reference because validation runs against the migrated snapshot.

## Operational notes

Local verification now expects at least 64 GiB of free disk and enforces repository-owned limits for builds, traces, caches, and gate artifacts. Run `uv run python scripts/resource_envelope.py --recover` when preflight identifies disposable pressure; unresolved pressure intentionally stops before product tests run.

An installation blocked by executable compatibility requires repairing the named persisted lifecycle or its catalog before promotion. The check is read-only against live state, so rerunning `lf install preflight --json` is safe after remediation.