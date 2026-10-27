# v0.14.19

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.14.19 keeps work connected as conversations move between logins and Machines, Tasks move through delivery, and installed binaries change. It brings a Wave standup board to Desktop, restores provider context hooks, and makes store cleanup and stalled Sessions easier to diagnose. This release raises the store compatibility floor: **lf < 0.14.19 can no longer open the store (raised by: drop_steer_adapter_thread)**; **lf < 0.14.19 can no longer open the store (raised by: forget_fly_rentals)**. Upgrade every CLI and macOS app that shares the store together.

## Move the work without losing the conversation

Changing accounts or Machines used to leave native conversation history behind. Sessions now carry that history with them, and Task moves preserve the Workflow position and branch. Follow-up fixes make an open Codex conversation movable and close its old terminal cleanly. (#1720, #1740, #1746)

- **Resume on another login or Machine.** `lf --account codex=<login> resume <session>` moves an open Codex conversation to the selected login. `lf --machine <machine> resume <session>` commits and pushes the branch, carries the provider transcript, and opens the same Session on the target. Resuming from the source follows the recorded destination. Missing transcripts and unusable named accounts produce errors instead of silently starting an empty conversation. (#1720, #1740)
- **Move a Task with its work.** `lf task move <task> --machine <machine>` stops its Graph and step Processes, commits and pushes the worktree, and transfers its Workflow record and Session. Run `lf task run` on the target to run the stopped route again. Target readiness is checked before stopping or pushing; a worktree mid-merge or mid-rebase cannot move. (#1720)
- **The old Codex terminal tells you where to go.** After takeover by another terminal, account or Machine, it exits, restores terminal settings, and prints the destination notice. Its exit no longer resets the conversation's account pin to the old login. (#1746)
- **Claude context survives `/clear`.** The new conversation receives its Task brief and pending steers, later hooks keep working, and resume follows the selected conversation. This does not add support for switching conversations with `/new` or `/resume` inside one Pi process. (#1731)
- **Codex context hooks run again.** Loopflow supplies thread trust for its own hooks, restoring context, prompt, tool and stop callbacks while preserving wrapper hooks and saved host trust decisions. (#1734)

## See the Wave and keep delivery moving

Desktop puts the Wave conversation beside a standup board so the plan and the work stay visible together. Delivery fixes preserve completed Tasks through unrelated tracker edits and keep uncommitted work in branch history when syncing. (#1705, #1742, #1748)

- **One board, four sections.** Backlog, Working, Autonomous and Done show the Wave's Tasks; Working includes Available or Busy Session markers, and Done has a remembered day filter. Working Tasks appear three to a page below the board and conversation. The repository view uses the same layout across its Tasks. (#1705)
- **Arrange the view from Desktop or the CLI.** Resize, zoom or hide its parts and restore them through `Show…`. `lf desktop state` exposes their layout; `lf desktop do` runs registered actions, including board section and filter changes. Dashboards remain available through `lf desktop view` but leave the sidebar, menus and command palette. (#1705)
- **Completion survives Linear rate limits.** When a completion write has an uncertain outcome, Linear state history distinguishes an unrelated comment from an actual reopening. Unrelated edits permit an automatic retry; a real state change is adopted. State updates are delivered before comments. (#1742)
- **Sync checkpoints edits before merging.** When merging the target into a feature branch, `lf sync` commits uncommitted edits, excluding `.lf/tmp/`, before resolving conflicts. Overlaps reach the merge resolver instead of being stranded in a stash. This adds a checkpoint commit and commits previously staged edits too. `--manual` keeps edits local; main, an existing merge/rebase, and other sync strategies retain their previous behavior. (#1748)
- **Auto-Ship has one approval.** The temporary split between route and landing approval was reverted. `lf ship` or a route into `end` approves shipping through landing; `lf ship --stop` withdraws it. There is no `--recover` flag or separate landing grant. The revert also removes automatic delivery of an open PR already left at `end`: that Task reports `already at its end`, and refused completion still fails the ship run. (#1741, reverting #1736)

## Find the old process and recover the Session

Store growth, lock contention and processes left on older binaries made failures hard to explain. This release removes disposable Process history, identifies slow writers and superseded binaries, and adds supported recovery for idle Sessions and a specific broken-ancestry case. (#1718, #1724, #1729, #1747)

- **Finished Processes are deleted when nothing needs them.** Agent-issued commands, results a running parent may read, referenced Processes and agent Processes are retained. The same rule replaces seven-day retention and cleans up interrupted exits. Unreferenced outcomes, including failures, no longer remain as historical Process rows. Desktop file, diff, save and Graph-history queries also stop counting as execution in the Task checkout. (#1718)
- **Lock diagnostics name binaries and processes.** Every `lf.log` line carries PID, version and revision. Writers holding the store lock for at least three seconds log their duration and call site; `lf doctor` groups recent contention by binary. The hourly prune now waits for the write lock. This improves diagnosis, not a guarantee that contention is eliminated. (#1724, #1726)
- **See what still runs the old install.** `lf monitor ps`, `lf doctor` and promotion output identify superseded binaries and those below the store's reader floor. `lf monitor rollover --dry-run` previews eligible Sessions; `lf monitor rollover` resumes idle, detached Sessions on the selected install from saved provider history. Attached terminals, active turns, Graph steps and headless Runs are left with a reason. (#1729)
- **Repair missing ancestry without discarding history.** When an ended agent retained an attachment to an lf process whose start was never recorded, `lf monitor prune` can release that ancestry and record the repair in Session history. `lf doctor` identifies repairable cases. New attachments write their owner's start record in the same transaction, preventing this failure from recurring. (#1747)
- **Abandoned snapshots stop lingering.** The next snapshot, hourly rotation or `lf disk clean` removes abandoned `.tmp*` copies when the copies lock is free. `lf disk` reports private blocks for APFS clones rather than counting shared storage as reclaimable space. (#1727)

## Run Tasks on rented Machines with their planning access

An AWS Task run exposed gaps in region selection, Git identity and provider credentials. Rentals now carry the setup needed to claim the Task and use Linear, while the laptop keeps the refresh grant. Fly support is removed because its disk could not hold the Rust build. (#1743)

- AWS rents in the priced `AWS_REGION`; preparation copies the repository's Git identity and installs a Claude token with inference scope. GitHub repositories clone over HTTPS and bind to the Task's repository identity.
- Linear repositories receive a fresh access token lasting about 24 hours after the login is checked. The refresh grant stays on the laptop.
- Failed preparation gives the Machine back immediately. Rentals still booting an hour after admission are also returned.
- Fly's provider and stored rental records are removed, along with scope assignments pointing at those rentals. Removing the records does not destroy leftover Fly apps or end their IPv4 billing.

## Operational notes

- **Upgrade the shared store's readers together.** Both `drop_steer_adapter_thread` and `forget_fly_rentals` raise the minimum reader to 0.14.19. Update the CLI, Desktop helper and cron installations that open that store. Installing a new binary does not replace already-running processes: inspect them with `lf monitor ps`. Rollover leaves Graph runners running; compatible runners use the selected install for later steps, Desktop-bundled runners need a Desktop update, and runners below the store floor fail at their next step and need rerunning. (#1731, #1743, #1729)
- **Update Machine-move scripts.** Replace `lf machine use --task` with `lf task move <task> --machine <machine>`. Bare `lf machine use` now selects the worktree's Wave, otherwise the repository. Moves require a successful branch push. (#1720)
- **Reinstall release schedules on the cron host.** From main, run `scripts/bootstrap-cron-host.sh infrastructure`. The final bootstrap writes short crontab lines calling `.lf/cron/<wave>.<job>.sh` wrappers, verifies both schedules, then removes old launchd jobs. Wrappers pin the installed lf gate and avoid cron's 999-byte command limit. An unplaced Wave runs locally; pin its Machine before bootstrapping on multiple hosts to avoid duplicate schedules. Rerun bootstrap after path changes, and inspect `.lf/logs/release-run.log` after the next scheduled run. The supplied evidence does not establish a successful live cron cutover. (#1711, #1732, #1738)
- **Clean up remaining Fly apps explicitly.** Use `fly apps destroy` for apps left behind to stop their IPv4 charges. Automatic rental return after Task completion and the grace period was not verified live; the recorded AWS run was returned manually. (#1743)
- **Uncertain tracker writes may need a retry.** If Linear history is unavailable or truncated, or the provider is Asana or Notion, an uncertain update stays pending. `lf task complete` or `lf task sync --retry-updates` explicitly retries it. (#1742)
- **Hotfixes can use a plain release build.** A build with no migration drafts and an exact match to the store frontier can promote without claiming published-release authority. Version output, receipts and doctor identify hotfix/development provenance. Only a published release may change or pass the frontier; promotion does not establish how close the source is to its release tag. (#1730)
- **Read contention reports with their limits.** Slow holds are logged only when they end; a writer that never commits or rolls back remains unnamed. Time before the first statement and WAL writes at commit are not measured. Older log lines group as unidentified binaries. (#1724)
- **Desktop has remaining polish work.** The standup-board PR records a case where the board does not fill its part, and canceled planning Tasks may remain in Backlog until marked abandoned locally. (#1705)

## Small changes

- cmux workspaces use `<Task title> · <identifier>` and follow Task title changes, while tabs follow Session names and standing. Hand-set workspace names are preserved unless they resemble an lf-generated name. Taskless Sessions stop renaming workspaces. (#1716)
- Desktop removes the workspace's extra outer border and squares both top corners while retaining pane outlines and rounded lower corners. (#1728)
- `lf ops people` lists repository Task owners with GitHub identities resolved from commit authorship. (#1705)
- Repository skills can declare `docs:` frontmatter to include documents as `--docs` does. Builtin gate and implement guidance calls for affected suites and focused tests, one build at a time; Loopflow's test runner adds `--rust-filter`, runs Clippy after tests, and moves the shared command list to `scripts/testing.md`. (#1745)
- Graph step discovery tracks the Graph's own children, avoiding missed steps when unrelated finished Process rows are deleted. (#1746)