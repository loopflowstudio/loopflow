# On-demand Linear checks

Jack Heart's 2026-10-11 comment `2d140be4-23a0-452c-ac69-5bd87b41d3e3` replaces the polling design and authorizes landing after checks without another review. Previous implementation/evidence: `06758e5401d6019e6cf9c65eedf4b28c430515b5`; the prior uncommitted plan is preserved at `/tmp/linear-polling-limit-before-on-demand.md` after `lf commit` failed with an empty diagnostic.

## Design

One team-scoped incremental query obtains changed issues and comments. Query only teams with live Tasks, exclude locally finished/abandoned identities, and include newly changed open issues. Complete pagination before advancing the successful watermark; failed checks retain it. A SQLite team stamp throttles attempts to ten seconds across processes. A short-lived team file lock serializes acquisition, without a resident owner or hand-off.

Check before Flow steps, after agent turns, in planning commands, and before Linear writes. Watchers alone own a slow timer, starting at 30 seconds and backing off to a few minutes when unchanged. No per-agent timer. Local edits and execution survive acquisition errors.

Successful mutation responses settle their captured receipt directly; delete successful-write readbacks. Lost responses retain uncertainty for subsequent team observations, without replaying non-idempotent effects. Preserve saved identities, concurrent saves, conflicts, authors and history. Keep the Home-wide reset-aware cooldown and positive-rejection retry handling.

## Delete — do not maintain

The original cut (`f1ab389a961417718f06f81615a47cec7e4bb0df`) removed per-agent Linear acquisition/delivery loops, repository comment fan-out, inventory timers, per-client 429 retries, and successful mutation readbacks. Integration with #1491 preserves its repository Git transport and typed `linear::{acceptance,observations}` owners. `PlanningSync` now runs only Git acquisition/publication; watchers hold that Git connection plus the slow Linear clock. Task-bound and Taskless provider exits share an on-demand check.

Deleted `pm` modules stay deleted. Delta ingestion and mutation responses use `LinearIssue`/`LinearProject` through typed acceptance, without `PmTaskRecord` or duplicate receipt storage. Existing creation identity, concurrent-save, revision-admission, conflict, prose and uncertain-response tests stay on the surviving owner. The incoming Project-adoption readback also uses its mutation payload; unchanged adoption uses its initial observation. Timer-only agent tests, including the embedded comment-runner polling scenario, are replaced by boundary request-count coverage. Comment reconnect/conflict/author coverage remains on explicit acquisition and local-save delivery.

Compression removes the intermediate delta vector and per-occurrence observation copies: pages accumulate by issue, then produce one observation from the newest fields with all comments. The pagination fixture retains newer completion against older comment-page fields. Team membership is selected once per check. One pending-Task scan runs comments and state concurrently, replacing the boolean-selected double scan at both delivery boundaries. Existing receipt locks, timeouts and error handling remain the owners of delivery behavior.

## Remaining

Implementation integrates #1491 at `b0788f4efb`; preservation and sync evidence remains in `scratch/questions.md` and `fe286293fb`. Gate and delivery remain.

Gate owns independent-process contention, native turn-hook execution, watcher backoff and migration from the released frontier. Fixtures cover pagination/comment-only changes, receipt uncertainty, shared freshness and idle-agent request counts. Live before/after measurement and rejected-request quota accounting remain unproved; no usable Doppler access token was found in the preceding implementation.

## Rollout

After installing a build with this fix, restore the paused Linear team setting (formerly `pm.linear_team`, now `linear.team` with `planning.provider: linear`) by hand in the main checkouts of loopflow, etude, kata and fantasia. Jack reports it is commented out and uncommitted. Include this instruction in the PR description; do not alter those checkouts here.

Checks: `cargo test -p loopflow --lib` with filters `linear::team_changes`, `linear_on_demand`, and `task_comments_read_and_publish_without_starting_work` passed (7 tests; inherited `LF_*` cleared, stdin closed); `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed. Prior integration checks remain at `fe286293fb:scratch/linear-polling-limit.md`; gate/CI own broader acceptance and installed validation.
