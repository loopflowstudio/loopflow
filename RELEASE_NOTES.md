# v0.14.15

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.14.15 is the release where two people can share one repository. Git planning now defaults to one shared plan. Claims follow your Git email, and a takeover keeps the Task's branch and PR. Comments steer agents under every planning transport, crons stay on the Machine that installed them, and operator passes leave teammates' Tasks alone. Asana and Notion join Linear as planning providers. The autonomous step chain is now called a Graph everywhere: `lf flow` becomes `lf graph`. Desktop gains saved Dashboards and pane layouts that survive relaunch. **lf < 0.14.15 can no longer open the store (raised by: rename_flow_to_graph, saved_selections, two_person_gaps).**

## Work alongside another person

Before this cycle, two people in one repository each saw a private plan, claimed each other's Tasks when neither had set a name, and forked a new branch and PR on every takeover. Every Machine that synced the config ran the repository's crons. The docs, website and operator skills now lead with shared work, and the behavior underneath them changed to match. (#1639, #1651, #1642, #1637)

- **One shared Git plan by default.** With no `planning` config, every clone reads and writes `refs/loopflow/planning/shared/main` on `origin`. `shared: <name>` still picks another shared ref. A personal plan now needs `personal: true`, and setting both is a config error. On its first save after upgrading, a plan that an older build published to an unconfigured personal ref moves to the shared ref with its identifiers, comments and ownership history. `lf planning key` only applies to personal plans. (#1639)
- **Claims match by Git email.** A claim names its person by the repository's lowercased `user.email`, and claiming fails without one. A claim written before this release still reads as yours if one of your Machines made it. `lf task list`, `lf wave show` and `lf wave status` show each Task's owner. Ownership log lines read like `Oct 21 14:02  claim  Lee on lee-mbp`. (#1651)
- **Machines are private unless you publish them.** `lf machine public NAME` sets the name teammates see on your claims and published crons. `lf machine public off` makes the Machine private again. (#1651)
- **Takeover keeps the branch.** Under Git planning, the first checkout records the Task's branch in the plan. The next claimant checks out that branch and finds its PR, as under Linear. (#1651)
- **Comments steer under Git planning.** A teammate's comment imported through the Git plan becomes a steer for the Task's Sessions. `lf refresh` fetches and imports the planning ref, then publishes pending changes. It works per repository and exits non-zero with the recorded error when the exchange fails. (#1651)
- **Crons are personal.** `crons:` is gone from `.lf/config.yaml`. `lf cron add` installs a cron on this Machine only. `lf cron publish` and `lf cron unpublish` share or withdraw it under `refs/loopflow/crons/<machine-id>`, and fetching a published cron never installs it. `lf cron list` shows your crons first, then teammates'. `lf wave cron preflight` is removed. (#1651)
- **Workflow definitions are committed files.** Wave Workflows come only from `.lf/workflows/` and builtins, and a Wave stores only the name it selected. `lf wave workflow set` no longer takes `--file`, and `lf wave workflow source NAME` no longer takes a Wave. (#1651)
- **Operators stay on your existing Tasks.** `repo-operate` and `wave-operate` compare each Task's owner with `lf config user`. Your started Tasks get a disposition. Teammates' Tasks are listed as "theirs · left alone" and are never run, recovered, completed or commented on. Unowned backlog is listed for you to select. Operators no longer check KRs, edit plans, rotate chapters, create or delete Tasks, or start due follow-ups. If you ask for new Tasks in a conversation, `capture-tasks` handles it. Cleanup only touches branches, PRs and worktrees of Tasks you own. Your own Task on another Machine is run there with `lf --machine <machine> task-operate`. `lf config user` now reports a forwarded `LF_USER_NAME`. (#1642)
- **A new "Work with another person" guide** in `docs/lf.md` covers naming yourself, reading the plan, claiming, commenting, requesting review and syncing, plus what travels between Machines and what stays local. The homepage, README and docs index lead with the team story, list Claude Code, Codex, OpenCode and Pi, and no longer assume Linear. The `/cli` page now redirects to the homepage. (#1637)

## Plan in Asana or Notion

Teams that already track their work in Asana or Notion can sync the repository's plan there instead of Linear. Coworkers create, edit, comment on, assign and close Tasks in their own tracker. Loopflow keeps its Tasks, Wave Projects and running agents in step with them. (#1607)

- `planning.provider` accepts `asana` and `notion` alongside `linear` and `git`. The new `planning.team` names the team for every provider: a Linear Team ID, an Asana team gid or URL, or a Notion page URL or ID. Connect with `lf account connect asana` or `lf account connect notion`.
- **Asana:** Tasks live in a team project named after the repository, created if missing. Wave Projects mirror as team projects. Asana tasks are only open or complete, so starting work and PR state appear as a comment on the task.
- **Notion:** the team page holds linked **Tasks** and **Projects** databases. Loopflow reuses existing ones or creates them, and adds `Loopflow ID`, `Status` and `Project` properties where they are missing. Comments need the integration's Read and Insert comments capabilities.
- **The last change sent wins.** An unsent local save rebases onto the provider's observed value and still goes out. A provider edit observed after an attempted write is adopted. Before, any observed conflict adopted Linear's value.
- [Plan and share Tasks](docs/planning.md) explains what flows each way for Git, Linear, Asana and Notion.

## Flow is now Graph

The word "flow" named the autonomous step chain, part of Workflow, and part of the product name. The chain is now a Graph in the CLI, Desktop, authored YAML, docs and website. Nothing keeps the old name working. (#1643)

- `lf flow`, `lf flow list`, `lf flow show`, `lf flow customize` and `lf desktop flow-log` become `lf graph`, `lf graph list`, `lf graph show`, `lf graph customize` and `lf desktop graph-log`.
- Repository definitions move from `.lf/flows/` to `.lf/graphs/`. Composition and Workflow edges name their Graph with `graph:` instead of `flow:`.
- Wire DTOs rename `flow_membership`, `flow_lf_process_id` and `flow_processes` to `graph_membership`, `graph_lf_process_id` and `graph_processes`. Desktop labels say Graph.
- Stored Graph processes, mid-edge Task Workflows and Wave workflow YAML in `loopflow.db` are migrated automatically when lf opens the store.

## Desktop: Dashboards and layouts that survive relaunch

Desktop used to treat selecting Work and arranging panes as one action. The working list depended on which Tasks you had opened, and layouts and file drafts disappeared when you quit the app. Now the sidebar shows a stable query, side-by-side Tasks come from named Dashboards, and pane intent lives in lfdb. (#1580)

- **Working Tasks** lists every started, unfinished Task in the repository's plan order, with waiting Tasks first. Opening a Task no longer adds it to the list or moves it. Clicking a Wave narrows the list to that Wave, and **‹ Repo** returns to Working Tasks. An unstarted Task opens its brief next to a **Start** button.
- **Dashboards** are named, pinned queries such as `(wave:product state:open) or state:available`. Each opens as a 7-slot paging grid of tiles that reuse each Task's Session, tools and drafts. You can reorder tiles by dragging, add or remove slots, and hide a tile and restore it later. Command-click now selects a single destination.
- **The breadcrumb is a jump bar.** Click a name in **Repo › Wave › Task** to go there, or click **›** to search what it contains. The Machine control is back in the breadcrumb.
- **Saved pane intent.** Pane layout, selected Shell and Files tabs, and file drafts persist per Task and follow it across Projects. A conflicting write keeps your local content and shows an error, and **Reload saved intent** reconciles the two.
- **New `lf desktop` commands:** `view`, `query`, `pin`, `unpin`, `rename`, `arrange`, `slots`, `order` and `layout`. `lf desktop list` is replaced by `lf desktop state`, which also reports `open_work` and the selected `view`.
- Terminal scrolling now matches Ghostty.
- **The Files view stops re-reading itself.** Writes to git-ignored paths such as `.lf/journal/`, `target/` and `.build/` no longer trigger a refresh. In one case, a feedback loop with `lf task diff` ran about 10 times a second for seven hours. The remote `TaskFilesFrame` now carries `paths: [String]` instead of `changed: Bool`, so only matching open documents are invalidated. Checkouts under `/tmp` and `/var` now match FSEvents correctly. (#1641)

## Agents that clean up after themselves

Several changes keep finished or abandoned agent work from consuming resources or your attention. (#1638, #1624, #1644, #1641)

- **Sessions nobody is viewing stop their agent.** After `session.end_detached_after` (default `24h`; accepts `90m`, `2d` or `off`), a detached Session's agent exits. The conversation stays in history and `lf resume` reopens it. An attached viewer, a running turn, a headless Run or Graph membership keeps it alive. (#1638)
- **Agents resolve merge conflicts themselves.** `pr-publish` runs `lf sync` and resolves conflicts before publishing. `task-operate` no longer reports a conflicting or out-of-date PR as available. After a Graph stops, the steps it never reached remain the agent's work. (#1624)
- **Graph steps finish under heavy load.** Processes the kernel is still tearing down now count as exited. The process group is read with one kernel query, and the wait after SIGKILL is now 10 s. A group that really survives produces an error that lists each survivor, such as `pid 1234 (sleep, S)`. (#1644)
- **Process receipts stop piling up.** A receipt whose OS process is dead and has no matching row is pruned by `lf disk clean` and by the hourly rotation. `lf monitor work` reads only unfinished rows' receipts. `lf disk` shows a `Process receipts  N` line. (#1641)

## Opening Work, and fresh machines

- **tmux is an `lf open` host.** Inside tmux, or with `--host tmux`, Work opens in a new window named for it. Opening it again focuses that window. A conversation whose window closed reopens here instead of being reported as open "elsewhere". Window names follow the Session name. (#1635)
- **Diagnostics go to `logs/lf.log`.** Launches no longer print diagnostics-path banners, and warnings stay off the terminal. Use `-v` or `RUST_LOG` to see them on stderr. (#1635)
- **No folder-trust prompt in lf-created worktrees.** lf marks the worktrees it creates. When it launches interactive Claude in one, it pre-accepts Claude's folder trust for that exact path only. The main checkout and hand-made worktrees still show Claude's own dialog. Worktrees created before this release prompt once. (#1648)
- **Fresh Linux machines work with all four agents.** `lf machine connect <machine> opencode` forwards an OpenCode 2 login. Pi starts without an inherited `LF_BIN`. Codex starts under Ubuntu's `002` umask. Agents installed under a user npm prefix are found over SSH. Agent failures now include the last stderr line, and version-skew errors say which side is older. With `-c`, lf also tries the tmux paste buffer, and when it finds no clipboard text it names every source it tried. (#1649)
- **Faster launch.** Each process reads a store's SQLite schema once and reuses it until a DDL change. A bare launch reads about a third fewer rows (24,638 → 15,738). (#1650)

## Operational notes

- **Store compatibility:** lf < 0.14.15 can no longer open the store (raised by: rename_flow_to_graph, saved_selections, two_person_gaps). Upgrade every Machine that shares a Loopflow home before opening it with this build. Older builds stop working against the migrated store. This release's migration renames the Flow tables to Graph, adds the Dashboard and pane-layout tables, drops `wave_workflows`, adds owner email and Machine label columns to `task_ownership`, and adds `machines.public_name`.
- **Rename authored Flow files.** This build does not read `.lf/flows/` or `flow:` keys. Move definitions to `.lf/graphs/` and change `flow:` to `graph:` in Graph and Workflow YAML. Scripts that call `lf flow` or `lf desktop list` need `lf graph` and `lf desktop state`.
- **Set your Git email before claiming,** and use the same `user.email` on every Machine. Otherwise your claims from another Machine will not read as yours.
- **Mixed versions diverge for a while.** Machines on older builds keep publishing to their personal Git planning ref until they upgrade. Once every Machine has upgraded, remove the old ref with `git push origin :<ref>`. Anyone who can push to `origin` can write the shared plan, because ref separation is not access control.
- **Cron hosts need reinstalling.** `lf cron sync` no longer installs jobs from config. Rerun `scripts/bootstrap-cron-host.sh`, or `lf cron add` each job and then run `lf cron sync`.
- **Detached Sessions now end by default.** After upgrading, detached owners left by older builds end once they are 48 hours old, whatever the configured limit. Set `session.end_detached_after: off` to keep them running.
- **Config fallback:** `linear.team` folds into `planning.team` when `planning.team` is unset, so the released shape stays readable. LOO-560 removes the fallback.
- **Not exercised against real services.** Asana and Notion were tested against mocked APIs. Real-Claude trust verification on macOS and Linux remains for review. Most PRs' suggested walkthroughs were not performed.
- `~/.lf/runtime/exec-processes-archive-20261021` can be deleted by hand.

## Small changes

- Swift CI ends a run within 300 s of a stuck test and names it. Pull requests get a 5-minute Swift test budget and a 2-minute per-test Rust budget (nextest `pr` profile). Failures list unfinished tests and the slowest finished tests. Changes limited to docs, the website, scratch, Wave memory or Python skip the macOS jobs. Superseded merge-queue runs are cancelled. (#1646, #1647)
- The `lf-world` Swift tests and the `every_recipe_builds_a_world_lf_reads` Rust test are removed until world builds are fast (LOO-564). Built worlds now record real Workflows and full eight-step `pursue` runs. Deferring the planning exchange cut the density world build from about 219 s to about 50 s. (#1645, #1647, #1640, #1646)
- The gate skill now says a timed-out required check gets investigated, not overridden by a later green run. (#1647)
- `lf clone create` points `TMPDIR` at the clone's own tmp before migrating. (#1643)
- `lf install` shows its promotion preview only for preview-only runs. `lf -b … > file` at a terminal writes only the answer. (#1649)
- The `lf init` skill looks for Wave goals under `.lf/wave/` and accepts Pi as an installed agent. (#1637)