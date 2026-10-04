# Dispatch deadlock and post-write planning recovery

Jack authorized autonomous implementation and landing, and explicitly combined
ETU-89's Linear/snapshot incident with the LOO-304 deadlock repair.

Implement one Session-scoped OS file lock shared by native dispatch and all
driver transfers/releases. Validate driver ownership under that lock, release
the SQLite mutex before transport, and retain the Session lock until dispatch
finishes. The lock follows the canonical database path and hashed Session ID,
so separate processes and independently opened stores share exclusion. Never
unlink lock files: an open older inode must not become a second lock owner.
No migration or copied Session history is needed.

The lock acquisition uses a monotonic OS-clock deadline, not a Tokio timer.
Codex transport retains its two-second timeout; OpenCode retains its ten-second
HTTP timeout. Driver-lock acquisition is bounded at fifteen seconds so even a
synchronous transfer on the transport's event loop cannot wait forever. Such a
transfer can fail visibly and be retried; it must not silently steal ownership
while a send's outcome remains unresolved. This is a residual synchronous API
limit, not a claim that all store calls are nonblocking.

Delete the SQLite write transaction around arbitrary native I/O. Keep exact
driver identity checks, stale-write rejection, ordered history, and unknown
transport outcomes. Both Codex dispatch sites and OpenCode use the same store
boundary, so repair it once rather than patching individual readers.

Delete post-create and post-edit/complete Wave-wide snapshot requirements.
Confirm the exact mutated issue through existing Task planning reads, retaining
ownership validation, current timestamps, failure visibility, and creation
markers. Do not refresh every Project merely to use one committed issue.

Proof: a child-process regression with an external watchdog runs a current-thread
runtime, real SQLite, a backpressured local WebSocket, and history ingestion.
It must retain history, permit unrelated database writes, time out transport,
and reject a stale driver. Isolated Linear tests must allow creation/edit when
post-write Wave reads fail and recover exact-issue confirmation failure without
duplicate issues or unintended worktree/worker creation. Gate includes affected
history, driver lifecycle, and planning suites plus formatting and clippy.

The unrelated LOO-372 settlement note and both incident analyses/stack are
preserved at `/Users/jack/.lf-retired/20261004-dispatch-planning` before scratch
cleanup. PR #1423 has merged. Publish this repair as a new PR from the supplied
checkout; `lf pr next` reports no owning Task in this Taskless checkout.

Review: retained exact driver fencing on dispatch, claim, release and exit;
kept Project association reconciliation on the exact post-write issue; did not
advance Wave snapshot freshness from an issue-only read.

Checks: final affected Rust suite passed (31 tests; one subprocess entry point
ignored by design); `cargo clippy --all-targets -- -D warnings`, `cargo fmt
--check`, and `uv run python scripts/check_architecture.py` passed. Sync with
main was conflict-free; CI owns the full matrix.
