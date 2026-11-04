Run `scripts/disk-cleanup.sh`, then report its results. Launchd runs this same
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

The mechanical runner and installed `lf` own deletion. Do not remove anything
else, change retained source or history, or edit the removal log.
