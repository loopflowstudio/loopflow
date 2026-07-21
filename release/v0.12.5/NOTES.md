# v0.12.5

v0.12.5 makes durable work easier to trust while it is running: blocking questions, task completion, Wave startup, provider activity, and release state now have explicit owners and observable evidence. A durable Ask is the only human-input primitive that can stop a Task, and `lf start` now returns only after a Wave is live or its attempted state has been rolled back. The terminal and Mac app expose the same live motion, durable history, and Work hierarchy instead of reconstructing competing control models.

## Work blocks and completes on durable facts

The runtime now separates control state from provider conversation. Work owns Epochs, Runs own leases and process containment, and AgentInvocations preserve provider evidence; this removes the former Session, Feedback/Continue, radio, receipt, and server-owned memory layers rather than keeping parallel ways to advance work (#1106).

- `lf ask`, `lf ask wait`, `lf work asks`, and `lf work answer` create immutable, Turn-scoped Ask/Answer exchanges. The first authorized answer wins, recovery survives process loss, and Task exchanges mirror to Linear through a retryable comment outbox.
- Interactive and demo phases are advisory. A Task makes one launch attempt and advances regardless of launcher failure, UI lifetime, or handback; Codex and Claude launch read-only, while OpenCode fails closed until it can enforce that boundary. Only a pending durable Ask marks work as waiting on a person (#1126).
- Automatic Task recovery is bounded to three no-progress attempts. Durable progress or explicit User input replenishes the budget, failure events settle atomically with their Run and Invocation, and Tasks pinned to the retired default `task` flow repair to `slice` before execution (#1151).
- Controller-proven Task and Project completion now commits lifecycle state, the terminal Work Epoch, completion evidence, Run state, and open Invocation state as one idempotent receipt. Repeated reconciliation does not manufacture synthetic Runs or duplicate completion events (#1130, #1157).
- Task gate proposals remain attached to the exact `finally` Epoch that created them, while terminal Work stays authoritative over a stale active snapshot (#1133).
- Durable provider turns are confined to their assigned worktree. Publication consumes validated gate copy before committing, strips gate-owned review artifacts, and coordinates registry-proven OpenCode orphan cleanup without touching unclaimed provider processes (#1163).

## Waves have one Home and one conversation

Starting and steering a resident Wave now follows one Home-owned lifecycle. The same operation drives the CLI and Mac app, and remote execution crosses an explicit `lf ssh` boundary instead of allowing inherited environment to decide where authority lives (#1119, #1161).

- `lf start <wave>` waits for the Home daemon and listener, drains durable startup observations, and returns a `live` or `failed` result. A failed attempt removes only registry state it introduced; successful sibling Waves remain running.
- `lf` and `lfd` are promoted and rolled back as one validated control-plane installation, so a new CLI cannot advance the shared store while an old daemon keeps serving it (#1145).
- `lf pause` keeps a listener live while refusing new message, heartbeat, and cron turns; `lf resume` releases queued and future turns. Both commands are idempotent, support JSON and `lf ssh`, and expose turn intent separately from listener liveness (#1138).
- Ordinary commands are machine-local. `lf ssh <home-id> ...` proves the target Home, invokes its `lf`, and can select from target-local or origin-forwarded subscription accounts for the foreground command. Detached residents scrub forwarded provider, GitHub, PM, SSH, and secret authority.
- A Wave can bind one Discord channel as its active conversation. Backing changes create immutable epochs, Discord remains transcript authority, and CLI or Mac compose posts idempotently through the bot before the canonical provider echo re-enters the resident with message, Steer, or interrupt intent (#1125, #1136, #1142, #1150).

## Live motion and durable history are both visible

The new operational surfaces keep process evidence, Work history, and planning distinct but navigable. Operators can see what is producing now, trace what happened before, and open the exact Run or PR without treating elapsed time or an absent measurement as failure (#1122, #1144).

- `lf ps` emits one stable live call-tree snapshot; `lf top` refreshes the same evidence every two seconds on a TTY. Both show cumulative completed output, five- and thirty-minute rates, age, idle time, health, provider ancestry, and exact Home ownership.
- `lf prune --dry-run` lists dead Exec receipts and registry-proven orphan OpenCode groups. Plain `lf prune` removes only those targets and never kills an unclaimed provider PID.
- `lf activity` provides a bounded, newest-first timeline of Work creation, Runs, Task PRs, and Steers, with Wave, Project, and Task filters applied before the result cap.
- The Mac app's Podium uses a repository-scoped Waves sidebar and a Wave → Project → Task → Exec output hierarchy. Selection focuses the Work and Activity panes, breadcrumbs preserve navigation, and exact captured traces or linked PRs open from their evidence (#1131, #1134, #1137, #1140, #1148, #1149, #1155, #1160).
- `lf performance` compares the last 14 days of Task latency, verification time, provider token usage, and reported cost with versioned budgets. Every row publishes eligible/measured coverage beside p50 and p95; missing reports remain `UNKNOWN` rather than becoming estimates or zero (#1162).

## Planning and catalog names resolve through one authority

Linear ownership is now repository-sized: one repository has one Team and issue-key namespace, each Wave has one Initiative, and each Project belongs to exactly one Wave. Task identity resolves through stable Issue, Project, and Initiative ids rather than title or issue-prefix inference (#1128, #1143, #1146).

- `.lf/config.yaml` owns the repository's `pm.linear_team`; Wave goals retain only `pm.linear_initiative`. `lf pm reteam` can migrate all linked Waves and Issues as one resumable operation while deferring Tasks whose active Runs could still write old identifiers.
- Status and roadmap reads remain useful when historical, non-terminal Task Work outlives a Project removed from the current PM snapshot. Loopflow shows the real Project and Task as degraded Wave-owned evidence and names a stable Work-id recovery command; it neither deletes history nor invents PM truth (#1129, #1152).
- `lf list` and `lf -l` now show one combined skill and flow catalog, including authored structure and collapsed execution chains; `lf ls` remains the Wave registry. Public ownership names use slash notation such as `task/pursue` and `wave/clarify`, with no underscore aliases (#1147).
- `docs/architecture.md` is now an executable ownership map spanning concepts, persistence, processes, commands, DTOs, providers, and compatibility seams. Required CI and a weekly retained report reject stale boundaries and retired control vocabulary (#1159).

## Upgrades preserve the control plane

Installation and release paths now treat schema history, paired binaries, and generated notes as parts of the same safety boundary. Source builds remain available for development, but an artifact cannot become the machine-wide launcher unless its embedded migration frontier can run the installed state (#1123, #1132).

- Promotion refuses any candidate built beside draft migrations, even when the live database already sits at the candidate's apparent frontier. The v0.12.4 production migration batch has been restored byte-for-byte to source history, and out-of-range durable timestamps are repaired through a forward migration.
- `scripts/install.py refresh` exits before compilation when main is unchanged, while still bootstrapping a missing or broken target. Draft-bearing checkouts also fail before an expensive build (#1124).
- Release-note generation now falls back to bounded deterministic notes only for typed provider unavailability such as cooldown, rate limits, exhausted quota, lost authentication, or outage. Verification, merge, tag, hosted build, publisher, and GitHub Release gates remain unchanged; unknown failures, stale or malformed output, and oversized notes still fail closed (#1158).

## Operational notes

**Migrate Linear Team ownership before removing legacy configuration.** Run `lf pm reteam` for a dry run, stop any Task Runs that can write old issue identifiers, then run `lf pm reteam --apply`. The resulting canonical binding is `pm.linear_team` in `.lf/config.yaml`; Wave-level provider and Team overrides are no longer supported.

**Update runtime automation.** Replace Feedback/Continue flows with `lf ask` and `lf work answer`, inspect provider conversations through `lf invocation`, use durable Work Steers instead of radio, and keep reviewed Wave memory in `wave/<name>/MEMORY.md`. Audit scripts for removed `lf launch`, radio, `lf receipt`, `lf memory`, and `lf work feedback`/`continue` commands.

**Update catalog references.** Replace public underscore names such as `task_pursue`, `project_clarify`, and `wave_mutate` with `task/pursue`, `project/clarify`, and `wave/mutate`. Run `lf list` to inspect the installed expansion before updating Project lifecycle policies or repo-local flows.

**Configure Discord only on its owning Home.** Set the Wave's `chat.provider`, `home_id`, `guild_id`, and `channel_id`; store `LF_DISCORD_TOKEN` in that Home daemon's Doppler configuration; reinstall or reload `lfd`; then restart the Wave. The bot needs View Channel, Read Message History, Send Messages, and Message Content intent.

**Do not promote draft-bearing source builds.** Canonicalize drafts through the release boundary first. Normal packaged upgrades embed the v0.12.5 migration batch and promote `lf` and `lfd` together.

## Small changes

- PR settlement recognizes both auto-merge requests and actual GitHub merge-queue entries, so landing does not try to replace a head GitHub has already admitted (#1141).
- `lf pr land -c` can complete a final-phase Task against its existing merged PR when the rotated successor is provably empty, while material successors and earlier phases retain the empty-range refusal (#1135).
- Long environment-bearing tmux launches use a mode-0600 self-unlinking script, and SQLite installs bounded contention handling early enough to preserve execution receipts under a 51-process fleet (#1139).
