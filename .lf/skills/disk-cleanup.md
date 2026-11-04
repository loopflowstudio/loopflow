Run `scripts/disk-cleanup.sh`, then report its results. Launchd runs this same
mechanical script hourly; this skill is available for an on-demand explanation.

The runner has a 30-minute wall-clock budget. Wait for its exit. A skipped run
means another resource cleanup holds the lock; report that and stop.

Read `~/Library/Logs/loopflow-disk-cleanup.json` for the latest completed run:
free bytes before and after, allocated bytes removed, candidate decisions,
failures and exit status. The text log is beside it; individual removals also
appear in `~/.lf/logs/disk.log`. Distinguish measured free-space change from
removed allocation: other processes and APFS sharing affect the former.

Report the largest retained outputs with their reasons and any incomplete
steps. Never describe a nonzero exit or an inspection failure as successful
cleanup. `scripts/disk-cleanup.sh --dry-run` inventories without deleting.

The mechanical runner and installed `lf` own deletion. Do not remove anything
else, change retained source or history, or edit the removal log.
