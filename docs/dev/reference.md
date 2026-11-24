---
layout: default
title: Architecture Reference
---

# Architecture Reference

## Intent flows down, execution facts flow up

Intent means Tasks, Waves, Wave planning, Task graph positions, ownership, pools and
budgets. It has one owner of record: the tracker, or Loopflow's
journal when the plan is local. Execution facts are Processes, Sessions,
GraphProcess rows and checkouts. The Machine that ran them owns them. Upward
copies (worker → controller → laptop) are read-only and keep their origin;
they become the final record only when the origin is gone. Upward copies are
planned, not built. The controller aggregates execution and is not the root.
`lf --machine` routes commands over SSH to execution destinations. A separate
local server exposes existing operations to Desktop; LOO-718 extends client
access to mobile. Server connectivity does not transfer execution ownership.
The [accepted responsibility split](server.md) separates Machine
routing, client admission and installation.

The Rust application owns Clap, server dispatch and compiled build identity.
[`loopflow-execution`](../../rust/loopflow-execution/README.md) owns the shared
operations, provider conversations, native lifetimes and tracker fetches above
synchronous storage. Model, definitions, storage and terminal state remain
separate private libraries, and so does
[`loopflow-harness`](../../rust/loopflow-harness/README.md): what each native
client writes, beside storage and below execution. Command-only edits leave execution reusable; the
application supplies its running identity and command semantics at their edges.

This page owns the checked current-source inventory and indexes the
execution contract and its cutover status. [Architecture](index.md) introduces the model;
[Execution](execution.md) and [Data](data.md) develop it.

## Contents

The checked inventory below stays on this page. The contract lives by area:

- [Client API server](server.md): Machine routing, client access and installation boundaries and installed-state migration.
- [Cutover status](reference-cutover.md): Which AgentProcess, Session, Graph and launch cutovers remain open.
- [Settings ownership and resolution](settings.md): File-backed declarations, source/scope/target, the catalogue and remote preference authority.
- [Models and vocabulary](reference-models.md): Work, Graph, Process and Session models and their APIs.
- [Code, persistence and public APIs](reference-code.md): Source territories, persistence and process entry points.
- [Launch, Graphs and delivery](reference-runtime.md): Harness launch, Task Graphs, communication and delivery.
- [Locks, processes and Machines](reference-processes.md): OS locks, process control, Machine topology and promotion.

## Complete ownership map

The following inventory describes **current source during the cutover** and is a
checked ownership index. Transitional input projections and wire names below are deletion
dependencies, not additional target product objects.
Every top-level CLI family, live SQLite table, process entrypoint, HTTP route,
provider, and literal subprocess edge must appear exactly once.

`work::wave::bootstrap` reads authored Wave IDs and seeds missing files before
SQL compares saved repository and hierarchy inputs and commits them together.
Prepared files survive a rolled-back write so retries retain IDs and later edits.
Session admission discovers its repository in `store::sessions`; primary-scope
reads decode the saved canonical address even after the checkout is gone.

Canonical Work readers in `store/sqlite/children.rs` serve Task status,
planning selection and Wave/Desktop lists. They read Task/planning
fields independently of graph position and provider inventory, retaining known
provider conflicts. Local state decisions and delivery receipts commit
together; observed Linear conflicts retire losing intentions while preserving
their receipts and local execution. Planning ingestion stores
editable content so KRs and targets survive the shared reader. Task creation/edits
and plan creation/edits and membership moves use common transactions and field
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
| **User** — a person or external harness originating work | User-attributed actions author root input and decide effects that require user intervention. User is actor provenance, not a control credential. | [`Author`](../../rust/loopflow-model/src/durable.rs) | Git supplies `user.name` unless personal Loopflow config overrides it; input records retain source author names. No User row; authored effects persist on the concept they change. | `lf` | `lf :`, `lf open`, `lf config`, `lf config user` | `process:open`, `process:osascript`, `process:pbpaste`, `process:id`, `process:herdr`, `process:rex`, `process:tmux`, `process:osacompile`, `process:plutil`, `process:/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister` |
| **Skill** — one reusable prompt with assembled context | Repository/builtin Skill Markdown is authoritative; discovery selects one source. | [`Skill`](../../rust/loopflow-execution/src/graph/mod.rs), [`SkillSource`](../../rust/loopflow-execution/src/skills/catalog.rs) | `.lf/skills/`, builtin Skill files, installed vendor Skill directories | `lf-prompt` | `lf skill`, `lf self sync-skills`, `lf list`, `lf help`, `lf docs` (local command/definition/doc discovery) | `process:python3` |
| **Graph** — definition and one execution | The Graph process compiles the GraphDefinition, including Xor paths, and holds the graph and cursor in memory. It records the graph and each step's node in GraphProcess; a step's result is its process exit. | `GraphDefinition`, `GraphComposition`, `GraphProcess`, typed node ID | `.lf/graphs/`; `graph_processes` and `graph_process_steps` carry each run's and step's pid, OS start, end, exit and error | One CLI Graph process per Graph; each step is its own child process running the plain command | `lf graph`, `lf task run` | — |
| **Task graph** — the graph a Task took up and the step it stands at | A Task takes up one graph, fixed from then on: the one named on `lf task run`, else its Wave's default, repository `graph:`, or builtin `code`. It stands at `start`, `end`, or an `interactive:` step, where a person takes part in the Task conversation. `lf task run` stops before each interactive step and the next run continues after it; `--from` starts anywhere, and starting from an interactive step only puts the Task there. The child Graph process writes where it left the Task; reaching `end` requests completion. After each step the child Graph process records where the run goes next, so a failed attempt starts again at the step that failed, and a stopped run's next `lf task run` starts there; the steps before it stay done. `lf task auto on` is a run that turns auto on: it stands in at each interactive step and runs to `end`. Other Graphs in the checkout never move the Task. | [`CapturedGraph`](../../rust/loopflow-execution/src/task_graph.rs), [`TaskGraph`](../../rust/loopflow-execution/src/task_graph.rs) | committed `.lf/graphs/` files and builtin graphs; a Wave records only the selected name; `task_graphs` (one row per Task) | `lf task run` and the child Graph process it starts; `lf task auto on` runs to `end` | Task status JSON (`work.graph`) | — |
| **Wave** — durable operating context with goal, memory, cadence, chat, and planning selection | The Wave UUID is durable identity, carried in the Wave's committed `config.yaml`. SQLite stores a one-segment name and optional parent Wave ID; the readable address is derived through parents within the canonical repository. Provisioning creates missing goal/memory files; default graphs are repository files or builtins. Checkout files own goal and memory; SQLite owns identity and plan membership in both connection modes; Linear Initiative IDs are optional links. Create seeds authored files and the first plan. Rename retains identity and execution; remove moves open Tasks and retains history. Both update authored files on the main checkout and retain pending file/provider effects for retry. Shared setup (`graph`, `agent`, `skills`, `graphs`) is committed in the Wave or repository `config.yaml`; `lf set` stores a person's own override above it on this Machine, and `lf wave status` reports each value's source. | [`Wave`](../../rust/loopflow-execution/src/work/wave/mod.rs), [`WaveLocator`](../../rust/loopflow-execution/src/work/wave/mod.rs), [`CanonicalRepo`](../../rust/loopflow-execution/src/repository.rs), [`WaveConfig`](../../rust/loopflow-execution/src/work/wave/config.rs) | `waves`; personal declarations in configuration files; goal and memory are checkout files; `wave_lifecycle_operations` retains Machine-local effect receipts outside planning exchange | Finite Wave-attributed conversations; relocation commits expected addresses in one store transaction | `lf wave`, `lf wave list`, `lf wave status`, `lf wave show`, `lf wave graph`, `lf set`, `lf unset`, `lf discord` | Discord when configured |
| **Wave Project** — one current Project per Wave, a Linear Project when connected | Project names, summaries and content save locally with per-field tracker actions. Creation, selection and activation save locally and queue actions the foreground drain sends; a Project is replaced in place and there is no rotation. | `Project`, `ProjectStatus` | `projects`, `project_changes`, `project_events`; Linear Project events | one local selection transaction; foreground tracker events | `lf plan`, `lf wave edit-project`, `lf wave update-project`, `lf wave plan bind`, `lf wave plan reteam` | Linear |
| **Live metric** — one reviewed measurement contract owned by exactly one Wave, plus revision-bound current evidence | `.lf/wave/<name>/metrics/*.md` owns meaning and Wave ownership; an accepted instrument observation owns its source-time fact; [`MetricPortfolioDto`](../../rust/loopflow-storage/src/metric.rs) is the sole derived reading shared across surfaces. Metrics inform KRs but never complete them. | [`MetricContract`](../../rust/loopflow-execution/src/work/wave/metrics.rs), [`MetricObservation`](../../rust/loopflow-execution/src/work/wave/metrics.rs), [`MetricPortfolioDto`](../../rust/loopflow-storage/src/metric.rs) | `.lf/wave/<name>/metrics/`, `metric_instruments`, `metric_observations` | Metric instruments write observations; foreground Rust readers derive bounded portfolios. | Status/planning JSON, Wave and Task prompts, the shared Swift DTO, and Mac Wave detail expose the same `metric_portfolio`. | — |
| **Task** — concrete repository work with optional Wave membership | Local Task planning owns directive/status with optional Linear synchronization. The checked-out branch identifies the Task through its checkout placement; the stored worktree path is placement. Git upstream tracking does not select Task identity. Every Graph process for the Task is equally its work; helpers and delivery commands may mutate the worktree without driving a Graph. Git owns commits/branch state; GitHub owns PR/check/merge truth. | [`Task`](../../rust/loopflow-storage/src/work/task/mod.rs), [`TaskPr`](../../rust/loopflow-storage/src/work/task/mod.rs) | `tasks` (shared planning only), `task_checkouts` (this Machine's checkout; no row elsewhere), `task_primary_sessions` (this Machine's Task conversation), `task_moves` (checked/leaving/sent/arrived moves a retry reads), `relocated_task_processes` (Process membership after a repository directory moves on this Machine), `task_auto` (auto on or off, who, when; a row per Task that has it set), `task_ship_events` (the released ship log the auto rows were filled from; unread, leaving with LOO-971), `task_summaries` (one row per ended Task, derived at completion or abandonment: Graph, loop passes, last auto setting, wall time, the last `reflect` line), `task_creation_intents` (what a Task was created with), `tracker_actions` (the outbound tracker queue, shared with Projects and Waves), `task_changes`, `task_state_deliveries` and `task_comment_deliveries` (copied into the tracker queue; unread until their contract migration), `task_events`, `task_prs`, `task_linear_observations`, `task_linear_ingested_comments`, `task_retirements` (completed-checkout cleanup pinned to completion; Session viewer holds live on `held_terminals.viewer_hold`); Linear Issue; Git worktree | `lf task run` places the worktree, then drives a fresh Graph from its execution owner; foreground operations record delivery evidence | `lf task`, `lf pr`, `lf wt`, `lf sync`, `lf commit`, `lf scratch` | Linear |
| **PR landing** — one recorded intent to merge an exact PR head | GitHub is authoritative for the PR head, required checks, and merge. One landing generation admits one check at a time; an incident without a recorded response admits one repair. | [`PrLanding`](../../rust/loopflow-storage/src/pr_landing.rs), [`CiIncident`](../../rust/loopflow-storage/src/work/task/mod.rs) | `pr_landings`, `ci_incidents` | The process holding the claim: `lf pr reconcile` to observe and settle, the waiting landing (`lf pr land --wait-and-fix`) to repair | `lf pr arm`, `lf pr land`, `lf pr reconcile`, `lf pr checks` | `provider:github`, model provider for `ci-fix`, `process:git`, `process:gh` |
| **Linear events** — what Linear last said about each Project and issue | Repository- and tracker-scoped Project and issue events keep Linear's last state. Task and Wave readers use saved planning; ingestion keeps pending saves; the change sent last wins, so an unsent save rebases onto a tracker edit and still sends. Wave membership and sync events reference those shared facts; changes invalidate admission without rewriting execution history. | [Linear events](../../rust/loopflow-storage/src/sqlite/linear_events.rs) | `tracker_projects`, `tracker_items`, `tracker_wave_projects`, `tracker_wave_sync`, `tracker_events`, `project_binding_imports`, `project_transitions`, `project_transition_items`, `planning_scope_checks` (replaces `linear_team_checks`, leaving in LOO-1084); until LOO-960's contract, mirror triggers keep the released `pm_projects`, `pm_items`, `pm_wave_projects`, `pm_wave_sync`, `pm_issue_changes` equal for released `lf`, and that contract drops the unused `pm_project_name_cutover` | On-demand team fetches at planning/Graph/write boundaries and native turn-end hooks (`lf __context-block --moment stop`, answered by the Machine server's `session.boundary`); the server and running Graphs share a poll for locally owned open Tasks. Watchers also refresh while open. SQLite shares attempt freshness and the successful watermark; a Home-wide cooldown retains an exhausted hourly allowance's reset, and a per-operation cooldown the reset of a limit Linear keeps on one mutation | `lf wave sync`, `lf task status` | `provider:linear` |
| **Asana planning** — the other tracker | A repository names one team in `tracker: {asana: {team}}`. Asana keeps Tasks in the team project named after the repository and Wave Projects as team projects. It is the second tracker behind `Tracker`: it sends the same queued actions, answers in store words and retires actions it has no form for (Project edits, order, Initiatives, follow-up relations). | [Asana actions](../../rust/loopflow-execution/src/asana/actions.rs), [`Tracker`](../../rust/loopflow-execution/src/ops/tracker.rs) | Shares the tracker event tables and the outbound tracker queue | Same planning/Graph/write boundaries as Linear | `lf account connect asana` | `provider:asana` |
| **Shared plan** — the plan of a repository without Linear or Asana, the same on every Machine that clones it | Every Machine holds the whole plan with one stamp per field; the newest stamp is the value. Carriers move stamped records and decide nothing: each Machine's own Git ref on the remote, and a hub when one is named. An unsent record waits per carrier; applying one twice changes nothing. | [`PlanCarrier`](../../rust/loopflow-storage/src/sqlite/plan_carriers.rs), [Git exchange](../../rust/loopflow-execution/src/ops/plan_git.rs), [Hub exchange](../../rust/loopflow-execution/src/ops/plan_hub.rs) | `repository_plans` (this clone's repository id, and the ids other Machines know it by), `plan_stamps`, `plan_changes`, `plan_carriers`, `plan_refs` | The shared planning writer stamps each changed field; every arrival goes through its one import, newest stamp wins. | `lf machine hub`, `lf repo tracker`, `lf task sync`, ordinary saves | `refs/loopflow/plan/<machine id>` on the remote; the hub's server (`plan.changes`, `plan.apply`) over SSH |
| **Steer** — message guiding work | Immutable source/revision UUID; existing associations select Sessions | [`Steer`](../../rust/loopflow-execution/src/steer.rs), [`SessionSteerStatus`](../../rust/loopflow-storage/src/steer.rs) | `steers`; `steer_delivered` holds one claim-then-delivery row per (Session, steer), `task_update_readings` the last Task reading behind Task-update steers, and `steer_wake_reading` the Machine server's cursor over `steers.seq` for starting stopped Sessions; `steer_adapters` binds the callback capability to an AgentProcess; callbacks follow the Session's selected native conversation, which a provider start (including `/clear`) moves along with the resume reference. `task_comments` retains the separate planning thread; its tracker delivery is a `comment` row in the tracker queue | Foreground Task connections acquire comments independently of pending writes | `lf steer`, `lf task report`, Linear issue comments | SQLite |
| **Creation** — one submitted Task or new Session thread | Client-chosen request ID and content fingerprint deduplicate the exact submission on its original Machine. Unknown external effects remain unresolved; removed resources do not free the identity. | [`CreationReceipt`](../../rust/loopflow-storage/src/sqlite/creation.rs), [`engine::creation`](../../rust/loopflow-execution/src/engine/creation.rs) | `creation_receipts`; Desktop keeps only immutable submitted payloads until reconciliation is presented | Shared creation operation; Graph processes and held terminals carry execution | `lf task create --request-id`, `lf session new --request-id`, `lf server receipt`; served `task.create`, `session.new`, `creation.get` | — |
| **Command answer** — a served write's first result | A client request ID and operation fingerprint retrieve the first answer after a lost reply. Unknown outcomes remain unremembered. | [`CommandAnswer`](../../rust/loopflow-storage/src/sqlite/command_answers.rs) | `command_answers`, retained for one day | Machine server write worker | Served writes with `request_id` | — |
| **LfSession** — one conversation | Session row owns name, ancestry, readiness, completion and publication. Its current capture references an immutable history event written at reservation. Earlier events retain caller and Work attribution. Historical feedback remains readable; current Task reviews happen at interactive graph steps. | `SessionRecord`, `SessionId` | `agent_sessions`, `session_events`; `session_activity` holds attachment stream readings and terminal-reported status; `session_moves` holds a busy Session's pending move to another agent | Native turn observation retains start/usage/completion; Codex protocol replies record native identity, and the Machine server reads a Codex Session's turns from its app-server at each turn end; Claude and OpenCode retain the ID lf supplied; the Pi extension's `session.boundary` events to the Machine server retain Pi native identity, activity and cumulative usage; the extension's `/bin/sh` bash guard admits the existing watchdog into Pi-owned detached tool groups; Session operations own state | `lf session`, interactive `lf` | `process:cmux` projects the Task title onto its workspace and the attached Session name onto its tab (tmux, mapped under User, onto its window) without owning either |
| **Machine** — execution identity and destination | `MachineId` is identity; the SSH route is mutable. Personal Wave/repository Machine defaults place unowned Work and Sessions; they grant no process authority. Connections, pools and rentals select execution capacity. | [`Machine`](../../rust/loopflow-execution/src/durable.rs), [settings placement](../../rust/loopflow-execution/src/config/placement.rs) | `machines` (id, public name, person); personal configuration declarations | Foreground destination commands and the receiving CLI | `lf machine` (destination commands), `lf set machine`, `lf --machine` | `process:ssh` |
| **Installation / Promotion** — selected artifacts and their registration | Owns immutable artifact selection, isolated schema proof, app replacement, server registration/update and rollback. Install follows the recorded stream (`stable`, `--preview`, `--dev`) independently of caller Git state. Checkout updates belong to sync. | [`SwitchReceipt`](../../rust/loopflow-execution/src/installation.rs), [`published installation`](../../rust/loopflow/src/lf/commands/install/published.rs), [server registration](../../rust/loopflow-execution/src/installation/server.rs) | Installation selection and switch receipts; install schedule; OS server registration | The promotion command owns its OS-locked switch transaction; registration delegates to the OS supervisor | `lf self`, `lf self install`, `lf self install schedule`; hidden `lf self install server` client-readiness bridge | `process:launchctl`, `process:/bin/launchctl`, `process:crontab`, `process:systemctl`, `process:/usr/bin/open`, `process:/usr/bin/osascript`, `process:brew`, `process:/bin/sh`, `process:tar` (main packages via `gh`), `process:/usr/bin/hdiutil`, `process:/usr/bin/codesign`, `process:/usr/sbin/spctl` (the main package's signed app) |
| **Client API server** — access to Loopflow on one host | Owns admission, compatibility, framing and bounded delivery. Shared reads/commands own Work semantics; holders and Graph processes keep execution ([What runs on a Machine](index.md#what-runs-on-a-machine)). LOO-718 adds remote client admission. | [`server`](../../rust/loopflow/src/server/README.md) and Swift `ServerProtocol`; [ownership](server.md) | No shadow Work store; socket/incarnation are ephemeral. Remote access settings and client credentials belong to server admission in LOO-718. | OS-supervised selected CLI executable, independent of Desktop; holders and Graph processes outlive it | `lf server` | Private same-user Unix socket; authenticated remote endpoint is LOO-718's extension |
| **Held terminal** — one PTY a holder keeps for a viewed conversation or a Desktop shell | The holder owns the PTY, the agent child and a ring of raw output; it never reads the store or interprets Sessions. The server keeps the terminal emulator, relays views by PTY byte position and settles exits. A terminal id names one PTY and is never reused. | [`holder`](../../rust/loopflow-execution/src/terminal_holder.rs), [`transport::registry`](../../rust/loopflow-execution/src/transport/registry.rs) | `held_terminals` rows name the holder's build, exit code and viewing; `agent_sessions.held_terminal_id` links the Session; the exit receipt is a file in the terminal's directory | `lf holder run`, one per build, started by the server and outliving it and installs | `lf resume`, Desktop panes through `session.attach`; `lf holder`, which only the server runs | Private same-user Unix socket under a kernel lock |
| **Rental / Pool** — a machine rented for bounded time and money, or borrowed from an owned pool | The provider's sandbox is the fact; the rental row records admission (price, cap, deadline) and state, and the provider, or the guardian on the machine, deletes it at the deadline. An on-demand is a provider and hardware with a concurrency and hourly-price limit, and may be a Wave's or repository's Tasks capacity; a pool is owned machines lent one at a time. Giving back copies the machine's records first; `--force` skips that copy. Machines rented by hand stay rented. | [`Rental`](../../rust/loopflow-storage/src/rental.rs), [`OnDemand`](../../rust/loopflow-storage/src/rental.rs), [`Pool`](../../rust/loopflow-storage/src/rental.rs), [`Sandboxes`](../../rust/loopflow-execution/src/rental/sandbox.rs) | `pool_machines` retains an owned Machine's pool name, cost and expiry; `rentals` and `machine_records` retain execution facts; templates, availability, limits and `machine.capacity` live in personal configuration | The renting command owns admission, boot and give-back; the provider owns the deadline | `lf machine ondemand`, `lf machine pool`, `lf machine end`, `lf set machine.capacity <capacity>`, `lf task run --rent`, `lf task run --pool` | `process:uv` runs the Daytona, Modal, RunPod or AWS bridge; `process:ssh-keygen` forgets a reused tunnel's host key; then the Machine row's SSH edge |
| **Session history projections** — captured events and exact provider evidence | LfSession owns provider outcomes and each run's owner record its result; original payload confers no Graph authority. | `SessionCaptureSpec`, `SessionCaptureManifest`, `SessionHistory`, `ProviderHistory`, `SessionUsage` | Projects LfSession-owned input/history; Machine-local `runs/<prefix>/<run-id>/` immutable payload | shared conversation admission and history | `lf session history show`, `lf session history replay`, `lf session usage`, `lf session history`; Work/status history | `process:lf`, provider harnesses |
| **LfProcess** — one actual lf process | An opaque id (`LF_PROCESS_ID`, a Graph run or step id) that no table holds; the record the lf owns (a Graph run or step, a Task's carrier, a Session's attachment) carries its pid, OS start time and end. Liveness is one OS lookup against them (`RecordedProcess::evidence`). Agent provenance grants no control authority. A held terminal is not an LfProcess: its holder is recorded on each attachment it holds, and views hold separate revocable input tokens. | [`LfProcessId`](../../rust/loopflow-model/src/id.rs), [`AgentCaller`](../../rust/loopflow-storage/src/process.rs), [`RecordedProcess`](../../rust/loopflow-execution/src/process/os_process.rs), [`transport`](../../rust/loopflow-execution/src/transport.rs) | The owner's row: a Graph run or step (Graph), `task_graphs.run_pid` (Task), `agent_sessions.attached_*`; `processes` (`kind='lf'`) holds only what a released lf wrote, leaving with LOO-965; `agent_limit_recoveries` holds a headless step's usage-limit wait and the `-a` its restart keeps | Foreground commands, holders, Graph runs and other detached Work (`os_process::detached`) ([What runs on a Machine](index.md#what-runs-on-a-machine)); the Machine server's liveness pass ends owners whose process died | `lf graph list --runs`, `lf graph show`, `lf session list`, `lf resume`; ordinary parsed CLI commands | — |
| **AgentProcess** — the provider's OS process | One generation of a Session's agent (`agent_process_id`), recorded on the Session row: launcher, pid, OS start, end and exit. A fresh token fences each attachment. | [`AgentProcessId`](../../rust/loopflow-model/src/id.rs), [`AgentProcessState`](../../rust/loopflow-storage/src/process.rs), `SessionAttachment` | `agent_sessions.agent_*`; the bridge keeps an LfProcess-table row of `kind='agent'` for released readers until LOO-965 | Provider launch and observed settlement; typed admission remains open | `lf session list`, Task execution inventory | Provider OS process; `process:pi` owns its RPC or native conversation, tools and credentials |
| **Local process observation** — recorded processes joined to OS facts | An owner row's pid and OS start establish an observation, not new ownership: alive means the OS still has that pid with that start time. Failed OS sampling never proves absence. Duplicate or unknown identities grant no signal authority. Doctor's stray scan reports OS processes carrying `LF_STEER_SESSION` or `LF_PROCESS_ID` that no live row claims, and kills nothing. | [`OsProcess`](../../rust/loopflow-execution/src/process/os_process.rs), [`RecordedProcess`](../../rust/loopflow-execution/src/process/os_process.rs) | Owner rows (LfProcess, AgentProcess); no provider registry | Foreground observation and the server's recovery pass | `lf session list`, `lf graph list --runs`, `lf machine doctor` | `process:/bin/ps`, `process:ps`, `process:sysctl`, `process:lsof`, `process:kill`, `process:which`; `process:/bin/sleep` stands in for a live process in tests |
| **Disk cleanup** — reclaiming a Machine's disk across Home and every repository under `~/src` | Cleanup policy is a maintenance script, not lf product surface. Nothing under `~/.lf` goes: the database, its pre-migration clones, captures, logs and account homes stay, and anything a process holds open never goes. Worktree, build-output and branch judgment stays with `lf wt prune`; the script judges idle checkouts that judgment keeps and removes only ignored build output untouched for two hours in checkouts no process holds, and the repository skill only reports. An unreadable process view stops removal. | [`disk-cleanup.sh`](../../scripts/disk-cleanup.sh), [`disk-cleanup` skill](../../.lf/skills/disk-cleanup.md) | Filesystem only; removals append to `~/.lf/logs/disk.log` | Hourly crontab entry running the skill from the main checkout | `scripts/disk-cleanup.sh install`; `lf machine prune` for lf's own Process records and history | Open-file inspection (`lsof`); `lf wt prune` |
| **Provider authentication** — native login and service credentials on one Machine | Claude and Codex own native authentication; saved subscription accounts never select launch credentials. Service connect/disconnect and historical account attribution remain. | [Native authentication](../subscriptions.md), [`provider_auth`](../../rust/loopflow-execution/src/provider_auth/mod.rs) | `provider_accounts`, `provider_account_limits`, `provider_account_switches`, `provider_session_accounts` remain historical; `access_profiles`, `provider_tokens` also support service auth | Native clients own subscription login and refresh; lf owns service authentication | `claude`, `codex login`, `lf account` | `provider:claude`, `provider:codex`, `provider:doppler`, `provider:opencodezen`; native homes and service flows through `process:claude`, `process:codex`, `process:doppler`, `process:opencode`, `process:security`, `process:secret-tool` |
| **Leaving the store** — tables nothing reads or writes | Nothing reads or writes them; released `lf` may still query some until the contract migration drops them (`-- leaving:` marks in `unused_store_members.sql` and `plan_hub.sql`). | [Store schema](../../rust/loopflow-storage/SCHEMA.md) | `ondemands`, `pools`, `session_steers`, `steer_batches`, `steer_deliveries`, `task_deletions`, `task_issue_identities`, `task_pr_repair_incidents`, `tool_responses`, `work_pane_layouts`, `git_tracker_changes`, `git_tracker_conflicts`, `git_tracker_destinations`, `git_tracker_imports`, `git_tracker_routes`, `git_tracker_seen`, `planning_destinations`, `planning_peer_changes`, `planning_peer_conflicts`, `planning_peer_imports`, `planning_peer_observed`, `planning_peer_routes`, `planning_user` | None | None | None |
| **Conversation context** — refreshed source block and authoring targets | Fixed additions preserve native instructions; skill/request/clipboard form the first turn. Saved source identities reread checkout files at startup and compaction, within 10,000 rendered UTF-8 bytes. Configured budgets measure authored memory/scratch, never gate launch. | [`ContextDelivery`](../../rust/loopflow-execution/src/context_block.rs), [`ContextBudgets`](../../rust/loopflow-execution/src/prompt/context_budget.rs), [`ContextBudgetReport`](../../rust/loopflow-execution/src/prompt/context_budget.rs) | Checkout sources and complete private references under `.lf/prompts/`; no measurement store | `lf __context-block` serves provider startup/compaction hooks from checkout files, with steers each hook claims from SQLite; foreground size reporting. Missing optional capture payload does not erase native resume identity; malformed retained payload still fails | `lf context` | — |
| **Code-size measurement** — repository blobs measured in model tokens | Git blob identity owns content; token counts are deterministic memoized measurements, not Run usage. | [`CodeNode`](../../rust/loopflow/src/lf/commands/tokens.rs), [`CodeSnapshot`](../../rust/loopflow/src/lf/commands/tokens.rs) | `blob_tokens` | Foreground command only | `lf repo tokens` | — |
| **Task owner identity** — the GitHub user behind a Task owner's Git email | Claims carry the owner's email; GitHub's commit authorship resolves it to a user once, and the answer is cached. A miss is retried after a day; an unreachable or unauthenticated `gh` caches nothing. | [`RepoPerson`](../../rust/loopflow-execution/src/ops/people.rs), [`GithubPerson`](../../rust/loopflow-execution/src/ops/people.rs) | `github_people` | Foreground command only | `lf repo people` | GitHub REST through `gh` |
| **Store change revision** — which displayed domain a commit changed | Derived inside the writer's transaction by schema triggers, never authored and never bumped at a call site. Transcript lines move nothing; usage moves only its own revision, which no subscription reads again for; Wave detail shows token totals as of its last reading. Each `--subscribe` reading pins revisions and database-derived joins in one read transaction; nested readers use savepoints. Events only say when to compare revisions. A confirmation request makes a subscription answer with a reading begun after it. OS/Git/file observations are not transactional facts. | [`StoreRevisions`](../../rust/loopflow-storage/src/sqlite/revisions.rs), [`SubscriptionFrame`](../../rust/loopflow-execution/src/read_stream.rs) | `store_revisions` | Store transaction | `--subscribe` on the ordinary reads | — |
| **Schema frontier** — ordered definition of durable control storage | Migration bytes on origin/main are immutable; `build.rs` orders slugs by origin/main's first-parent history. Release preparation records that order in `rust/loopflow-storage/migration-order.txt` for source packages without Git history. Any build opens a store whose history extends its own; destructive change is staged, so older lf keeps running against the superset. | [`Migration`](../../rust/loopflow-storage/src/migration_catalog.rs), [`MigrationId`](../../rust/loopflow-storage/src/migration_catalog.rs) | `schema_migrations`; migration files | Private-Home opens migrate themselves; only the install switch advances the shared store, with merged migrations | `lf self install` | Build-time merge order from Git or the packaged snapshot; no runtime edge |
| **Task ownership** — (person, Machine) or unowned | An append-only log; the later entry wins in tracker order. Exactly one writer appends, always one of the caller's own Machines: their Machine holding the claim, else their Wave Machine; nothing routes to another person's Machine. Entries travel as marked Task comments. An inbound entry never ends this Machine's own open claim. | [`TaskOwnership`](../../rust/loopflow-model/src/durable.rs), [`TaskOwnershipEntry`](../../rust/loopflow-model/src/durable.rs) | `task_ownership` (append-only by trigger; each entry names its writing Machine), `people` | The writer's IMMEDIATE transaction, reached over `lf --machine` | `lf task claim`, `lf task move <task> --machine <machine>`, `lf task unassign`, `lf task run`, `lf task checkout` | Linear assignee; SSH |
| **Portfolio** — repository selection on one Machine | Rust owns folder discovery and manual membership; Desktop reads and changes the same selection. Removing membership preserves files and retained Work. | [`Portfolio`](../../rust/loopflow-execution/src/portfolio.rs), `PortfolioMode`, `PortfolioRepo` | Machine-local config.yaml; locked atomic writes | Foreground CLI; Portfolio controls call its API | `lf portfolio`, `lf repo` | — |
| **Desktop workspace** — retained presentation of repository, Wave or Task | Work identity selects one repository window and retained workspaces through the sidebar. Pane contents own Machine locations; exact window/content/surface lifetimes fence mutations, which all run through the Swift `DesktopAction` registry. Reads acquire no client and receipts promise no readiness. | `WorkWorkspaceKey`, `DesktopInspection`, `DesktopPaneTarget`, `DesktopActionRequest`, `DesktopActionReceipt` | `dashboards`, `dashboard_arrangements`, `task_orders` for saved Dashboards and Task order; each Mac's pane layouts under `$LF_HOME/desktop-cache/pane-layouts/`; window-owned registry, multiplexer and native surfaces | Loopflow.app on macOS | `lf desktop` | `$LF_HOME/run/desktop/<bundle-id>.sock` |
| **Desktop shell** — retained execution behind a Desktop shell tab | Desktop creates once, reattaches, closes and keeps ended history; the shell is plain (no hooks or steers) and its viewers add no checkout hold. Close and Task cleanup end it through the server. Task retirement requests Desktop settlement before Git deletion. Rex owns its ordinary shells. | [`desktop::shells`](../../rust/loopflow-execution/src/desktop/shells.rs), [held terminal](../../rust/loopflow-execution/src/transport/README.md#task-shells-and-completed-workspaces) | `desktop_shells` receipts name the held terminal (`terminal_id`) and the launcher LfProcess | Destination-side Desktop runtime, including headless Linux; the destination's server and holder, and the SSH view stream | Desktop New Shell, Reconnect, Close and history-only Undo | — |

<!-- architecture-map:end -->

The public API column covers top-level command families, not every subcommand or
Rust function. [`lf` reference](../lf-reference.md) owns argument-level detail. DTOs emitted
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
| [Linear events](../../rust/loopflow-storage/src/sqlite/linear_events.rs) / `tracker_projects`, `tracker_items`, `tracker_wave_projects`, `tracker_wave_sync`, `tracker_events` | Linear planning | Shared entity ingestion orders tracker revisions and retains last-good facts when refresh fails. Canonical Task and Wave views read saved Work with its separate tracker state; ownership checks read retained claims directly. List and detail events store concrete Linear fields in one transaction; there is no public snapshot envelope. Event age and invalidation remain explicit. |
| [`TaskTrackerCursor`](../../rust/loopflow-storage/src/work/task/mod.rs) / `task_linear_observations` | Linear Issue title, description and comment revisions | Reconciliation saves comments and appends deduplicated direction. Team refresh also stores issue state and Project membership through the typed Linear owner; the saved comment projection grants no graph or Process authority. |
| [`GithubObservation`](../../rust/loopflow-storage/src/work/task/mod.rs) / `task_prs`, `ci_incidents` | GitHub PR/check state | Webhook or foreground reads update Task delivery evidence; GitHub remains merge truth. |
| `tests/fixtures/dto/` | Rust `lf --json` DTOs | Rust and Swift fixture tests reject required-field or enum drift. |
| `rust/loopflow-storage/tests/fixtures/migrations/` | Released schemas that upgrade tests seed from | Rust build/runtime tests and `check_migrations.py` reject ordering, body-byte and checksum drift. |
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
| Provider support | Add a provider adapter and normalized stream mapping | Provider credentials/finality remain provider-authored |
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
| `shim:retired-app-replacement` | Promotion removes the previously shipped app bundle after the current app commits. | [`AppPromotion`](../../rust/loopflow/src/lf/commands/install.rs); remove after the retired bundle name is outside supported installs. |
<!-- architecture-shims:end -->

## Appendix: historical-only vocabulary

The scanner matches exact phrases, not overloaded words. Provider resume
sessions are current. The authored chat
reference `project:<slug>` is also current; it is not the old Linear-label PM
model.

<!-- architecture-vocabulary:start -->
| Retired term | Allowed scopes | Current language |
| --- | --- | --- |
| `Project Session`, `project_sessions`, `task_sessions` | `rust/loopflow-storage/src/migrations/`, `rust/loopflow-storage/src/migration_runner.rs`, `rust/loopflow-storage/src/tests/fixtures/`, `release/` | Stable Project/Task **Work**; LfSession owns the conversation and native history; a Graph is a Graph process and its step processes. |
| `session context`, `LF_SESSION` | — | Typed Work ancestry and Process/LfSession provenance; transitional launch environment names are listed in cutover status. |
| `lf radio`, `agent bus` | `release/` | Typed Work observations, Steer, synchronous questions, and review GraphSteps. |
| `pm.linear_project`, `projects/<slug>.md` | `release/` | `linear.initiative`; Linear Initiative → Project → Issue. |
| `machine-local host`, `machine-global command`, `machine-global mutation`, `machine-global reservation` | — | Machine-local keeper, command, mutation, or reservation. |
<!-- architecture-vocabulary:end -->

The following map gives the destination of retired or transitional representations.
Cutover status identifies which runtime consumers remain; the map does not claim
their deletion is finished.

| Historical or transitional representation | Target owner |
| --- | --- |
| Wave-scoped `wave_chapters` rotation | Linear Project status; a Wave's Project is replaced in place, with no rotation |
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
  conflicts win while losing intentions remain history. A Wave's planning is one
  In Progress Linear Project; uncertain delivery stays visible.
- Wave instruments and observations survive plan changes. Missing target
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

The bounded check builds the live schema from every migration file, discovers
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
