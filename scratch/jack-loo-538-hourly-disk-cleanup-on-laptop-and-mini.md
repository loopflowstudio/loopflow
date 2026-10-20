# LOO-538: Loopflow footprint, budget and hourly cleanup

Status: accepted direction from Jack's 2026-10-20 steers; implemented, awaiting gate.

## Outcome

Jack's laptop hit 98% full. Three layers (Jack, 2026-10-20):

1. `lf disk` — product command, safe for any user.
   - `lf disk [--json]`: footprint by category (database, backups, copies,
     accounts, captures (`runs/`), logs, runtime, other) plus every worktree of each Task repository
     (checkout and generated output), with Home usage against its budget.
   - `lf disk clean [--dry-run] [--json] [--keep-days N]`: remove merged/closed/
     stale clean worktrees (existing `lf wt prune` rules, every Task repository),
     completed Tasks' build output (existing `prune_builds`, no root/byte caps),
     dead process receipts, every DB backup but the newest per database,
     draft store copies (`copies/`, from #1612; never the selected one),
     captures and logs older than N days (default 7), then rotate to budget.
   - `lf disk rotate [--quiet]`: rotate oldest backups, copies, captures and logs until
     Home fits `disk_budget` (global `~/.lf/config.yaml`, e.g. `disk_budget: 20G`;
     default 20G). Runs automatically: starting a Session capture spawns a detached
     `lf disk rotate` at most once an hour (stamp `runtime/disk-rotation.stamp`).
   - Never removed: the database and its sidecars, accounts/, the newest backup
     per database, anything modified within 24h, anything a process holds open
     (lsof), dirty or open-PR worktrees, unfinished Tasks' output.
   - Every removal is logged to `~/.lf/logs/disk.log` with category, path and bytes.
2. Remote-branch cleanup — LOO-539, not here. The script has a marked slot.
3. `scripts/disk-cleanup.sh` — Jack's machines only. Hourly launchd agent
   (`scripts/disk-cleanup.sh install`). Calls `lf disk clean`, the LOO-539 slot,
   then Jack-approved machine caches: uv cache, unavailable simulators,
   `~/Library/Caches` files untouched for 7 days, idle cargo `target/` outside
   lf worktrees, Trash (FDA note). Logs to `~/Library/Logs/loopflow-disk-cleanup.log`.

## Delete — do not maintain

Nothing replaced: `lf wt prune` and `lf monitor prune` stay as the per-repo/
per-process primitives `lf disk clean` composes.

## Remaining

- Install the launchd agent on the laptop and mini-heart after landing (the plist
  points at the main checkout's script).

Check (after compress; realign rebuilt and confirmed `lf disk clean|rotate --help` name store copies under a disposable LF_HOME): `cargo test -p loopflow --lib disk::` 4 pass; clippy clean; dry runs on an APFS clone of
Jack's ~/.lf: footprint 107.8G (55s), clean would free 13.1G, rotation floor 28.5G.
