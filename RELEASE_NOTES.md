# v0.14.15

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.14.15 makes sharing a repository practical: teammates see one plan, Task takeovers keep the branch and PR, and operator passes stay with the work each person owns. Asana and Notion join the planning options, Desktop remembers Dashboards and drafts, and agent cleanup reduces the work of keeping Sessions running. **lf < 0.14.15 can no longer open the store (raised by: rename_flow_to_graph, saved_selections, two_person_gaps)**. **lf < 0.14.15 can no longer open the store (raised by: rename_flow_finished_events)**.

## Work alongside another person

The shared plan now carries the information needed to hand work over without starting again. Ownership, comments and schedules have clearer boundaries, so sharing a repository does not mean operating a teammate's Tasks or running their crons. (#1639, #1651, #1642)

- **One Git plan by default.** Unconfigured repositories use `refs/loopflow/planning/shared/main` on `origin`. On the first save after upgrading, an older unconfigured personal plan moves there with its Task identifiers, comments and ownership history. Choose `shared: <name>` for another shared ref or `personal: true` for a personal plan; the two settings cannot be combined. (#1639)
- **Claims follow Git email.** Claims use the repository's lowercased `user.email`; claiming fails when it is missing. Earlier claims still read as yours when made on one of your Machines. Task lists and Wave status show owners, and a Git-planned Task takeover checks out its recorded branch and finds its existing PR. (#1651)
- **Comments reach the agent.** Comments imported through Git planning steer the Task's Sessions. `lf refresh` fetches and imports the repository's planning ref, then publishes pending changes; a failed exchange returns an error. (#1651)
- **Operators stay on existing work you own.** `repo-operate` and `wave-operate` leave teammates' Tasks alone and list unowned backlog for selection. They no longer change plans, check KRs, rotate chapters, create or delete Tasks, or start unstarted follow-ups. Task capture belongs to `capture-tasks`. (#1642)
- **Crons stay where they were installed.** `lf cron add` installs a job on this Machine. Publishing a cron makes it visible to teammates; fetching it never installs it. Machines are private by default; `lf machine public NAME` publishes the name shown on claims and crons. (#1651)
- **Workflow definitions travel as committed files.** Waves select a Workflow by name from `.lf/workflows/` or builtins. `lf wave workflow set` no longer accepts `--file`, and `lf wave workflow source NAME` no longer takes a Wave. (#1651)

## Keep planning in the team's tracker

Teams can use Asana or Notion alongside the existing Git and Linear options. Coworkers' Task edits, assignments, comments and completion changes flow back into Loopflow's plan and agents. (#1607)

- Set `planning.provider` to `asana` or `notion`, supply `planning.team`, and connect with `lf account connect asana` or `lf account connect notion`. The team value is a Linear Team ID, Asana team gid or URL, or Notion page URL or ID.
- **Asana** keeps Tasks in a team project named after the repository and mirrors Wave Projects as team projects. Because Asana Tasks are open or complete, starting work and PR state appear in comments.
- **Notion** uses linked Tasks and Projects databases beneath the team page, reusing them or creating them and adding missing Loopflow properties. Comments require the integration's Read and Insert comments capabilities.
- **The change sent last wins.** An unsent local save rebases onto an observed provider value and still goes out. A provider edit observed after an attempted write is adopted. This replaces the rule that always adopted Linear's observed value on a conflict.

## Return to the same Desktop workspace

Working Tasks is now a stable view of started work, and side-by-side Tasks come from named Dashboards. Opening a Task no longer changes which Tasks count as working; pane layouts and file drafts survive relaunch. (#1580)

- **Working Tasks** lists started, unfinished Tasks across Machines in repository plan order, with waiting Tasks first. Selecting a Wave narrows the list. An unstarted Task opens its brief and a Start button without starting work.
- **Dashboards** pin queries such as `(wave:product state:open) or state:available` into a paging grid. Drag tiles to reorder them, change slots, or hide and restore a tile. Each tile reuses its Task's Session, tools and drafts.
- **Pane intent persists per Task.** Layout, selected Shell and Files tabs, and drafts follow a Task across Projects. A conflicting save preserves local content and shows an error; **Reload saved intent** reconciles it.
- **Navigation has one destination.** Command-click selects one destination; Dashboards own side-by-side arrangements. The Repo › Wave › Task breadcrumb supports jumping and searching, with the Machine control back alongside it.
- **Desktop commands expose the saved view.** New commands include `view`, `query`, `pin`, `unpin`, `rename`, `arrange`, `slots`, `order` and `layout`. `lf desktop state` replaces `lf desktop list`.
- **Files stop refreshing on their own output.** Git-ignored journal and build writes no longer trigger comparison reads. Remote updates name changed paths so only matching open documents refresh, and checkouts under `/tmp` and `/var` now match filesystem events correctly. (#1641)

## Spend less time tending Sessions

Agent guidance now includes syncing and resolving merge conflicts before presenting work for review. Runtime fixes also address completed steps reported as failures, unviewed agents left running, and first-run failures on Linux. (#1624, #1644, #1638, #1649)

- **Resolve conflicts before review.** The publishing skill runs `lf sync` and resolves conflicts before publishing. Task operators repair conflicting or outdated PRs before reporting them available, and steps a stopped Graph never reached remain their work. (#1624)
- **Detached Sessions stop idle agents.** `session.end_detached_after` defaults to `24h` and accepts durations such as `90m`, `2d`, or `off`. Conversation history remains available through `lf resume`. Attached viewers, running turns, headless Runs and Graph work keep agents alive. (#1638)
- **Finished steps settle under Mac load.** Processes already being torn down count as exited, and process-group checks use one kernel query. If a group really survives termination, the error names its surviving processes. (#1644)
- **Open Work in tmux.** `lf open` detects tmux or accepts `--host tmux`, opens a named window, and focuses it on subsequent opens. A closed viewer window can reopen locally, and window names follow Session names. Diagnostics go to `logs/lf.log`; `-v` or `RUST_LOG` also sends them to stderr. (#1635)
- **Claude opens directly in newly lf-created worktrees.** Interactive launches pre-accept folder trust for that exact worktree path. Main checkouts, hand-made worktrees and worktrees created before this change retain Claude's dialog. (#1648)
- **Fresh Linux startup works across all four agents.** OpenCode 2 logins can be forwarded with `lf machine connect`; Pi no longer depends on inherited `LF_BIN`; Codex starts under Ubuntu's default umask; and SSH commands find agents installed under a user npm prefix. Failures include the last stderr line, and version-skew errors identify the older side. (#1649)
- **Less recurring disk work.** Store schema reads are cached per process until a schema change. `lf monitor work` reads receipts for unfinished rows, while `lf disk clean` and hourly rotation prune receipts whose process is dead and whose store row is absent. (#1650, #1641)

## Operational notes

This upgrade changes stored data, authored Graph names and cron installation. The final release range also includes the finished-Flow history repair and the patch-release retry fix. (#1643, #1651, #1655, #1661)

- **Upgrade every build that opens the same store.** Both compatibility notices in the opening apply: older lf versions cannot open the migrated store. Automatic migrations rename Flow storage to Graph, add saved Desktop state, change shared-work ownership storage, and rewrite old `flow_finished` Task events as `graph_finished` while retaining their summaries and invocation IDs. The history repair is included in this cut; it is not deferred to a future release. (#1580, #1643, #1651, #1655)
- **Rename authored Flows to Graphs.** Move `.lf/flows/` to `.lf/graphs/` and change `flow:` to `graph:` in composition and Workflow YAML. Update scripts from `lf flow` to `lf graph`, `lf desktop flow-log` to `lf desktop graph-log`, and `lf desktop list` to `lf desktop state`. There are no aliases for the old names. Wire consumers must adopt the Graph field names and the remote Files frame's `paths` array in place of `changed`. (#1643, #1580, #1641)
- **Set the same Git `user.email` on each Machine before claiming.** Older builds keep publishing to personal planning refs until upgraded, so mixed versions can diverge. After all Machines upgrade, the old inactive ref can be removed with `git push origin :<ref>`. Anyone able to push to `origin` can write the shared plan; ref separation is not access control. (#1651, #1639)
- **Reinstall repository cron jobs on their intended host.** `crons:` is removed from `.lf/config.yaml`; `lf cron sync` no longer installs jobs from it. Rerun `scripts/bootstrap-cron-host.sh`, or add each job with `lf cron add` and then run `lf cron sync`. `lf wave cron preflight` is removed. (#1651)
- **Detached cleanup is on by default.** Older owners that never recorded viewer state become eligible after 48 hours from startup rather than the configured duration. Set `session.end_detached_after: off` to disable cleanup. (#1638)
- **Patch retries can include later fixes.** When a release PR has merged but no candidate was built, resuming `lf release run` builds and tags the current remote default branch if it descends from the merge commit. Progress names both commits. Review the intervening changes before resuming; minor releases still publish the exact patch snapshot. (#1661)
- **Provider setup and limits.** `linear.team` remains a fallback when `planning.team` is unset. Asana and Notion validation used mocked APIs; no real-workspace run is recorded in the supplied evidence. OpenCode forwarding requires version 2 on both Machines. (#1607, #1649)
- **Remote ignored files do not auto-refresh.** An open git-ignored file on a remote checkout no longer picks up outside edits automatically; local open files still do. (#1641)

## Small changes

- Swift PR test runs have a five-minute budget; individual Swift tests have a 300-second limit. Rust PR tests use a two-minute nextest limit with named allowances for existing slow tests. Failure summaries identify unfinished and slow tests. Merge queue and main retain their separate limits. (#1646, #1647)
- Changes confined to docs, the website, scratch, Wave memory or Python skip macOS jobs. Superseded merge-queue runs are cancelled. (#1646)
- Built worlds record real Workflows and authored eight-step `pursue` runs. Planning exchange happens once at completion, reducing density-world build time. The costly Swift world suites and the all-recipes Rust integration test remain removed pending faster builds. (#1640, #1645, #1646, #1647)
- Terminal scrolling matches Ghostty. Explicit `-c` tries the tmux paste buffer and reports attempted sources if no text is available; redirected headless runs write only their answer. (#1580, #1649)
- The website and guides lead with shared work, include a “Work with another person” walkthrough, and recognize Pi. `/cli` redirects to the homepage. (#1637)
- Ruff is updated from 0.17.0 to 0.17.1. (#1657)