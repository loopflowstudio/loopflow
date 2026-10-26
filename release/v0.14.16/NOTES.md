# v0.14.16

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.14.16 lets you hand off work and leave it running. Bare `lf task run` now returns once the Graph is running, and `lf task interrupt` stops it. A Task can also run on a free Machine from a pool you own, or on a rented RunPod pod that is deleted after its history is copied back. Loopflow now costs less while you wait: the store shrinks from gigabytes to hundreds of megabytes, idle relays stop using CPU, and landings stop only when a person removes the merge request. Agents can now script and read back Loopflow Desktop through `lf desktop`, and a feature Task opens with several visions to choose from instead of a single plan. This release changes the store format: `lf < 0.14.16 can no longer open the store (raised by: designated_panes, session_turn_events)`. Desktop now requires macOS 26.

## Start work and leave it running

A Task's autonomous run used to occupy the shell or agent tool that launched it until the Graph finished. Now the default command returns as soon as the run starts, and you can stop the run. (#1675, #1678, #1681)

- **Bare `lf task run` returns once running.** It returns after the first Graph process registers, or after the Process starts for a single-skill edge. It prints `running as Graph process <id>` with a `follow:` command and a `log:` path, then gives back the prompt. Retries and the arrival write continue in the background, and output goes to `~/.lf/logs/graph-<id>.log`. A run refused before it registers still prints the refusal on your stderr. `-b` still waits for the end, and `-i` still stays attached. Without an execution owner, a bare run stays in the foreground as before. (#1675)
- **`lf task status` shows each run's log** next to its `Graph:` or `Process:` line. Desktop and the builtin operator skills use the plain command. (#1675)
- **`lf task interrupt` stops the Graph.** A Graph checks for an interrupt before each step, and about once a second during an agent step. It kills an agent step immediately, lets an operation step such as a land finish, and starts no further steps. The Graph ends `Interrupted` with exit 130, and the Task run prints `Task run interrupted; not starting <graph> again.` (#1678)
- **The interrupt receipt reports what happened.** It reads `stopped Graph <name> at <step>`. If nothing is live, nothing is recorded and it prints `<issue> has no live run; nothing to stop.` Scheduled automation is turned off either way. JSON receipts gain `target` and a `no_live_run` kind. A native takeover that exits 130 or 3 without a recorded interrupt still pauses the Graph as `Waiting`. (#1678)

## Run a Task on another Machine, then return it

A Task can run on a Machine from a pool you own, or on a rented RunPod pod within a price limit. When a rented pod's work drains, its history is copied back before the pod is deleted. (#1653)

- **Pools.** Add owned Machines with `lf machine add --pool`, plus `--cost-usd` and `--expires-at`. `lf task run --pool P` uses the first Machine in P that holds no unfinished Task and has not expired. There is no queue.
- **On-demand rentals.** An on-demand sets a provider template, a limit on concurrent Machines and a maximum hourly price. `--rent R` checks the limit, stock and price, records the rental and makes a single create call. It then waits for SSH, connects the Machine, forwards GitHub, installs Claude, clones the repository and hands off the Task. If the create call returns no answer, it is not resent; the rental stays `unconfirmed` until the inventory resolves it. `--quote` prints where the Task would run and its maximum cost, without running anything.
- **Guardian and give-back.** Each pod starts a guardian that deletes the pod at its deadline, or sooner if the budget runs out. Give-back runs when the Task's work drains, 15 minutes before the hard deadline, or when you run `lf machine end`. Before the pod is deleted, its records are copied through `lf history export`. Give-back is refused if the copy fails or the pod holds uncommitted or unpushed work; `--force` overrides only the unpushed-work refusal. Owned Machines are never deleted.
- **Copied history stays readable.** `lf session list` and `lf task status` mark copied Sessions "from <machine>", and `lf history show <session>` still works after the pod is gone. Copies are stored separately from this Machine's own records.
- **Claude on rented Machines** uses one long-lived subscription token per account, minted with `claude setup-token` or read with `--file`. Only `sk-ant-oat` tokens are accepted, and account status shows the token's expiry. `lf -b task run … --rent` refuses before renting if no account has a current token.

## A smaller store, faster installs and quiet idle relays

On a clone of Jack's store, the database went from 9.8 GB to 550 MB, and `lf install` preflight took about 5 seconds instead of several minutes. Loopflow now stores only the facts it reads back; the full provider stream stays in each run's `events.jsonl`. Later in the cycle, the remaining background costs on large stores were removed as well. Each relay now checks only its own viewers, and the history queries that Desktop polls finish in well under a second. (#1666, #1667, #1690, #1692)

- **Recorded facts only.** History keeps attempts, account selection, provider sessions, usage, turn outcomes, and silences longer than 600 s. Usage is stored as one row per provider turn. Provider hooks, `__` callbacks, `--context`/`--explain` previews and Desktop file and diff polls no longer create Process rows. (#1666, #1667)
- **Existing stores shrink automatically.** An hourly prune, also run by `lf disk clean`, trims old captures and compacts old usage rows. It removes finished, unreferenced inspection Processes after 7 days, closes Processes whose OS process is gone, then runs `VACUUM`. `lf disk clean` reports what it removed. (#1666)
- **Install copies are APFS clones.** The pre-migration backup and the validation copy are clones, and the backup is removed once the install settles. (#1666)
- **Idle relays stop scanning the machine.** Previously every running `lf` relay searched the whole store for unviewed Sessions, which cost about 4 s of CPU per pass. Now each relay times its own viewers in memory. When the last viewer has been gone for `session.end_detached_after`, it ends itself if its Sessions are waiting for a person. If a turn or Graph still holds it, it rechecks at most every 5 minutes. On Jack's live store, the check fell from 4.2 s to 44 ms and found the same owners. A new index on Sessions by execution owner makes the lookup direct. (#1690, #1692)
- **The Desktop Work feed stops churning.** On Jack's store, the `lf history` conversation-inputs snapshot fell from about 35 s to 0.17 s, and a new covering index over provider turn events keeps history away from large payload pages. `lf history usage --weekly` reads recorded silences; it fell from 53 minutes to under 2 minutes on a debug build. `lf history show --events` prints `events.jsonl`, or only the stored facts once rotation has removed the file. (#1666, #1690)
- The steer hook on `UserPromptSubmit` starts `lf` only when a steer newer than the last one seen exists, matching `PostToolUse`. (#1690)

## Landings and releases stop only for a person

Shipping v0.14.15 exposed three ways a removed merge request could be misread. A release re-armed after a person dequeued it. Loopflow's own CI-repair disarm looked like a person's withdrawal. A GitHub merge-queue eviction stalled a landing with nothing left to repair. Landing and release now share one decision. Late in the cycle, a fourth case appeared: `lf land` dropped a green PR's request and then reported that it had handed off. (#1668, #1673, #1679, #1680, #1684)

- **When a person dequeues a PR or disables auto-merge, the operation stops and names who did it and when.** A landing blocks with a message such as `… removed #248 from the merge queue at …; run lf arm to resume landing`. A release stops with `ManualMergeRemoval` and resumes when you run `lf release run` again. A scheduled release stopped this way does not count as a clean qualifying release.
- **Evictions and Loopflow's own disarms go to CI repair.** A queue eviction shows as a failing `merge-queue (<reason>)` check, linked to the merge-group run within the eviction window. A disarm during a recorded repair, or an unexplained removal with green checks, shows as `merge-request (repair-disarmed|unexplained)`. ci-fix handles both: for a runner fault or a disarm, it confirms that the head's checks pass and runs `lf arm`.
- **Release no longer re-arms while it waits.** A failing observation sends the release to CI repair, so a green head with a red merge result goes to ci-fix instead of re-queuing over and over.
- **`lf land` keeps a queued PR queued and confirms every arm.** Membership in the merge queue counts as armed even before GitHub shows the queue entry. A PR already queued at the same head is left alone. After each arm request, lf checks GitHub up to 3 times, 1 s apart. If GitHub still shows no request, lf asks once more, then fails with gh's reply (for example `already queued to merge`) rather than reporting a handoff. (#1684)
- **Release notes come from origin.** `lf release notes` collects changes up to `origin/<default>` and writes into the checkout you run it from. The packaged-app resource check no longer hangs while Loopflow Desktop is running.

## Keep your work and your terminal

These fixes cover Task branches through syncs, lost terminals, and the shell you return to. (#1665, #1672, #1674, #1676, #1689, #1691)

- **Graph sync no longer resets authored branches.** `lf sync` makes its plan once, while your edits are present, and runs that plan. A branch with commits of its own always merges its target. Reset applies only to branches with no commits of their own, or to persistent branches whose commits have already landed. The `generated_only` class, which reset `checkpoint:`/`wip:` branches, is removed. (#1672, #1676)
- **Stranded edits stop the Task run.** Recovery stashes are named `lf checkout update: caller edits in <path>`. If a Graph fails while one exists, `lf task run` stops instead of retrying and prints the `git stash apply --index <sha>` command. (#1672)
- **Detached Sessions can be reconnected.** A Session whose agent is live but has no viewer appears as `detached` and offers **Reconnect** instead of "Move here". This is a new wire value in Rust, Swift and the DTO fixtures. (#1674)
- **`lf connect <TASK-ID>`** opens that Task's conversation; an exact Session ID still wins. Unknown IDs resolve faster: on Jack's store, lookup fell from 26 s to under 1 s. (#1674)
- **`lf resume` in an unassociated worktree** reopens the unfinished interactive conversation you most recently opened there. The earlier error remains when there is none, or with `--fresh`. (#1665)
- **Attaching no longer clears your screen.** `lf resume`, `lf connect`, takeover, and a launch that attaches after Claude Code has drawn now push your existing output into scrollback, then redraw only the Session's visible screen. Owner history is not replayed. On exit, the `Continue with: lf resume …` hint stays above your prompt. The viewer leaves the alternate screen only if the terminal is still on it. (#1691)
- **A stalled execution owner gets named.** When an owner stops reading input, the viewer now exits with `Execution owner stopped reading requests; outcome is unknown and later input was not delivered` instead of a bare `os error 35`. Queued mouse reports no longer spill into your shell. (#1689)

## Script and read back Loopflow Desktop

Across the cycle, `lf desktop` went from loosely targeted Apple events to a contract an agent can verify. First, `lf desktop` and `lf open` began to target the Desktop app they run inside. Then a socket replaced Apple events. Every Desktop key and click now has a verb, and every verb reports what it reached. (#1660, #1664, #1677)

- **One registry of verbs.** `lf desktop actions` lists every verb with its default key. They include navigation (`back`, `forward`, `goto`, `next`, `previous`, `working-tasks`, `enter`) and terminals (`send`, `send-key`, `capture`, `run`, `show`, `close`, `reopen`). Browser verbs are `browse`, `page-back`, `page-forward`, `reload` and `switch`. `lf desktop ls` reads back windows, panes and destinations; `events` streams changes; `wait` waits for shell commands to finish. (#1677)
- **One target grammar.** `-p/--pane` accepts `session`, `shell:2`, a tile (`3`, `LOO-519`) or `TILE/PANE`. Each part resolves by full ID, then label, then 1-based position, then ID suffix. With no `-p`, the target is the caller's own terminal (`LF_TERMINAL_ID`), then the checkout's Task; focus never decides it. (#1677)
- **Exit statuses mean something.** 0 ok · 1 failure · 2 bad arguments or unknown verb · 3 missing or ambiguous target, nothing changed · 4 Desktop unreachable · 130 interrupted. Receipts carry `target {window, work, pane, tab, rule}` and, for tile edits, `applies_to: window | dashboard`. `lf help` shows these statuses. (#1677)
- **Each Task has three fixed panes: session, shell and browser.** They replace the free split tree. Browser tabs hold Files, the Graph log or a URL, with Back, Forward and Reload. ⌘W closes the addressed tab, and ⇧⌘T reopens the last closed tab with its shell still alive. (#1677)
- **Wave view and tile editing.** A Wave view shows the Wave's conversation with its Tasks as tiles. `hide`, `show`, `move` and `resize` aimed at a tile edit the shared record: a saved Dashboard, or the repository's Task order. `enter TILE` opens the tile as full Work, and Back returns to the board. (#1677)
- **Desktop keeps up.** Sidebar scrolling, hover and Session refreshes no longer re-render every row, tile and terminal, and writes that change nothing are skipped. Creating a pinned Dashboard works again. Zooming a Working Tasks tile keeps Working Tasks in the breadcrumb. A tile offers Start only for an unstarted Task, and the New Dashboard `+` appears only on hover. Dev builds add **Debug › Flash Re-renders** (⌃⌥⌘R). (#1660)

## Choose a direction before building

A feature Task used to begin at `review-design`, refining the one plan it already had, in Markdown. Now the feature Workflow starts at `explore`. The agent lays out about three distinct visions on an HTML page, and Desktop renders that page next to the conversation, so you can pick one and say "build it." (#1687)

- **`explore` replaces `review-design`.** It writes `scratch/explore.html`, including at least one radical-success vision, and iterates as long as you want ("A + B", "three more, keep X fixed"). Only what you decide reaches the Markdown design, with your name and the date. When run headless, it recommends one vision, records the open choice in `scratch/questions.md`, and never records acceptance itself.
- **`concept-review`, `demo` and `research` produce pages too:** `scratch/concept-review.html`, a `scratch/demo.html` runbook, and an answer page. Each page is self-contained, uses no network, leads with the decision and ends with numbered questions.
- **Files renders pages.** The view picker offers Read / Edit / Diff. Read shows Markdown, and shows HTML in a web view that blocks network loads and opens links in your browser. A rewritten page reloads at the same scroll position. A new scratch page takes over the view only if nothing is selected, the selection was itself automatic, or Files has been idle for 2 minutes. Otherwise it opens as a tab.

## Operational notes

- **Store compatibility break.** `lf < 0.14.16 can no longer open the store (raised by: designated_panes, session_turn_events)`. Upgrade every lf that shares an `LF_HOME`, including relays, cron jobs and Machines, together. Relays from older builds keep running the old machine-wide idle sweep until they restart. The release migration also adds pools, on-demands, rentals and `machine_records`, the Sessions-by-owner index, and the turn-events index, and it drops `processes.detached_at`. (#1653, #1677, #1690, #1692)
- **Saved pane layouts reset.** `designated_panes` deletes old saved pane layouts; every Task opens with the session, shell and browser panes. (#1677)
- **Desktop requires macOS 26.** Support for macOS 15 is dropped. The `swift-test` and `loopflow-ui-test` CI jobs run on `macos-26`; `macos-process-test` stays on `macos-15`. (#1687)
- **`lf desktop` scripts need updating.** `text` → `send`, `key` → `send-key`, `read` → `capture`, `restore` → `show`, `shell` → `run`. `lf desktop arrange` is removed. The `LF_DESKTOP=1` fallback is gone: `LF_DESKTOP` is a bundle ID. From an ordinary terminal, `LF_DESKTOP=com.loopflow.mac.dev lf desktop ls` reaches the Dev build. (#1664, #1677)
- **Tasks parked at the feature Workflow's `design` node** need `lf task workflow move <task> explore`. (#1687)
- **Scripts that relied on bare `lf task run` blocking** should add `-b`. (#1675, #1681)
- **First cleanup may be slow.** The first hourly prune or `lf disk clean` on a large store rewrites many rows and vacuums; on Jack's 9.8 GB clone, it took about four minutes. Later passes take milliseconds. Captures younger than 24 h wait for the next pass. After 7 days, task work listings and `lf ps` history no longer show unreferenced inspection Processes. (#1666, #1667)
- **0.14.14 cron jobs upgrade in place.** On first contact, `lf wave cron list` and scheduled runs rewrite Flow-shaped plists, receipts and obligations to the Graph shape, keeping their labels and activation times. These shims are marked for removal (LOO-574). Cron accounting reads only the current repository's records. (#1668)
- **Branches made only of `checkpoint:`/`wip:` commits** now get a merge commit from main instead of a reset. Stashes made before this release have no path in their name, so `lf task run` won't detect them; find them with `git stash list`. (#1672)
- **Relay detach timers live in memory**, so they restart from zero if a relay restarts while nobody is viewing. A relay that never reported viewers is no longer ended by another relay. (#1690)
- **Known gaps:** a person who dequeues in the few seconds between a repair's `lf arm` and the end of its Process can be mistaken for Loopflow's own disarm, and ci-fix may re-arm. In a repo without a merge queue, `lf land` still fails with gh's error when gh refuses `--auto` on an already-mergeable PR. (#1680, #1684)
- **Verify after install:** these behaviors are covered by unit or integration tests, but no end-to-end run has been recorded for them:
  - a real merge-queue eviction going through ci-fix;
  - `lf land` leaving a queued PR untouched;
  - reconnecting a real detached Codex Session;
  - the first scheduled run of a 0.14.14 cron job after the upgrade;
  - the explore page flow in Desktop;
  - relay and Work-feed CPU on a large store.

  (#1668, #1674, #1679, #1684, #1687, #1690)

## Small changes

- `swift-test` fails when a test leaks a continuation and names the test. The Desktop test that hung the merge queue is fixed, and a reused barrier key now fails fast instead of hanging. (#1683)
- The task-installation proof waits up to 120 s for Docker before reporting an infrastructure fault. (#1679)
- `capture-tasks` and `launch-plan` name open PRs that touch the same files and leave those PRs' open decisions unresolved. (#1682)
- The README now opens with what `lf` does for you. A small first exercise, requirements, the files and services it touches, and uninstall steps follow. Reference material moved into `docs/`, including a new *Own and rent Machines* section. Install with `curl -fsSL https://loopflow.studio/install.sh | sh`. (#1671)
- An agent turn that ends `Interrupted` exits 130, and headless `lf run` preserves that code. (#1678)
- TESTING.md rules out CPU-load generators for reproducing races. Use a temporary delay or a bounded repeat loop instead. (#1686)
- Desktop's local agent launcher polls its pipes, so large CLI responses no longer hit its deadline. Packaged third-party sources are reproducible. (#1687)