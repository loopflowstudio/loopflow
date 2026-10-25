# v0.14.16

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.14.16 lets you hand off work and leave it running. Bare `lf task run` now returns once the Graph is running, and `lf task interrupt` stops a running Graph. A Task can also run on a free Machine from a pool or on a rented RunPod pod that is deleted after its history is copied back. The rest of the release removes the follow-up work that came after 0.14.15. The store shrinks from gigabytes to hundreds of megabytes, so `lf install` takes seconds. Landings and releases stop only when a person removes the merge request. Graph syncs no longer discard Task commits, and Sessions whose terminal closed can be reconnected.

## Start work and leave it running

A Task's autonomous run used to occupy the shell or agent tool that launched it until the Graph finished. Now the default command returns as soon as the run starts, and the run can be stopped. (#1675, #1678, #1681)

- **Bare `lf task run` returns once running.** After the first Graph process registers, or the Process for a single-skill edge, it prints `running as Graph process <id>` with a `follow:` command and a `log:` path, then returns the prompt. Retries and the arrival write continue in the background. Output goes to `~/.lf/logs/graph-<id>.log`. If the run is refused before it registers, the refusal still appears on your stderr. `-b` still waits for the end, and `-i` still stays attached. Without an execution owner, a bare run stays in the foreground as before. (#1675)
- **`lf task status` shows each run's log** next to its `Graph:` or `Process:` line. Desktop and the builtin operator skills use the plain command, and their docs no longer recommend backgrounding `task run` yourself. (#1675)
- **`lf task interrupt` stops the Graph.** A Graph checks for an interrupt before each step and about once a second while an agent step runs. It kills the agent step right away, lets an operation step such as a land finish, and starts no further steps. The Graph ends `Interrupted` with exit 130, and the Task run prints `Task run interrupted; not starting <graph> again.` (#1678)
- **The interrupt receipt reports the result.** It reads `stopped Graph <name> at <step>`. If nothing is live, it records nothing and prints `<issue> has no live run; nothing to stop.` Scheduled automation is turned off in both cases. JSON receipts gain `target` and a `no_live_run` kind. A native takeover that exits 130 or 3 without a recorded interrupt still pauses the Graph as `Waiting`. (#1678)

## Run a Task on another Machine, then return it

A Task can now run on a Machine from a pool you own or on a rented RunPod pod under a price limit. When a rented pod's work drains, its history is copied back before the pod is deleted. (#1653)

- **Pools.** Add owned Machines with `lf machine add --pool` (plus `--cost-usd` and `--expires-at`). `lf task run --pool P` uses the first Machine in P that holds no unfinished Task and has not expired. There is no queue.
- **On-demand rentals.** An on-demand sets a provider template, a limit on concurrent Machines and a maximum hourly price. `--rent R` checks the limit, stock and price, records the rental, and makes a single create call. It then waits for SSH, connects the Machine, forwards GitHub, installs Claude, clones the repository and hands off the Task. If the create call returns no answer, the request is not resent; the rental stays `unconfirmed` until the inventory resolves it. `--quote` prints where the Task would run and its maximum cost without running anything.
- **Guardian and give-back.** Each pod starts a guardian that deletes the pod at its deadline, or sooner when the budget runs out. Give-back happens when the Task work drains, 15 minutes before the hard deadline, or when you run `lf machine end`. Before deleting the pod, it copies the pod's records through `lf history export`. Give-back is refused if the copy fails or the pod holds uncommitted or unpushed work. `--force` overrides only the unpushed-work refusal. Owned Machines are never deleted.
- **Copied history stays readable.** `lf session list` and `lf task status` mark copied Sessions with "from <machine>", and `lf history show <session>` works after the pod is gone. Copies are kept separate from this Machine's own records.
- **Claude on rented Machines** uses one long-lived subscription token per account, minted with `claude setup-token` or read with `--file`. Only `sk-ant-oat` tokens are accepted. Account status shows the token's expiry. `lf -b task run … --rent` refuses before renting if no account has a current token.

## A smaller store and faster installs

On a clone of Jack's store, the database went from 9.8 GB to 550 MB, and `lf install` preflight finished in about 5 seconds instead of several minutes. Loopflow now stores the facts it reads back. The full provider stream remains in each run's `events.jsonl`. (#1666, #1667)

- **Recorded facts only.** History keeps attempts, account selection, provider sessions, usage, turn outcomes and silences longer than 600 s. Usage is stored as one row per provider turn. Provider hooks, `__` callbacks, `--context`/`--explain` previews and Desktop file and diff polls no longer create Process rows.
- **Existing stores shrink automatically.** An hourly prune, which also runs in `lf disk clean`, trims old captures and compacts old usage rows. It removes finished, unreferenced inspection Processes after 7 days and closes Processes whose OS process is gone. It then runs `VACUUM`. `lf disk clean` reports what it removed.
- **Install copies are APFS clones.** The pre-migration backup and validation copy are clones, and the backup is removed once the install settles.
- **Readers use the stored facts.** `lf history usage --weekly` reads recorded silences; on Jack's store it fell from 53 minutes to under 2 minutes on a debug build. `lf history show --events` prints `events.jsonl`, or only the stored facts after rotation removes the file.

## Landings and releases stop only for a person

Shipping v0.14.15 exposed three ways a removed merge request could be misread. A release re-armed after a person dequeued it. Loopflow's own CI repair disarm looked like a person's withdrawal. A GitHub merge-queue eviction left a landing stalled with nothing to repair. Landing and release now share one decision. (#1668, #1673, #1679, #1680)

- **A person's dequeue or disabled auto-merge stops the operation, with the person and time named.** A landing blocks with a message such as `… removed #248 from the merge queue at …; run lf arm to resume landing`. A release stops with `ManualMergeRemoval` and resumes when you run `lf release run` again. A stopped scheduled release cannot count as a clean qualifying release.
- **Evictions and Loopflow's own disarms go to CI repair.** A queue eviction appears as a failing `merge-queue (<reason>)` check linked to the merge-group run within the eviction window. A disarm during a recorded repair, or an unexplained removal with green checks, appears as `merge-request (repair-disarmed|unexplained)`. ci-fix handles both: for a runner fault or disarm, it confirms that the head's checks pass and runs `lf arm`.
- **Release no longer re-arms during its wait.** A failing observation sends the release to CI repair, so a green head with a red merge result reaches ci-fix and no longer keeps re-queuing.
- **Release notes come from origin.** `lf release notes` collects changes up to `origin/<default>` and writes into the checkout you run it from. The packaged-app resource check no longer hangs while Loopflow Desktop is running.

## Keep your work through sync and reconnect

These fixes affect Task branches and lost terminals. (#1672, #1676, #1665, #1674)

- **Graph sync no longer resets authored branches.** `lf sync` creates its plan once while your edits are present and runs that plan. A branch with its own commits always merges its target. Reset applies only to branches with no commits of their own, or persistent branches whose commits have already landed. The `generated_only` class, which reset `checkpoint:`/`wip:` branches, is removed. (#1672, #1676)
- **Stranded edits stop the Task run.** Recovery stashes are named `lf checkout update: caller edits in <path>`. If a Graph fails while one is present, `lf task run` stops instead of retrying and prints the `git stash apply --index <sha>` command. (#1672)
- **Detached Sessions can be reconnected.** A Session whose agent is live but has no viewer now appears as `detached` and offers **Reconnect** instead of "Move here". It can be resumed. This is a new wire value in Rust, Swift and the DTO fixtures. (#1674)
- **`lf connect <TASK-ID>`** opens that Task's conversation; an exact Session ID still takes precedence. Unknown IDs resolve faster: on Jack's store, lookup fell from 26 s to under 1 s. (#1674)
- **`lf resume` in an unassociated worktree** reopens the unfinished interactive conversation you opened there most recently. The earlier error remains when no such conversation exists or with `--fresh`. (#1665)

## Desktop that keeps up

Dashboards from 0.14.15 re-rendered much more than they displayed and failed to save some changes. (#1660, #1664)

- Sidebar scrolling, row and pane hover, and Session refreshes no longer re-render every row, tile and terminal. Writes that change nothing are skipped.
- Creating a pinned Dashboard and saving a pane layout work again. Desktop had passed a `--json` flag those commands rejected.
- Zooming a Working Tasks tile keeps Working Tasks in the breadcrumb, so Back returns to it. A tile offers Start only for an unstarted Task. The New Dashboard `+` appears only while you hover over the Dashboards section.
- `lf desktop` and `lf open` target the Desktop app they run inside. Desktop sets `LF_DESKTOP` to its bundle identifier. From an ordinary terminal, `LF_DESKTOP=com.loopflow.mac.dev lf desktop state` reaches the Dev build. (#1664)
- Dev builds add **Debug › Flash Re-renders** (⌃⌥⌘R).

## Operational notes

- **Compatibility.** This release reports no store compatibility break, so older lf versions keep working with the migrated store. The release migration adds pools, on-demands, rentals and `machine_records`. (#1653)
- **First cleanup may be slow.** The first hourly prune or `lf disk clean` on a large store rewrites many rows and vacuums. On Jack's 9.8 GB clone, it took about four minutes; later passes take milliseconds. Captures younger than 24 h are skipped until the next pass. After 7 days, task work listings and `lf ps` history lose unreferenced inspection Processes. (#1666, #1667)
- **Scripts that relied on bare `lf task run` blocking** should add `-b`. Rust tests that read state written by a run now use `lf -b task run`. (#1675, #1681)
- **0.14.14 cron jobs upgrade in place.** On first contact, `lf wave cron list` and scheduled runs rewrite Flow-shaped plists, receipts and obligations to the Graph shape without changing labels or activation times. These shims are marked for removal (LOO-574). Cron accounting reads only the current repository's records. (#1668)
- **`LF_DESKTOP=1` still selects the installed app** for released Desktop builds. LOO-571 removes that fallback. (#1664)
- **`lf desktop arrange` accepts only `{"order": [...], "hidden": [...]}`.** Input that includes `mode` is rejected. (#1660)
- **Branches made only of `checkpoint:`/`wip:` commits** now get a merge commit from main instead of a reset. Stashes created before this release lack a path, so `lf task run` will not detect them; find them with `git stash list`. (#1672)
- **Known gap:** a person who dequeues in the few seconds between a repair's `lf arm` and the end of its Process can be read as Loopflow's own disarm, and ci-fix may re-arm. (#1680)
- **Verify after install:** these behaviors are unit- or integration-tested, but no end-to-end run is recorded: a real merge-queue eviction going through ci-fix, reconnecting a real detached Codex Session, and the first scheduled run of a 0.14.14 cron job after the upgrade. (#1668, #1674, #1679)

## Small changes

- `swift-test` fails when a test leaks a continuation and names that test. The Desktop test that hung the merge queue is fixed, and a reused barrier key now fails quickly instead of hanging. (#1683)
- The task-installation proof waits up to 120 s for Docker before reporting an infrastructure fault. (#1679)
- `capture-tasks` and `launch-plan` identify open PRs that touch the same files and leave those PRs' open decisions unresolved. (#1682)
- The README starts with what `lf` does for you, then gives a small first exercise, requirements, the files and services it touches, and uninstall steps. Reference material moved into `docs/`, including a new *Own and rent Machines* section. The install command is now `curl -fsSL https://loopflow.studio/install.sh | sh`. (#1671)
- An agent turn that ends `Interrupted` exits 130, and headless `lf run` preserves that code. (#1678)