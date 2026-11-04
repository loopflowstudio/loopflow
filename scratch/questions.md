# Decisions

2026-11-04: No separate design was supplied. The Task directive selects the existing
cleaner; implement its requirements here. Use a dependency-free Python runner
through the system interpreter on macOS so launchd never waits for uv dependency
resolution or holds uv's cache lock. Developer verification still uses `uv run`.
The unchanged shell entry point supports the already-installed job.

2026-11-04: Keep the 64/32 GiB thresholds in performance-budgets.json as the shared
disk-pressure authority. Reduce generated-output age to 1 hour/15 minutes under
pressure. Never shorten Home, cache or Trash retention. Native Task pruning stays
unchanged. Arbitrary new direct builds racing cleanup remain unsupported, as in
the existing resource recovery contract; inspect again immediately before removal.

2026-11-04: The supplied context reports the launchd job started at 09:24. Its plist
exists and the log ends in `lf monitor prune`; this run's launchctl query could not
find the service in gui/501. Treat running state as unknown; do not reinstall,
stop, or kick off machine cleanup. Deployment verification remains with delivery.
