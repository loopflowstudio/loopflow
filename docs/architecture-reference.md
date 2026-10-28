---
layout: default
title: Architecture Reference
---

# Architecture Reference

## Intent flows down, execution facts flow up

Intent means Tasks, Waves, Projects, Task graph positions, ownership, pools and
budgets. It has one owner of record: the planning provider, or Loopflow's
journal when the plan is local. Execution facts are Processes, Sessions,
GraphProcess rows and checkouts. The Machine that ran them owns them. Upward
copies (worker → controller → laptop) are read-only and keep their origin;
they become the final record only when the origin is gone. Upward copies are
planned, not built. The controller aggregates execution and is not the root.
There are no daemons or listeners; `lf --machine` over SSH is the only
transport between Machines.

This page owns the checked current-source inventory and indexes the
execution contract and its cutover status. [Architecture](architecture.md) introduces the model;
[Execution](architecture/execution.md) and [Data](architecture/data.md) develop it.

## Contents

The checked inventory below stays on this page. The contract lives by area:

- [Cutover status](architecture/reference-cutover.md): Which AgentProcess, Session, Graph and launch cutovers remain open.
- [Models and vocabulary](architecture/reference-models.md): Work, Graph, Process and Session models and their APIs.
- [Code, persistence and public APIs](architecture/reference-code.md): Source territories, persistence and process entry points.
- [Launch, Graphs and delivery](architecture/reference-runtime.md): Harness launch, Task Graphs, communication and delivery.
- [Locks, processes and Machines](architecture/reference-processes.md): OS locks, process control, Machine topology and promotion.

## Complete ownership map

The following inventory describes **current source during the cutover** and is a
checked ownership index. Transitional input projections and wire names below are deletion
dependencies, not additional target product objects.
Every top-level CLI family, live SQLite table, process entrypoint, HTTP route,
provider, and literal subprocess edge must appear exactly once.

Canonical Work readers in `store/sqlite/children.rs` serve Task status,
Project selection and Wave/Desktop lists. They read Task/Project
fields independently of graph position and provider inventory, retaining known
provider conflicts. Local state decisions and delivery receipts commit
together; observed Linear conflicts retire losing intentions while preserving
their receipts and local execution. Project ingestion stores
editable content so KRs and targets survive the shared reader. Task creation/edits
and Project creation/edits, rotation and membership moves use common transactions and field
receipts. Task deletion commits local removal and its pending receipt without changing execution.
Field conflicts go to the change sent last: an unsent save rebases onto the observed
provider value and still delivers, and a provider edit observed after an attempted
write is adopted.
Independent foreground loops deliver creation, fields, ordering, state, deletion
and comments; receipt-backed CLI/Desktop projections expose pending and uncertain
effects. The unconditional-update race remains a protocol limit.

<!-- architecture-map:start -->
| Concept | Truth and authority | Data structure | Persistence | Process owner | Public surface | External edge |
| --- | --- | --- | --- | --- | --- | --- |
| **User** — a person or external harness originating work | User-attributed actions author root input and decide effects that require user intervention. User is actor provenance, not a control credential. | [`Author`](../rust/loopflow/src/durable.rs) | Git supplies `user.name` unless personal Loopflow config overrides it; input records retain source author names. No User row; authored effects persist on the concept they change. | `lf` | `lf :`, `lf open`, `lf config`, `lf config user` | `process:open`, `process:osascript`, `process:pbpaste`, `process:id`, `process:herdr`, `process:tmux`, `process:osacompile`, `process:plutil`, `process:/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister` |
| **Skill** — one reusable prompt with assembled context | Repository/builtin Skill Markdown is authoritative; discovery selects one source. | [`Skill`](../rust/loopflow/src/graph/mod.rs), [`SkillSource`](../rust/loopflow/src/skills/catalog.rs) | `.lf/skills/`, builtin Skill files, installed vendor Skill directories | `lf-prompt` | `lf skill`, `lf self sync-skills`, `lf list`, `lf help` (local command/definition discovery) | `process:python3` |
| **Graph** — definition and one execution | The Graph process compiles the GraphDefinition, including Xor paths, and holds the graph and cursor in memory. It records the graph and each step's node in GraphProcess; a step's result is its process exit. | `GraphDefinition`, `GraphComposition`, `GraphProcess`, typed node ID | `.lf/graphs/`; `graph_processes`, `graph_process_steps` beside the Graph process and step Process rows | One CLI Graph process per Graph; each step is its own child process running the plain command | `lf graph`, `lf task run` | — |
| **Task graph** — the graph a Task took up and the step it stands at | A Task takes up one graph, fixed from then on: the one named on `lf task run`, else its Wave's default, repository `graph:`, or builtin `code`. It stands at `start`, `end`, or an `interactive:` step, where a person takes part in the Task conversation. `lf task run` stops before each interactive step and the next run continues after it; `--from` starts anywhere, and starting from an interactive step only puts the Task there. The child Graph process writes where it left the Task; reaching `end` requests completion. A stopped run leaves the Task on it, and the next `lf task run` starts it again from where it began. `lf ship` repeats `-b task run` until `end`, standing in at each interactive step. Other Graphs in the checkout never move the Task. | [`CapturedGraph`](../rust/loopflow/src/task_graph.rs), [`TaskGraph`](../rust/loopflow/src/task_graph.rs) | committed `.lf/graphs/` files and builtin graphs; a Wave records only the selected name; `task_graphs` (one row per Task) | `lf task run` and the child Graph process it starts; `lf ship` runs to `end` | Task status JSON (`work.graph`), interactive step guidance in a bound Task launch | — |
| **Wave** — durable operating context with goal, memory, cadence, chat, and project selection | The Wave UUID is durable identity, carried in the Wave's committed `config.yaml`. SQLite stores a one-segment name and optional parent Wave ID; the readable address is derived through parents within the canonical repository. Provisioning creates missing goal/memory files; default graphs are repository files or builtins. Checkout files own goal and memory; SQLite owns identity and plan membership in both connection modes; Linear Initiative IDs are optional links. Renaming changes stored addresses, retaining execution; authored files remain in place. Shared setup (`graph`, `agent`, `skill_agents`) is committed in the Wave or repository `config.yaml`; `lf set` stores a person's own override above it on this Machine, and `lf wave status` reports each value's source. | [`Wave`](../rust/loopflow/src/work/wave/mod.rs), [`WaveLocator`](../rust/loopflow/src/work/wave/mod.rs), [`CanonicalRepo`](../rust/loopflow/src/repository.rs), [`WaveConfig`](../rust/loopflow/src/work/wave/config.rs) | `waves`, `scope_settings`; goal and memory are checkout files; historical relocation receipts remain retained | Finite Wave-attributed conversations; relocation commits expected addresses in one store transaction | `lf wave`, `lf wave list`, `lf wave status`, `lf wave show`, `lf wave graph`, `lf set`, `lf unset`, `lf discord` | Discord when configured |
| **Chapter / Project** — shared current plan name and one selected Project per Wave | Project names, summaries and content save locally with per-field delivery/conflict receipts. Creation, selection, activation and rotation save locally with receipts consumed by foreground provider delivery. The repository chapter name is derived from its Waves' In Progress Projects. | `Project`, `ProjectStatus` | `projects`, `project_changes`, `project_events`; Linear Project observations | one local rotation transaction; foreground provider observation | `lf wave edit-plan`, `lf wave new-chapter --all`, `lf wave reteam` | Linear |
| **Live metric** — one reviewed measurement contract owned by exactly one Wave, plus revision-bound current evidence | `.lf/wave/<name>/metrics/*.md` owns meaning and Wave ownership; an accepted instrument observation owns its source-time fact; [`MetricPortfolioDto`](../rust/loopflow/src/work/wave/metrics.rs) is the sole derived reading shared across surfaces. Metrics inform KRs but never complete them. | [`MetricContract`](../rust/loopflow/src/work/wave/metrics.rs), [`MetricObservation`](../rust/loopflow/src/work/wave/metrics.rs), [`MetricPortfolioDto`](../rust/loopflow/src/work/wave/metrics.rs) | `.lf/wave/<name>/metrics/`, `metric_instruments`, `metric_observations` | Metric instruments write observations; foreground Rust readers derive bounded portfolios. | Status/roadmap JSON, Wave and Task prompts, the shared Swift DTO, and Mac Wave detail expose the same `metric_portfolio`. | — |
| **Task** — concrete repository work with optional Project membership | Local Task planning owns directive/status with optional Linear synchronization. The checked-out branch identifies the Task through its checkout placement; the stored worktree path is placement. Git upstream tracking does not select Task identity. Every Graph process for the Task is equally its work; helpers and delivery commands may mutate the worktree without driving a Graph. Git owns commits/branch state; GitHub owns PR/check/merge truth. | [`Task`](../rust/loopflow/src/work/task/mod.rs), [`TaskPr`](../rust/loopflow/src/work/task/mod.rs) | `tasks`, `relocated_task_processes` (observed checkout membership retained through moves), `task_creation_intents`, `task_changes`, `task_state_deliveries`, `task_issue_identities`, `task_deletions`, `task_events`, `task_prs`, `task_pr_repair_incidents`, `task_linear_observations`, `task_linear_ingested_comments`; Linear Issue; Git worktree | `lf task run` places the worktree, then drives a fresh Graph from its execution owner; foreground operations record delivery evidence | `lf task`, `lf ops` (delivery and environment plumbing), `lf ops pr`, `lf ops wt`, `lf ops sync`, `lf ops commit`, `lf scratch` | Linear |
| **PR landing** — one recorded intent to merge an exact PR head | GitHub is authoritative for the PR head, required checks, and merge. One landing generation admits one check at a time; an incident without a recorded response admits one repair. | [`PrLanding`](../rust/loopflow/src/pr_landing.rs), [`CiIncident`](../rust/loopflow/src/work/task/mod.rs) | `pr_landings`, `ci_incidents` | The process holding the claim: `lf ops pr reconcile` to observe and settle, the waiting landing (`lf ops pr land --wait-and-fix`, the ship process) to repair | `lf ops pr arm`, `lf ops pr land`, `lf ops pr reconcile`, `lf ops pr checks` | `provider:github`, model provider for `ci-fix`, `process:git`, `process:gh` |
| **Linear observations** — acquisition and ownership evidence | Repository/provider-scoped Project and issue observations retain provider evidence. Task and Wave readers use saved planning; ingestion preserves pending saves; the change sent last wins, so an unsent save rebases onto an observed provider edit and still delivers. Wave membership and sync observations reference those shared facts; change receipts invalidate admission without rewriting execution history. | [Linear acquisition](../rust/loopflow/src/linear/acceptance.rs) | `pm_projects`, `pm_items`, `pm_wave_projects`, `pm_wave_sync`, `pm_issue_changes`, `pm_project_name_cutover`, `project_binding_imports`, `project_transitions`, `project_transition_items`, `linear_team_checks` | On-demand team acquisition at planning/Graph/write boundaries and native turn-end hooks (`lf __context-block --moment stop`); watchers alone retain a slow clock. SQLite shares attempt freshness and the successful watermark; a Home-wide cooldown retains the reset deadline | `lf wave status --sync`, `lf task status` | `provider:linear` |
| **Asana and Notion planning** — the other remote planning providers | A repository names one team in `planning.team`. Asana keeps Tasks in the team project named after the repository and Wave Projects as team projects; Notion's team page holds linked Tasks and Projects databases. Both return the shared Linear observation types through one `PlanningClient`, so acceptance, receipts and steering stay on the Linear path. | [Asana](../rust/loopflow/src/asana/mod.rs), [Notion](../rust/loopflow/src/notion/mod.rs), [planning client](../rust/loopflow/src/ops/planning_client.rs) | Shares the Linear observation and receipt tables | Same planning/Graph/write boundaries as Linear | `lf ops account connect asana`, `lf ops account connect notion` | `provider:asana`, `provider:notion` |
| **Planning exchange** — shared Work changes across Machines | Canonical Work owns planning; repository configuration selects Git or Linear. Peer changes retain causal history and conflicts without importing execution or delivery authority. | [`PlanningSnapshot`](../rust/loopflow/src/planning_exchange.rs), [`PlanningGit`](../rust/loopflow/src/planning_git.rs), [`PlanningConfig`](../rust/loopflow/src/config.rs) | `repository_plans`, `planning_destinations`, `planning_peer_routes`, `planning_peer_changes`, `planning_peer_conflicts`, `planning_peer_imports`, `planning_peer_observed`, `planning_user` | The shared planning writer saves/imports; foreground exchange observes and publishes configured refs. | `lf config planning`, ordinary Work reads and saves | Git refs |
| **Steer** — message guiding work | Immutable source/revision UUID; existing associations select Sessions | [`Steer`](../rust/loopflow/src/steer.rs), [`SessionSteerStatus`](../rust/loopflow/src/steer.rs) | `steers`, `session_steers`, `steer_batches`, `steer_deliveries`; `steer_adapters` binds the callback capability to an AgentProcess; callbacks follow the Session's selected native conversation, which a provider start (including `/clear`) moves along with the resume reference. `task_comments` and `task_comment_deliveries` retain the separate planning thread and Linear sync | Foreground Task connections acquire comments independently of pending writes | `lf task comment`, Linear issue comments | SQLite |
| **Tool response** — one idempotent response to a Work-scoped tool request | Stable Work identity plus request id names the response slot; a second, different answer is rejected. | [`ToolResponseWrite`](../rust/loopflow/src/durable.rs), [`ToolResponseReceipt`](../rust/loopflow/src/durable.rs) | `tool_responses` | Store transaction | Internal Work store API | — |
| **LfSession** — one conversation | Session row owns name, ancestry, readiness, completion and publication. Its current capture references an immutable history event written at reservation. Earlier events retain caller and Work attribution. Complete returns saved feedback; the following typed decision chooses navigation. | `SessionRecord`, `SessionId` | `agent_sessions`, `session_events`; `session_activity` holds attachment stream readings and terminal-reported status | Native turn observation retains start/usage/completion; Codex protocol replies record native identity; Claude retains its supplied ID and OpenCode confirms its native ID through the private server API; `lf __context-block --moment pi-event` retains Pi native identity, activity and cumulative usage; `--moment tool-group` admits the existing watchdog into Pi-owned detached tool groups; Session operations own state | `lf session`, interactive `lf` | `process:cmux` projects the Task title onto its workspace and the attached Session name onto its tab (tmux, mapped under User, onto its window) without owning either |
| **Machine / Promotion** — stable machine identity and artifact selection | `MachineId` is identity; SSH route is mutable. Each person's Machine-local Wave and repository Machine defaults (`lf set machine`), copied to their other Machines, place their unowned Work and, separately, their Wave or repository Session; it is never process ownership or repository content. Promotion owns immutable artifact selection, isolated schema proof, app replacement, and rollback only. Install selects the latest published release independently of caller Git state; the laptop schedule invokes that same command. Checkout updates belong to sync. | [`Machine`](../rust/loopflow/src/durable.rs), [`scope_defaults`](../rust/loopflow/src/store/sqlite/scope_defaults.rs), [`SwitchReceipt`](../rust/loopflow/src/installation.rs), [`published installation`](../rust/loopflow/src/lf/commands/install/published.rs) | `machines`; Machine-local SQLite `scope_defaults`; installation selection and switch receipts; laptop refresh LaunchAgent | The promotion command owns its OS-locked switch transaction | `lf ops machine`, `lf set machine`, `lf self`, `lf --machine`, `lf self install`, `lf self install schedule` | `process:ssh`, `process:launchctl`, `process:systemctl`, `process:/usr/bin/open`, `process:/usr/bin/osascript`, `process:brew`, `process:/bin/sh` |
| **Rental / Pool** — a machine rented for bounded time and money, or borrowed from an owned pool | The provider's sandbox is the fact; the rental row records admission (price, cap, deadline) and state, and the provider, or the guardian on the machine, deletes it at the deadline. An on-demand is a provider and hardware with a concurrency and hourly-price limit, and may be a Wave's or repository's Tasks capacity; a pool is owned machines lent one at a time. Giving back copies the machine's records first; `--force` skips that copy. Machines rented by hand stay rented. | [`Rental`](../rust/loopflow/src/rental/mod.rs), [`OnDemand`](../rust/loopflow/src/rental/mod.rs), [`Pool`](../rust/loopflow/src/rental/mod.rs), [`Sandboxes`](../rust/loopflow/src/rental/sandbox.rs) | `ondemands`, `pools`, `rentals`, `machine_records`, `scope_defaults.tasks_capacity` | The renting command owns admission, boot and give-back; the provider owns the deadline | `lf ops machine ondemand`, `lf ops machine pool`, `lf ops machine end`, `lf set machine <capacity>`, `lf task run --rent`, `lf task run --pool` | `process:uv` runs the Daytona, Modal, RunPod or AWS bridge; `process:ssh-keygen` forgets a reused tunnel's host key; then the Machine row's SSH edge |
| **Session history projections** — captured events and exact provider evidence | LfSession owns provider outcomes and Process owns command outcomes; original payload and exact process receipts confer no Graph authority. | `SessionCaptureSpec`, `SessionCaptureManifest`, `SessionHistory`, `ProviderHistory`, `SessionUsage` | Projects LfSession-owned input/history; Machine-local `runs/<prefix>/<run-id>/` immutable payload and process receipts | shared conversation admission and history | `lf history show`, `lf history replay`, `lf history usage`, `lf history`; Work/status history | `process:lf`, provider harnesses |
| **LfProcess** — one actual lf process | The journal transaction records command completion and fixes each child's causal parent at admission. Agent provenance grants no control authority. A detached transport Process owns execution and terminal state; viewers hold separate revocable input tokens. | [`LfProcessId`](../rust/loopflow/src/id.rs), [`AgentCaller`](../rust/loopflow/src/process.rs), [`transport`](../rust/loopflow/src/transport.rs) | `processes` (`kind='lf'`); Session rows reference their transport Process | Foreground commands and detached execution owners (`lf session serve`, launched by `transport::start`); installation/bootstrap coverage remains a cutover obligation | `lf monitor`, `lf history list`, `lf session attach`; ordinary parsed CLI commands | — |
| **AgentProcess** — the provider's OS process | Its original parent and served Session survive attachment transfer. PID/birth and terminal evidence belong to the record; a fresh token fences each attachment. | `AgentProcess`, `SessionAttachment`, common `LfProcess` projection | LfProcess's table, rows of `kind='agent'`; Session retains its current record reference | Provider launch and observed settlement; typed admission remains open | `lf monitor top`, Task execution inventory; LfProcess's process readers | Provider OS process; `process:pi` owns its RPC or native conversation, tools and credentials |
| **Disposable data clone** — offline development data and explicit provider-history archives | SQLite backup and clone-only draft preparation own the disposable schema. A fresh Machine and removed credentials/claims separate retained history from authority; the inherited macOS sandbox confines descendants. Ordinary opens remain validation-only. | [`CloneCommand`](../rust/loopflow/src/data_clone.rs), [usage and proof limits](data-clones.md) | Private clone directory and manifest; copied Machine-local SQLite, captures and archival provider JSONL | Explicit foreground create/run/dispose; no resident service | `lf clone` | `process:/usr/bin/sandbox-exec` |
| **Fake world** — a built world for demos and tests | Recipes in code write through the same ops and store writers as `lf`, so the current schema owns every row; Graphs compile from their authored definitions and Tasks stand at the steps `lf task run` leaves them at. Times are relative to the build. Nothing is shared with a real Machine. | [`World`](../rust/loopflow/src/world/mod.rs), [`RECIPES`](../rust/loopflow/src/world/recipes.rs) | New world directory: its own `LF_HOME`, provider homes, git repository and bare origin, a `bin/gh` that answers PR reads from `github/pulls/` and refuses the rest, and `env.json` (with `PATH` putting that `gh` first) | `lf-world` | `lf-world build`, `lf-world list` | — |
| **Local process observation** — recorded processes joined to OS facts | Lf receipts and AgentProcess PID/birth establish observations, not new ownership. Detached records and unknown identities remain visible; failed OS sampling never proves absence. Duplicate or unknown identities grant no signal authority. Prune closes rows whose exit is proven, forgets finished rows nothing references, merges per-receipt usage and reclaims store pages; a dry run changes nothing. | [`ActivitySnapshot`](../rust/loopflow/src/lf/commands/top.rs), [`ProcessPruneReport`](../rust/loopflow/src/lf/commands/top.rs) | Process rows and Machine-local lf receipts; no provider registry | Foreground observation and scheduled record settlement | `lf monitor ps`, `lf monitor top`, `lf monitor prune`, `lf self doctor` | `process:/bin/ps`, `process:ps`, `process:sysctl`, `process:lsof`, `process:kill`, `process:which` |
| **Disk cleanup** — reclaiming a Machine's disk across Home and every repository under `~/src` | Cleanup policy is a maintenance script, not lf product surface. Home captures, logs, draft store copies and all but the newest backup per database go once untouched for a week; the database and its sidecars, account homes, Home material modified within the last day and anything a process holds open never go. Worktree, build-output and branch judgment stays with `lf wt prune`; the script judges idle checkouts that judgment keeps and removes only ignored build output untouched for four hours in checkouts no process holds, and the repository skill only reports. An unreadable process view stops removal. | [`disk-cleanup.sh`](../scripts/disk-cleanup.sh), [`disk-cleanup` skill](../.lf/skills/disk-cleanup.md) | Filesystem only; removals append to `~/.lf/logs/disk.log` | Hourly crontab entry running the skill from the main checkout | `scripts/disk-cleanup.sh install`; `lf monitor prune` for lf's own Process records | Open-file inspection (`lsof`); `lf wt prune` |
| **Provider account / route** — credential authority and ordered provider selection on one Machine | Provider token/account rows and Access Profiles own routing; credentials stay in provider homes, encrypted storage or Doppler. Machine connection installs a separate resident login. | [`Provider`](../rust/loopflow/src/provider_auth/mod.rs), [`AccessProfile`](../rust/loopflow/src/profile.rs), [`ProviderRoute`](../rust/loopflow/src/profile.rs), [`ProviderAccount`](../rust/loopflow/src/store/mod.rs) | `access_profiles`, `auth_browser_bindings`, `provider_accounts`, `provider_account_limits`, `provider_account_switches`, `provider_routes`, `provider_session_accounts`, `provider_tokens` | The foreground auth command owns provider login process groups and passive browser handoff; durable processes use credentials installed on their Machine | `lf ops account` | `provider:claude`, `provider:codex`, `provider:doppler`, `provider:opencodezen`, `process:claude`, `process:codex`, `process:doppler`, `process:opencode`, `process:security`, `process:secret-tool` |
| **Conversation context** — refreshed source block and authoring targets | Fixed additions preserve native instructions; skill/request/clipboard form the first turn. Saved source identities reread checkout files at startup and compaction, within 10,000 rendered UTF-8 bytes. Configured budgets measure authored memory/scratch, never gate launch. | [`ContextDelivery`](../rust/loopflow/src/context_block.rs), [`ContextBudgets`](../rust/loopflow/src/prompt/context_budget.rs), [`ContextBudgetReport`](../rust/loopflow/src/prompt/context_budget.rs) | Checkout sources and complete private references under `.lf/prompts/`; no measurement store | `lf __context-block` serves provider startup/compaction hooks; foreground size reporting. Missing optional capture payload does not erase native resume identity; malformed retained payload still fails | `lf context` | — |
| **Code-size measurement** — repository blobs measured in model tokens | Git blob identity owns content; token counts are deterministic memoized measurements, not Run usage. | [`CodeNode`](../rust/loopflow/src/lf/commands/tokens.rs), [`CodeSnapshot`](../rust/loopflow/src/lf/commands/tokens.rs) | `blob_tokens` | Foreground command only | `lf ops tokens` | — |
| **Task owner identity** — the GitHub user behind a Task owner's Git email | Claims carry the owner's email; GitHub's commit authorship resolves it to a user once, and the answer is cached. A miss is retried after a day; an unreachable or unauthenticated `gh` caches nothing. | [`RepoPerson`](../rust/loopflow/src/ops/people.rs), [`GithubPerson`](../rust/loopflow/src/ops/people.rs) | `github_people` | Foreground command only | `lf ops people` | GitHub REST through `gh` |
| **Store change revision** — which displayed domain a commit changed | Derived inside the writer's transaction by schema triggers, never authored and never bumped at a call site. Transcript lines move nothing; usage moves only its own revision, which no watch reads again for; Wave detail shows token totals as of its last reading. Each `--watch` reading pins revisions and database-derived joins in one read transaction; nested readers use savepoints. Events only say when to compare revisions. A confirmation request makes a watch answer with a reading begun after it. OS/Git/file observations are not transactional facts. | [`StoreRevisions`](../rust/loopflow/src/store/sqlite/revisions.rs), [`WatchFrame`](../rust/loopflow/src/lf/commands/watch.rs) | `store_revisions` | Store transaction | `--watch` on the ordinary reads | — |
| **Schema frontier** — ordered definition of durable control storage | Released migration bytes are immutable authority; drafts join only through deterministic release materialization. | [`Migration`](../rust/loopflow/src/store/migration_catalog.rs), [`MigrationId`](../rust/loopflow/src/store/migration_catalog.rs) | `schema_migrations`; canonical and draft migration files | Store open validates/applies; release cut publishes | `lf ops release` | `process:sh` (release hooks) |
| **Task ownership** — (person, Machine) or unowned | An append-only log; the later entry wins in provider order. Exactly one writer appends, always one of the caller's own Machines: their Machine holding the claim, else their Wave Machine; nothing routes to another person's Machine. Entries travel as marked Task comments. An inbound entry never ends this Machine's own open claim. | [`TaskOwnership`](../rust/loopflow/src/durable.rs), [`TaskOwnershipEntry`](../rust/loopflow/src/durable.rs) | `task_ownership` (append-only by trigger) | The writer's IMMEDIATE transaction, reached over `lf --machine` | `lf task claim`, `lf task move <task> --machine <machine>`, `lf task unclaim`, `lf task run`, `lf task checkout` | Linear assignee; SSH |
| **Portfolio** — repository selection on one Machine | Rust owns folder discovery and manual membership; Desktop reads and changes the same selection. Removing membership preserves files and retained Work. | [`Portfolio`](../rust/loopflow/src/portfolio.rs), `PortfolioMode`, `PortfolioRepo` | Machine-local config.yaml; locked atomic writes | Foreground CLI; Portfolio controls call its API | `lf portfolio` | — |
| **Desktop workspace** — retained presentation of repository, Wave or Task | Work identity selects one repository window and retained workspaces through the sidebar. Pane contents own Machine locations; exact window/content/surface lifetimes fence mutations, which all run through the Swift `DesktopAction` registry. Reads acquire no client and receipts promise no readiness. | `WorkWorkspaceKey`, `DesktopInspection`, `DesktopPaneTarget`, `DesktopActionRequest`, `DesktopActionReceipt` | `dashboards`, `dashboard_arrangements`, `task_orders`, `work_pane_layouts` for saved Dashboards, Task order and portable pane layouts; window-owned registry, multiplexer and native surfaces | Loopflow.app on macOS | `lf desktop` | `$LF_HOME/run/desktop/<bundle-id>.sock` |
<!-- architecture-map:end -->

The public API column covers top-level command families, not every subcommand or
Rust function. [`lf` reference](lf-reference.md) owns argument-level detail. DTOs emitted
by `--json` are required-field projections; Rust/Swift fixture tests own their
wire parity.

## Truth and projections

The map is the ownership index. Truth remains distributed across Machine-local
SQLite, repository files and Git, the command journal, Linear, GitHub, and provider
homes or Doppler; none is a fallback authority for another.

Intentional copies stay read projections:

<!-- architecture-projections:start -->
| Projection | Authority copied | Freshness and consumer |
| --- | --- | --- |
| [Linear acquisition](../rust/loopflow/src/linear/acceptance.rs) / `pm_projects`, `pm_items`, `pm_wave_projects`, `pm_wave_sync`, `pm_issue_changes` | Linear planning | Shared entity ingestion orders provider revisions and retains last-good facts when refresh fails. Canonical Task and Wave views read saved Work with separate acquisition evidence; ownership checks read retained claims directly. Inventory and detail acceptance consume concrete Linear fields in one transaction; there is no public snapshot envelope. Observation age and invalidation remain explicit. |
| [`TaskLinearObservation`](../rust/loopflow/src/work/task/mod.rs) / `task_linear_observations` | Linear Issue title, description and comment revisions | Reconciliation saves comments and appends deduplicated direction. Team acquisition also accepts issue state and Project membership through the typed Linear owner; the saved comment projection grants no graph or Process authority. |
| [`GithubObservation`](../rust/loopflow/src/work/task/mod.rs) / `task_prs`, `ci_incidents` | GitHub PR/check state | Webhook or foreground reads update Task delivery evidence; GitHub remains merge truth. |
| `tests/fixtures/dto/` | Rust `lf --json` DTOs | Rust and Swift fixture tests reject required-field or enum drift. |
| `tests/fixtures/migrations/` | Ordinal-free migration drafts and the Python canonicalizer | Rust build/runtime and Python release tests reject ordering, body-byte, checksum, and graph-error drift. |
<!-- architecture-projections:end -->

`lf wave status` and `lf wave show` derive Task conditions from Work, Process and
PR facts. LfSession and Process own ancestry; immutable history keeps
earlier attribution. The transitional joins are listed in the cutover status. No projection acquires launch, Work-mutation,
credential or signal authority. UI grouping is cached presentation, not another
attribution store.

## Extension rules

| Area | Safe extension | Architectural constraint |
| --- | --- | --- |
| Execution queries | Add an index for a measured Process or LfSession query | Filter before payload IO; identity/ancestry come from the owning row, never fallback files |
| Multi-Machine views | Fan out read-only commands through `lf --machine` | Do not centralize Machine-local execution ownership or silently mix local and remote scope |
| Process control | Publish birth-validated ownership at the launcher spawn seam | No PID/tmux/Work/telemetry inference |
| Planning input | Add a naturally keyed fact or provider observation | Do not create a global input revision protocol |
| Provider support | Add a provider adapter, account route, and normalized stream mapping | Provider credentials/finality remain provider-authored |
| Promotion | Add artifact roles or service adapters within the locked switch transaction | Artifact activation does not depend on conversation discovery |

A new writer must have one authoritative model. Avoid backend dispatch between
old and new representations, dual authoritative SQLite/filesystem records, mandatory
collector daemons, or planning capabilities derived from observation data.

## Appendix: compatibility seams

Compatibility survives only when it crosses immutable external history. Each
seam names its translation and deletion boundary; none is a second current
model.

<!-- architecture-shims:start -->
| Seam | Current concept | Source and removal boundary |
| --- | --- | --- |
| `shim:retired-app-replacement` | Promotion removes the previously shipped app bundle after the current app commits. | [`AppPromotion`](../rust/loopflow/src/lf/commands/install.rs); remove after the retired bundle name is outside supported installs. |
<!-- architecture-shims:end -->

## Appendix: historical-only vocabulary

The scanner matches exact phrases, not overloaded words. Provider resume
sessions are current. The authored chat
reference `project:<slug>` is also current; it is not the old Linear-label PM
model.

<!-- architecture-vocabulary:start -->
| Retired term | Allowed scopes | Current language |
| --- | --- | --- |
| `Project Session`, `project_sessions`, `task_sessions` | `rust/loopflow/src/store/migrations/`, `rust/loopflow/src/store/migrations.rs`, `rust/loopflow/src/store/tests/fixtures/`, `release/` | Stable Project/Task **Work**; LfSession owns the conversation and native history; a Graph is a Graph process and its step processes. |
| `session context`, `LF_SESSION` | — | Typed Work ancestry and Process/LfSession provenance; transitional launch environment names are listed in cutover status. |
| `lf radio`, `agent bus` | `release/` | Typed Work observations, Steer, synchronous questions, and review GraphSteps. |
| `pm.linear_project`, `projects/<slug>.md` | `release/`, `rust/loopflow/src/ops/project.rs` | `linear.initiative`; Linear Initiative → Project → Issue. The Project operation reads the historical YAML field once into SQLite and retains original bytes; later file edits are inert. |
| `machine-local host`, `machine-global command`, `machine-global mutation`, `machine-global reservation` | — | Machine-local keeper, command, mutation, or reservation. |
<!-- architecture-vocabulary:end -->

The following map gives the destination of retired or transitional representations.
Cutover status identifies which runtime consumers remain; the map does not claim
their deletion is finished.

| Historical or transitional representation | Target owner |
| --- | --- |
| Wave-scoped Chapter / `wave_chapters` | Linear Project status; Chapter is the shared In Progress name, with no stored object |
| Recommended Graph / `graphs.recommended` | Wave or repository default graph |
| `GraphPosition`, `PinnedTaskGraph`, `task_graph_positions` | The Graph process's in-memory cursor; GraphProcess records the graph and each step's node |
| `GraphRun`, `graphs/<id>/position.json` | The Graph process and its step processes |
| Subject selector list on a Run | Typed LfSession/Process ancestry and immutable event attribution |
| Four Session projections, Ask files, composite boundary Session IDs | `agent_sessions`, keyed by stable Session ID, with attached LfProcess and native history |
| Session name/resolution and provider attachment sidecars | LfSession attributes, native identity and exact process evidence |
| Step occurrence / path-string node key | Graph process ID, local node key and iteration tuple on the GraphProcess step row |

Canonical migrations, migration fixtures, and release notes retain historical
names because changing shipped evidence would rewrite history. Operational docs
and current runtime source do not. Dated operational archives belong outside
the tracked source tree.

## Authority and failure invariants

- SQLite owns planning with optional Linear synchronization; observed Linear
  conflicts win while losing intentions remain history. A Chapter is the shared
  name of one In Progress Project per Wave; uncertain delivery stays visible.
- Wave instruments and observations survive Chapter changes. Missing target
  planning is unknown, not proof that an instrument is untargeted; it cannot
  erase an observed reading.
- An agent launch needs no planning parent, but requires writable admission and
  immutable captured input before provider side effects.
- LfProcess is an actual lf process. LfSession history owns native outcomes and
  usage; a Graph is a Graph process and its step processes, whose exits are its results.
- Command outcome, native completion, current liveness and Work disposition are
  separate facts. Missing terminal evidence stays unknown.
- Causal parentage and Task attribution confer neither process control nor Graph
  settlement. Exact process identity and native scope govern signaling.
- An authoritative conversation attachment is singular; its claim token differs
  from provider identity. Passive readers acquire neither claim.
- A Graph consumes its selected successful native completion under version/claim
  fencing. Failed turns cannot donate verdicts or routes to successful retries.
- Complete closes a conversation or returns review feedback. Readiness and provider
  exit never choose an edge; a following decision owns its own navigation.
- Bind is write-once, same-target idempotent and valid for done Tasks. CLI states
  the permanent target and writes; Desktop confirms. Historical attribution and
  the ownership log survive assignment and attachment changes.
- OS locks cover their named critical sections. Sync fences agents only while it
  merges or publishes; causal identity does not bypass Git or PR mutation authority.
- Store copies preserve evidence without acquiring process authority. Promotion
  and historical-writer continuity remain separate proof obligations.
- Reads are Machine-local unless explicitly routed by `lf --machine`. Indexed summaries
  precede payload IO; retired files are never identity fallback.
- DTO fields are required unless explicitly optional; Rust/Swift consumers and
  fixtures migrate together.

## Drift proof

```bash
uv run python scripts/check_architecture.py
```

The bounded check materializes the live schema (including drafts), discovers
root CLI families, binaries/internal process commands, any local HTTP routes,
provider kinds, literal Rust subprocess edges, read projections, declared
shims, and exact stale vocabulary. Every discovered item must occur exactly
once in the map or its named inventory. It validates the map's source links and
reports mapped/discovered counts. The vocabulary scan covers active top-level
docs, product docs, prompts, scripts, website code, production Python/Rust/Swift
trees, migration SQL, and release history. Generated `website/docs/` is excluded
because the authoritative `docs/` source is already scanned. Historical
allowances must shelter at least one current match, so dead scopes fail instead
of becoming a permanent allowlist; declared compatibility seams must retain
their exact source marker. The check does not pretend to interpret every Rust
type or sentence.

CI runs the same command for every proposed merge. The weekly Architecture
Drift workflow retains the JSON result as time-based evidence. A new owner,
projection, shim, or API either maps to an existing concept or updates this page
in the same change.

A Task Session is an LfSession associated with a Task checkout. A Task can have
any number; repo and Wave Sessions retain their explicit scopes. This term does
not restore the retired Task Session controller or a second conversation store.
`session list --orphan` selects Sessions without Task association; it cannot opt
a Session out of checkout membership.
