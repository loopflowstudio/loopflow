Run `scripts/disk-cleanup.sh`, then report its results. This requires an explicit
local machine opt-in; do not install or enable a schedule implicitly. Launchd runs this same
mechanical script hourly; this skill is available for an on-demand explanation.

The runner has a 30-minute wall-clock budget. Wait for its exit. A skipped run
means another resource cleanup holds the lock; report that and stop.

Read `~/Library/Logs/loopflow-disk-cleanup.json` for the latest observation:
free bytes before and after, allocated bytes removed, candidate decisions,
failures and exit status. The text log is beside it; individual removals also
appear in `~/.lf/logs/disk.log`. Distinguish measured free-space change from
removed allocation: other processes and APFS sharing affect the former.

Check `dry_run` and timestamps; identify previews as previews. Report remaining
target shortfall separately from failures. If the receipt is missing or predates
this invocation, report the invocation's exit and log evidence without
attributing the older receipt to it.

Report the largest retained outputs with their reasons and any incomplete
steps. Never describe a nonzero exit or an inspection failure as successful
cleanup. `scripts/disk-cleanup.sh --dry-run` inventories without deleting.

The mechanical runner owns generated-output deletion and verified owner teardown.
Read its private usage history for comparable growth and visible measurement
failures; unknown measurements are not zero. General app data, Trash, source and
durable history are observation-only. Chrome-copy cleanup requires a separate
machine opt-in and never terminates Chrome. Do not remove anything
else, change retained source or history, or edit the removal log.

Successful disposable evidence retains 24h; failed or unknown evidence retains
seven days. Preserve `.cleanup-keep` and explicitly saved evidence. Native cache
caps are private machine configuration: uv 8 GiB per cache, sccache 10 GiB,
local Docker build cache 8 GiB. Preserve active cache users, Docker volumes,
containers, unpublished images and remote engines. Reinstall the same runner
after changes; never enable this utility through product packaging.

Disposable demos have two roots and a release marker; `scripts/disk-cleanup.md` owns that contract.
