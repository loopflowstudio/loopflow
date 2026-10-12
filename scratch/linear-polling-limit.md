# On-demand Linear checks

Jack Heart's 2026-10-11 comment `2d140be4-23a0-452c-ac69-5bd87b41d3e3` replaces the polling design and authorizes landing after checks without another review. Previous implementation/evidence: `06758e5401d6019e6cf9c65eedf4b28c430515b5`; the prior uncommitted plan is preserved at `/tmp/linear-polling-limit-before-on-demand.md` after `lf commit` failed with an empty diagnostic.

## Design

One team-scoped incremental query obtains changed issues and comments. Query only teams with live Tasks, exclude locally finished/abandoned identities, and include newly changed open issues. Complete pagination before advancing the successful watermark; failed checks retain it. A SQLite team stamp throttles attempts to ten seconds across processes. A short-lived team file lock serializes acquisition, without a resident owner or hand-off.

Check before Flow steps, after agent turns, in planning commands, and before Linear writes. Watchers alone own a slow timer, starting at 30 seconds and backing off to a few minutes when unchanged. No per-agent timer. Local edits and execution survive acquisition errors.

Successful mutation responses settle their captured receipt directly; delete successful-write readbacks. Lost responses retain uncertainty for subsequent team observations, without replaying non-idempotent effects. Preserve saved identities, concurrent saves, conflicts, authors and history. Keep the Home-wide reset-aware cooldown and positive-rejection retry handling.

## Delete — do not maintain

No remaining predecessor paths to delete. Removed:

- `PlanningSync`, `repeat_sync`, `acquire_poll`, repository comment fan-out and background inventory refresh; their timer-only tests move to on-demand/watch behavior.
- `TaskInput::start_planning_sync` and the agent-owned synchronization thread.
- Successful state/field/create/attachment/order write readbacks. Reconcile mutation payloads through existing receipt owners; retain uncertain outcomes.
- Per-client 429 retry loops (already removed).

## Remaining

Implementation and deletion cuts are complete. Gate owns remaining acceptance: independent-process contention, native turn-hook execution, watcher backoff and migration from the released frontier. Focused fixtures cover pagination/comment-only changes, receipt uncertainty, shared freshness and idle-agent request counts. Live before/after measurement and rejected-request quota accounting remain unproved; no usable Doppler access token was found in the preceding implementation.

## Rollout

After installing a build with this fix, restore `pm.linear_team` by hand in the main checkouts of loopflow, etude, kata and fantasia. Jack reports it is commented out and uncommitted. Include this instruction in the PR description; do not alter those checkouts here.

Checks: `cargo test -p loopflow --lib --no-run`, network-isolated planning (56), Linear (30), context (67), `cargo fmt --check`, and `cargo clippy --all-targets -- -D warnings` passed; final scale fixtures passed (350 terminal Tasks: zero idle requests over 60 seconds, one query at turn end; four concurrent checks: one query). Gate owns remaining acceptance above; CI owns the full matrix.
