# LOO-955 verify (2026-11-17)

PR #1969 merged as c5504bca (2026-11-18T07:03Z). PR checks and merge_group CI green.
Push CI on c5504bca: "CI" push succeeded; "macOS on main" and "Packages (main)" still running at verify time, not reported as passed.

Checked on origin/main (built `target/debug/lf`, isolated LF_HOME, fresh git repo):
- `lf repo tracker` → "Tracker: git. Saves stay on this Machine until its remote takes them."
- `lf repo plan` → "error: unrecognized subcommand 'plan'"
- `lf repo tracker --help` lists key, journal, answer, --subscribe
- `lf config set --shared repo.tracker.linear.team T` → .lf/config.yaml `planning: {team: T, provider: linear}` (released shape, marked "until LOO-784")
- `lf config get --shared repo.tracker.linear.team` → T; `planning.provider` → linear
- git grep: no tracker_observe/TrackerObservation; no Chapter in rust/swift/docs; remaining `planning_provider_revision` columns are in released tables kept by mirror triggers
- Follow-ups exist: LOO-960 (retire released tracker tables), LOO-784 (canonical config writes), LOO-942 (wire binding names, still open)

Unrelated, pre-existing (#1813): `lf config set` prints the lifecycle with Debug (`saved; DomainAction`), rust/loopflow/src/lf/commands/setting.rs:46.
