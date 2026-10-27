# v0.14.19

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.14.19 makes the store smaller, easier to diagnose, and less likely to stall your work. Finished Process rows that nothing references are now deleted instead of kept for a week. When the store reports `database is locked`, `lf.log` and `lf doctor` name the process and binary that held the write lock. Claude Sessions keep receiving steers and Task context after `/clear`. This release changes the store format: **lf < 0.14.19 can no longer open the store (raised by: drop_steer_adapter_thread)**. Upgrade every `lf` that shares `~/.lf` together, including the macOS app.

## A smaller store with readable lock contention

Routine commands used to leave a Process row behind for every run. On Jack Heart's live store, that history reached about 950,000 rows and 10.5 GB. Lock contention was also hard to trace: when a write timed out, only the waiting process logged anything. Finding the 0.14.16 usage-receipt delete that held the lock for seconds at a time took hours of reading the store by hand. (#1718, #1724, #1726)

- **Finished Processes are deleted unless something needs them.** A finished lf Process is kept only if an agent issued it, a running parent may still read its result, or another record references it. That covers Graph results, Session history and Task shipping references. Agent Processes are always kept. Retention cleanup uses the same rule in place of the seven-day expiry, and it also removes rows left behind by an interrupted exit. On a clone of the live store, this cut Process rows from 953,191 to 104,998, and the store went from 10.5 GB to 287 MB after VACUUM. (#1718)
- **Unreferenced command outcomes are no longer history.** That includes failures. If you relied on finding an old failed command among the Process rows, it is gone once the command exits. (#1718)
- **Desktop polling no longer looks like Task execution.** File, diff, save and Graph-history queries from the Mac app now run outside the Task checkout. Keeping a Task's files open no longer counts as work in that checkout. (#1718)
- **Every `lf.log` line ends with `pid=… lf=<version> rev=<short revision>`.** Existing lock-timeout warnings from the journal, activity tracker and prune now name their process without further changes. (#1724)
- **Slow writers report themselves.** A write transaction that holds the store lock for 3 s or more logs `held the store write lock held_seconds=… call_site=…`. The call site is the innermost `loopflow::` frame. (#1724)
- **`lf doctor` adds a `contention` check.** It reads the last 24 hours of `lf.log` and groups `database is locked` failures and slow writes by binary, as `lf <version> (<rev>)`. For each binary it lists the pids, the longest hold and the call sites. Lines from older binaries appear as `unidentified (older lf)`. The check can warn but never fails. (#1724)
- **The hourly store prune waits for the write lock** instead of failing whenever another writer holds it. (#1726)

## Claude Sessions survive `/clear`

After `/clear` in an lf-launched Claude Session, every later prompt failed with `UserPromptSubmit hook error … Context callback does not belong to the selected native conversation`. Steers and the Loopflow context block stopped arriving for the rest of that process, and `lf session connect` reopened the conversation you had cleared. The cause was two records of the selected conversation that drifted apart. (#1731)

- **The new conversation gets the start context.** The per-launch `SessionStart` hook now also matches `clear`, so the conversation `/clear` opens receives its Task brief and any pending steers.
- **One record of the selected conversation.** The Session's agent-session row is now the only record. The `drop_steer_adapter_thread` migration removes the stale copy that steer adapters kept, which is what raises the store floor.
- **Resume follows the move.** When the selection moves, Loopflow appends a resume reference on the same account, so `lf session connect` opens the conversation you are in. The move is also recorded in Session history.
- **Refusals name both sides:** `Context callback names native conversation <B>; this Session selects <A>`.
- **Unchanged protections:** subagent callbacks, `PostToolUse` calls that name another conversation, and replaced or completed agent processes are still refused. Only the provider's own `SessionStart` inside the owned process can move the selection.
- **Not included:** switching conversations with `/new` or `/resume` inside one Pi process still fails as before, because Pi reports only `session_start`.

## Hotfixes without pretending to be releases

Promoting a hotfix used to require `LOOPFLOW_BUILD_PROVENANCE=release LOOPFLOW_MIGRATION_AUTHORITY=published`. That meant a second full rebuild, it recorded the hotfix as a release, and the same variables could have let a build carrying a migration draft advance the store. Promotion now asks for release authority only when the store would change. (#1730)

- **A plain `cargo build --release` promotes** when it has no migration drafts and matches the store's frontier exactly. Builds with pending drafts are still refused. An unreleased build facing a store that is ahead of it, or one it would migrate, is also refused. Only a published release may change or pass the migration frontier.
- **Builds say what they are.** Unreleased builds carry a label such as `lf 0.14.17+e9b2c2eac (hotfix e9b2c2eac on v0.14.17)`, with `development` and `dirty` variants. `lf --version`, the install receipt, promote output and `lf doctor`'s `build:` and `install-selection` lines show it. Releases still print plain `lf X.Y.Z`.
- Promote messages no longer say "published": `promoted …`, `promotion refused; every target is unchanged`.
- The `hotfix` skill now takes a single plain release build and reports the hotfix label.

## Cleaner disks and one release scheduler

A draft build killed while snapshotting the shared store left a full-size `.tmp…` directory in `~/.lf/copies`, and it stayed there for up to 30 days. On the release cron host, launchd agents from an older lf kept running `release-run` alongside the new crontab. They retried a stale v0.14.13 selection and failed every day. (#1727, #1711)

- **Abandoned snapshots are removed promptly.** The next snapshot clears them first. Hourly rotation and `lf disk clean` remove them whenever no snapshot holds the copies lock, whatever their age, and log each one to `~/.lf/logs/disk.log` as `abandoned snapshot`. If a snapshot is running, that step is skipped. (#1727)
- **`lf disk` sizes count what a removal would free.** On macOS, an APFS clone reports only its private blocks. Other platforms still count allocated blocks. (#1727)
- **`scripts/bootstrap-cron-host.sh` retires the old launchd agents.** After its preflight checks, it boots out and deletes every `~/Library/LaunchAgents/loopflow.cron.<wave>.*.plist` and prints `removed launchd job <label>`, then installs the crontab. On Linux or a fresh host, the step does nothing. `release/CRON_HOST.md` shows how to confirm which scheduler is live. (#1711)

## Operational notes

- **Store break:** lf < 0.14.19 can no longer open the store (raised by: drop_steer_adapter_thread). The release migration also carries the `forget_processes` changes, including indexes for the Process reference checks. Upgrade the CLI and the macOS app together. An older `lf` still on `PATH`, for example in a cron job or on another machine sharing the home, will refuse to open the store until you upgrade it.
- **Expect a large first cleanup.** Retention cleanup after upgrading removes Process history that the old rules kept. The 10.5 GB → 287 MB figure above includes a VACUUM. The PR's measurements came from a clone with indexes applied by hand and pruning invoked directly, not from `lf disk clean`.
- **Reading contention:** after upgrading, `lf doctor`'s `contention` line is the first place to look for `database is locked`. Lines written by lf before 0.14.19 have no `pid=… lf=…` suffix and are grouped as `unidentified (older lf)`. The check has known blind spots. A hold is reported only when it ends, so a writer that never commits goes unnamed, although the processes it blocks are still counted. Time between `BEGIN` and the first statement, and the WAL write at commit, are not measured.
- **Release cron host:** rerun `scripts/bootstrap-cron-host.sh infrastructure` from main. Afterwards, `launchctl list | grep loopflow.cron` should print nothing and `crontab -l | grep loopflow:infrastructure` should show both jobs.
- **Hotfix label source:** until a release that includes the new promotion rule is tagged, the label's `on <tag>` part comes from `git describe` against the nearest `v*` tag. Promote does not check how far the source has drifted from that tag. The hotfix skill's empty-migration-diff step remains the review point for that.
- **Abandoned-snapshot rule:** a `copies/.tmp*` directory counts as abandoned whenever nobody holds the copies lock. Anything that stages there must hold that lock.

## Small changes

- cmux workspaces opened by `lf open` are now named `<Task title> · <identifier>`, without a stage suffix, and are renamed only when the Task title changes. Before renaming, lf reads the current name. A name you set by hand in cmux is kept, unless it happens to look like one of lf's own names. Taskless Sessions no longer rename their workspace. The tab and tmux window still follow the Session name and standing. (#1716)
- The same title-first order now applies to Herdr labels, new tmux windows, `lf open` links and receipts, and the `lf task create` and Task status lines. Session names still lead with the identifier. (#1716)
- Provider hooks no longer inherit attribution as agent-issued commands. (#1718)
- `docs/troubleshooting.md`, `docs/telemetry.md`, `docs/sessions.md` and `docs/architecture-reference.md` describe the contention check, process retention, snapshot cleanup, the workspace/tab split, steer ownership and the hotfix frontier rule.