# v0.15.0

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.15.0 collects the 0.14.11–0.14.20 cycle into one release: a Task can now be handed off and left to finish. One Graph carries it from first step to merged PR, `lf ship` or Auto-Ship lands it, and it can run on a Machine you own or one you rent. Two people can share a repository without crossing claims or branches. The store has been cut down to facts, so installs, live views and idle processes stop slowing down as history grows. Each Wave's setup now lives in one committed file, with `lf set` for your own overrides. This release changes the store format: **lf < 0.15.0 can no longer open the store (raised by: wave_setup)**. Upgrade every `lf` that shares `~/.lf` together, including the macOS app.

## Hand a Task off and let it finish

The cycle started with three overlapping models of a Task's progress: Flows, Workflow stages and routes, and per-Task automation holds for CI. It ends with one. Flow became Graph everywhere (#1643). Workflows then folded into the Graph file itself, which now says where a person takes part (#1733). Bare `lf task run` returns once the Graph is running (#1675), and `lf ship` runs the rest of it through CI repair and merge (#1652).

- **One Graph per Task.** A Task captures one Graph and its position in it. `interactive:` steps run in place under `lf graph`. Under `lf task run`, the run stops before them and the Task waits in its conversation. `-i`/`-b` choose for every step, and `--from <step>` starts anywhere. A Task with no Graph takes its Wave's default, then repository `graph:`, then builtin `code`. (#1733)
- **Runs leave your shell.** `lf task run` returns once the Graph registers. `lf -b task run` still waits. `lf task interrupt` now stops a running Graph. It used to record a request that nothing acted on. (#1675, #1678)
- **Steps default to headless.** A Graph started from a terminal no longer opens every step as a conversation. Headless Claude steps stay alive until any commands they backgrounded have finished. (#1628, #1688)
- **Ship in one move.** Click **Auto-Ship** or run `lf ship`, and Loopflow runs the remaining steps, repairs CI, merges the PR and completes the Task. A route into `end` also approves Auto-Ship. A two-tier approval that briefly stopped this (#1736) was reverted (#1741). `lf ship --stop` withdraws approval. (#1652, #1741)
- **Agents handle conflicts themselves.** A PR that conflicts with or falls behind `main` is synced and resolved by the agent; it no longer goes to a person as ready for review. Graph `sync` keeps branch commits and checkpoints uncommitted edits before merging, so conflicts reach the resolver rather than a stash. (#1624, #1672, #1748)
- **Choose a direction before building.** The feature Graph starts at `explore`. It lays out several visions as pages you can read in Desktop's Files view, so you pick one before any code is written. (#1687)
- **Finished work goes quiet.** Completed and abandoned Tasks with old failed runs no longer ask for attention. Completed Tasks stay complete through Linear rate limits and unrelated edits. (#1621, #1742)

## Set up a Wave once

Setting up an area of work used to take a separate command for each setting, split between `GOAL.md` frontmatter and the store, and nothing showed which value a launch used. (#1751)

- **Committed Wave config.** `.lf/wave/<name>/config.yaml` has the same shape as `.lf/config.yaml` and holds only what the Wave changes: `graph`, `agent`, `skill_agents`, `context_budgets`, plus the Wave's `id`, `linear.initiative` and `chat`. Launches in a Wave layer it over the repository's config and `~/.lf/config.yaml`. Repository-only keys are an error there. (#1751)
- **`lf set <key> <value>` and `lf unset <key>`** store your own values on this Machine and never edit committed config. They apply to this repository by default, to a Wave with `lf --wave <wave> set`, or everywhere with `--global`. Keys are `graph`, `agent`, `skill_agents.<skill>`, `machine`, `machine.tasks`, `machine.session` and `account.<provider>`. (#1751)
- **`lf wave status`** shows each setting and where it came from. (#1751)
- **Removed:** `lf wave graph set` (use `graph:` in the Wave's `config.yaml`, or `lf --wave <wave> set graph`), `lf ops machine use` (use `lf set machine`) and `lf account route set` (use `lf set account.<provider>`). (#1751)

## Land PRs without babysitting the merge queue

Landing and release each guessed why a PR's auto-merge had disappeared, and they guessed in opposite directions. Now only a person removing the merge request stops them (#1680). Every other kind of removal gets repaired.

- Removals Loopflow makes itself during CI repair, and drops by GitHub's merge queue, are repaired instead of read as withdrawal. `lf land` no longer drops the request on PRs that are already green. (#1673, #1679, #1684)
- CI repair and sync run alongside an open Session in the Task checkout, and sync no longer holds its lock across network fetches or conflict resolution. (#1714)
- `lf land` merges `main` only when GitHub or the queue requires it. Other repairs push on top of the base CI already tested. (#1749)
- Rust-only PRs skip hosted macOS runners. macOS jobs run once per PR, and only when the PR's paths can affect them. `main` is tested after merge, and a failure is bisected to open an unarmed revert PR. (#1646, #1749)

## Run work on a Machine you own or one you rent

Tasks used to run where they started. Now each has a single owner: a person on a Machine, or nobody. That owner can be claimed, handed to another Machine or released (#1610). A Task can also go to free capacity and come back with its history.

- **Ownership is explicit.** Claims follow the repository's Git `user.email` rather than display names. Moving a Task to another of your Machines carries its conversation, which is no longer completed on the way. (#1610, #1651, #1720)
- **Rent capacity.** `lf task run --pool lab --rent code` sends a Task to a free owned Machine or rents one, and gives it back with its history afterwards (#1653). By the end of the cycle the providers are Daytona, Modal, RunPod and AWS, with GPU and spot options where the provider has them. A Wave can send its Tasks to rented machines by default. (#1704, #1707)
- **Fly is gone.** Fly arrived in #1707 and was removed in #1743. AWS rentals now use a pinned region, a working Claude login, a Git identity for claims, and Linear planning. (#1743)
- **Fresh Linux machines work.** Claude, Codex, OpenCode and Pi run on a newly rented Linux box. The fixes cover OpenCode login forwarding, Pi startup, Codex under Ubuntu's default umask, and npm-installed agents over SSH. (#1649)
- **Conversations move with their history.** `lf resume` moves a conversation to another login or Machine with its native history intact. Open Codex conversations are pinned to their login so a move finds them, and the terminal a conversation left closes and says where it went. (#1720, #1740, #1746)
- **Launch on the account with the most room.** Shared-mode Claude and Codex launches pick the account with the most remaining usage. They no longer stay on the signed-in account until it reaches 95%. (#1717)

## Share a repository with someone else

Before this cycle, two people in one repository saw separate plans, and a takeover opened a new branch and PR. Now they share one plan and one branch per Task.

- Git planning defaults to the shared ref `refs/loopflow/planning/shared/main` on `origin`. (#1639)
- A takeover keeps the Task's branch and PR. Comments and crons stay with the person who owns them. (#1651)
- `repo-operate` and `wave-operate` work only on your own existing Tasks. They no longer edit plans, file Tasks or reach into other people's work. (#1642)
- Asana and Notion can be a repository's tracker as well as Linear, set in `.lf/config.yaml`. (#1607)
- Briefs for new Tasks name any open PRs already rewriting the same files. (#1682)
- The website, README and docs now lead with building software together and with what `lf` does for you. Reference detail has moved into the docs. (#1637, #1671)

## Desktop shows the work you're doing

Desktop used to treat opening a Task and arranging panes as the same act. Its views also re-rendered and re-read far more than they displayed.

- **Dashboards and a working sidebar.** Saved Dashboards put Tasks side by side, and pane layouts and drafts survive relaunch. At every scope the sidebar lists only started Tasks of the selected Wave or Dashboard, and ⌘1–9 follow those rows. (#1580, #1660, #1669, #1703)
- **Wave standup board.** A Wave opens as one board beside its conversation, showing what's waiting, in progress, shipping and landed. (#1705)
- **Less churn.** Scrolling and hovering no longer re-render every row and tile. The Files view no longer re-reads itself from the journal trace it causes. Each Desktop part runs its own `--watch` read over open Tasks. (#1641, #1660, #1719)
- **Scriptable from `lf desktop`.** Agents can arrange panes and read back the result, and exit status distinguishes a missing pane from an unreachable Desktop. `lf desktop` and `lf open` address the Desktop they run inside, Loopflow Dev included. (#1664, #1677)
- **Checked against real data.** Every public Desktop read decodes output from a source-built `lf-world`, so a Rust wire change can't silently break a screen. (#1619, #1617, #1640)

## A smaller, cheaper store

On Jack's store (about 10 GB, 943k processes, 2.7M session events), `lf install` took minutes, idle relays burned CPU scanning Sessions nobody watched, and a single live reader cost about 48 s of CPU per minute. The store now keeps only the facts Loopflow reads back.

- **Facts only.** The duplicate provider stream copy is gone. The store keeps one usage row per provider turn, and `events.jsonl` is no longer written. Conversations are read from provider transcripts, and `lf history` shows what a provider will remove next. (#1666, #1698, #1700, #1706)
- **Fewer Process rows.** Hooks and Desktop polls no longer create Processes, and finished rows nothing references are deleted. Store retention is in `lf monitor prune`. (#1667, #1718, #1744)
- **Faster opens and idle checks.** Each store's schema is read once per process instead of on each of the ~17 opens per launch. Sessions are indexed by execution owner. Idle relays and the Desktop feed stop rescanning. (#1650, #1690, #1692)
- **`--watch` on ordinary reads.** `lf task list`, `lf task status`, `lf session list`, `lf wave show`, `lf ps` and more stream `{answers, unavailable, body}` lines and exit when their reader goes away. This replaces `lf monitor work`. (#1719)
- **Lock contention has a name.** `lf.log` and `lf doctor` name the process and binary holding the write lock. The hourly prune waits for the lock instead of failing, and usage updates no longer scan `session_events` under it. (#1713, #1724, #1726)

## Commands you can see and run

`lf`'s root help listed about thirty unordered peer groups, and Graphs, Desktop and hooks drove it through hidden `__` commands.

- Root help leads with Task, Wave, Session, Graph and Skill. Delivery, machine and account plumbing moved under `lf ops`. (#1708)
- Hidden commands became ordinary, documented ones, so `lf help` shows what Graphs and Desktop run. (#1697)
- Undesigned surface was removed, including the public `lf transport` group, the `--create-pr` alias and cron-only global flags. `lf skill customize` was added. (#1615)
- `lf wave workflow`, `lf task workflow`, `lf graph route`, `lf monitor work`, `lf monitor active` and `lf disk` are gone. See Operational notes for their replacements. (#1719, #1733, #1744)

## Sessions that behave in your terminal

- `lf` keeps your screen, scrollback and the "Continue with: lf resume …" hint. Warnings and diagnostics-path banners go to logs, not the terminal. (#1635, #1691)
- Inside tmux, `lf open` opens Work in tmux. A Session whose terminal closed reads as reconnectable, and `lf connect <Task-ID>` connects right away; it used to search provider history for about 26 s. (#1635, #1674)
- `lf resume` continues the last conversation in a worktree no Task, Wave or repository owns. (#1665)
- An unviewed Session stops its agent after a day (`session.end_detached_after`). (#1638)
- Claude opens in lf-created worktrees without a folder-trust prompt. Steers and Task context keep arriving after `/clear`. Codex Sessions receive Loopflow's context hooks again. (#1648, #1731, #1734)
- Sessions started under an older `lf` stay stoppable after an upgrade removes a hook they still call. (#1629, #1630)
- A viewer whose execution owner stalls names the owner; it no longer dies with `EAGAIN`. Steps whose agent exited under heavy load now finish. (#1644, #1689)
- Selected documents (`--docs`, config and skill `docs:`) reach the agent as full bodies, matching preview and capture. (#1752)

## Cleanup that keeps your work

Cleanup policy used to live in three places. Under some of those rules it could delete the only copy of committed work.

- `scripts/disk-cleanup.sh install` runs an hourly cleanup skill from the main checkout. `~/.lf` rotates by age, after seven days, instead of chasing a 20 GB budget it could never meet. Idle ignored build output is reclaimed. (#1696, #1744)
- Live caches like SwiftPM's are left intact, the uv cache lock no longer hangs cleanup, and staging directories from killed draft builds are removed. (#1628, #1633, #1727)
- `lf wt prune` removes a checkout only when a merged or closed PR matches its exact local head. (#1753)
- `lf sync` checkpoints ordinary edits and leaves `.lf/tmp/` as it found it. That fixes automatic sync stopping partway on Apple Git 2.50.1. (#1754)

## Operational notes

- **Store floor.** Releases in this cycle raised the store's minimum reader at 0.14.12, 0.14.15, 0.14.16, 0.14.17, 0.14.19 and 0.14.20. v0.15.0 raises it again: upgrade every CLI, Desktop helper and cron install that shares the store at the same time. A Wave graph chosen with `lf wave graph set` lived in `waves.graph`, which this release drops; add `graph:` to the Wave's `config.yaml` before upgrading to keep it. (#1751)
- **Find stale processes.** Running Sessions and Graph runners keep the binary they started with. `lf monitor ps` shows which ones are behind, and idle Sessions can move to the new install. (#1729)
- **Update scripts.** `lf wave workflow` → `lf wave graph`. `lf task workflow` and `lf graph route` → `lf task run`, `--from` and `lf task status`. `lf monitor work` → `--watch` on each read. `lf monitor active` → `lf session list --watch` with `lf ps --watch`. `lf disk` → `scripts/disk-cleanup.sh` and `lf monitor prune`. Plumbing commands now live under `lf ops`. (#1708, #1719, #1733, #1744)
- **Repository `workflow:`** is still accepted as an alias for `graph:`. LOO-637 tracks its removal. (#1733)
- **Old records still read.** Task events recorded as `flow_finished` and Graph records that use `human` both still load after the renames. Cron jobs installed by 0.14.14 with `--flow` keep running. (#1655, #1668, #1722)
- **Release and cron host.** Rerun `scripts/bootstrap-cron-host.sh`. It retires leftover launchd agents, accepts a Wave with no Machine default, and keeps crontab commands under cron's 999-byte limit. (#1711, #1732, #1738)
- **Release run.** `lf release run` builds fixes merged after an unbuilt release PR, and repair cuts no longer fail promotion over an existing `BREAKING.md`. A release stops when someone takes its PR out of the queue. Hotfixes promote from a plain release build and are labeled as hotfixes; the `hotfix` skill describes how. (#1661, #1662, #1668, #1712, #1730)
- **Install cleanup.** Run `scripts/disk-cleanup.sh install` and check the cron entry. Checkouts lost to the old pruning rules are not recovered, and checkouts with no matching PR now need explicit disposal. (#1744, #1753)
- **macOS regressions surface after merge.** A PR the classifier skipped can break macOS on `main`. The revert PR the bisect opens is unarmed, so someone has to merge it. (#1749)

## Small changes

- Rust tests no longer write test logins into your real Codex or Claude home. (#1693)
- Each provider has one decoder and one writer for what it records. (#1702)
- Agent ancestry that was never recorded is repaired, and `lf self doctor` passes `lineage` again. (#1747)
- cmux workspaces are named `<Task title> · …` and keep names you set by hand. (#1716)
- Gate and implement run only the tests a change reaches, one build at a time. PRs fail when their tests run too long and name the slow ones. Swift tests fail on a leaked continuation. (#1647, #1683, #1745)
- `lf-world` Swift tests were removed until world builds are fast (LOO-564). The Swift 6.2.3 release-optimizer crash is avoided. (#1645, #1699)
- `lf wt prune --branches` keeps its protections and summary in one place. `lf sync` borrows its plan instead of cloning it. (#1626, #1676)
- `TESTING.md` says to reproduce races by delaying or repeating tests, never by loading the CPU. (#1686)
- The Desktop workspace drops its outer border and squares its top corners. (#1728)
- ruff 0.17.1. (#1657)