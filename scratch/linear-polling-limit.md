# On-demand Linear checks

Jack Heart's 2026-10-11 comment `2d140be4-23a0-452c-ac69-5bd87b41d3e3` replaces the polling design and authorizes landing after checks without another review. Previous implementation/evidence: `06758e5401d6019e6cf9c65eedf4b28c430515b5`; the prior uncommitted plan is preserved at `/tmp/linear-polling-limit-before-on-demand.md` after `lf commit` failed with an empty diagnostic.

## Accepted design (2026-10-11)

One team-scoped incremental query obtains changed issues and comments. Query only teams with live Tasks, exclude locally finished/abandoned identities, and include newly changed open issues. Complete pagination before advancing the successful watermark; failed checks retain it. A SQLite team stamp throttles attempts to ten seconds across processes. A short-lived team file lock serializes acquisition, without a resident owner or hand-off.

Check before Flow steps, after agent turns, in planning commands, and before Linear writes. Watchers check immediately, then wait 30 seconds after a change or back off through 60, 120 and 240 seconds when unchanged. No per-agent timer. Local edits and execution survive acquisition errors.

Successful mutation responses settle their captured receipt directly; delete successful-write readbacks. Lost responses retain uncertainty for subsequent team observations, without replaying non-idempotent effects. Preserve saved identities, concurrent saves, conflicts, authors and history. Keep the Home-wide reset-aware cooldown and positive-rejection retry handling.

## Delete — do not maintain

The original cut (`f1ab389a961417718f06f81615a47cec7e4bb0df`) removed per-agent Linear acquisition/delivery loops, repository comment fan-out, inventory timers, per-client 429 retries, and successful mutation readbacks. Integration with #1491 preserves its repository Git transport and typed `linear::{acceptance,observations}` owners. `PlanningSync` owns one foreground thread lifetime: providers run Git acquisition/publication only; watchers select Git exchange or the slow Linear clock. The duplicate `PlanningWatch` wrapper, runtime and shutdown path are deleted, so Git watchers no longer start an unused Linear clock. Task-bound and Taskless provider exits share an on-demand check.

Deleted `pm` modules stay deleted. Delta ingestion and mutation responses use `LinearIssue`/`LinearProject` through typed acceptance, without `PmTaskRecord` or duplicate receipt storage. Existing creation identity, concurrent-save, revision-admission, conflict, prose and uncertain-response tests stay on the surviving owner. The incoming Project-adoption readback also uses its mutation payload; unchanged adoption uses its initial observation. Timer-only agent tests, including the embedded comment-runner polling scenario, are replaced by boundary request-count coverage. Comment reconnect/conflict/author coverage remains on explicit acquisition and local-save delivery.

Compression removes the intermediate delta vector and per-occurrence observation copies: pages accumulate by issue, then produce one observation from the newest fields with all comments. The pagination fixture retains newer completion against older comment-page fields. Team membership is selected once per check. One pending-Task scan runs comments and state concurrently, replacing the boolean-selected double scan at both delivery boundaries. Existing receipt locks, timeouts and error handling remain the owners of delivery behavior.

## Remaining

Implementation integrates #1491 and main through merge `dc0e4aedb0`, with the shared watcher lifecycle completed in `b078a83f3b`; preservation and sync evidence remains in `scratch/questions.md` and `fe286293fb`. No implementation work remains identified. Gate repairs are verified; delivery remains.

Commits `8dd77452ab` and `c3de7d06ee` close the missing headless coverage: four concurrent Claude/Codex Stop-hook processes plus a later process share one request; skill and command Flow steps acquire before spawning; watcher delays back off to 240 seconds and reset after changes; the migration preserves seeded Work from the preceding frontier. Hook fixtures execute generated commands through a test-binary CLI adapter, not installed providers. Existing fixtures retain pagination/comment-only changes, receipt uncertainty, shared freshness and idle-agent request counts.

Gate completed affected-suite verification with focused repairs. Live before/after measurement and rejected-request quota accounting remain unproved. Jack Heart's steer `6d169dc1-0674-4f92-917d-a94a134e715c` explicitly permits publication without live measurement while Linear is paused; the first installed check after restoring the team setting owns that measurement. No usable Doppler access token was found in the preceding implementation.

## Rollout

After installing a build with this fix, restore the paused Linear team setting (formerly `pm.linear_team`, now `linear.team` with `planning.provider: linear`) by hand in the main checkouts of loopflow, etude, kata and fantasia. Jack reports it is commented out and uncommitted. Include this instruction in the PR description; do not alter those checkouts here.

Gate review fixed missing architecture ownership for the turn hook/team stamp, classified freshness bookkeeping as non-displayed state, and updated the projection fixture for boundary-triggered delivery errors. The peer-admission fixture now uses canonical repository paths: macOS `/var` aliases otherwise exported no Task. Documentation no longer promises removed export timers or successful-write readbacks. No additional production-code change was needed.

Checks: `uv run python scripts/test.py --reuse-passing` ran architecture, fmt, Clippy, materialized `cargo nextest run --all` (2,522 passed, 21 failed, 18 skipped) and website (75 passed); `scripts/check_architecture.py` passed after map repair; materialized nextest filtered to the 21 failures with inherited `LF_*` cleared passed 20, including repaired projection/revision assertions and 17 account-selection failures; `cargo test -p loopflow --lib exchange_preserves_authority_but_never_imports_admission` passed after canonical-path repair; final `cargo fmt --all -- --check`, `cargo clippy --all-targets --jobs 4 -- -D warnings` and `git diff --check` passed. Tests used network isolation; focused reruns cleared inherited `LF_*` before the runner and closed stdin. The scheduled-release child exited after a signal in the broad run and passed the focused rerun; cause and repair remain unproved. CI owns the full candidate matrix; live measurement remains deferred to installation.
