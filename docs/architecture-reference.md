---
layout: default
title: Architecture Reference
---

# Architecture Reference

This is the checked inventory behind the developer architecture guide. It is
organized for lookup and drift detection, not as a reading path. Start with
[Architecture](architecture.md), then enter the area that owns the code you
are changing.

The smallest successful launch and its source-owner trace live in
[Architecture](architecture.md) and [Execution](architecture/execution.md).
This page starts where tutorial prose ends: the complete inventory and the
contracts checked against source.

## The system grows outward from a direct Skill launch

The direct Skill launch is the kernel of Loopflow. It remains useful with
no Wave, Project, Task, or daemon. It requires its Home's writable Run store. Higher layers
supply composition, durable context, delivery, placement, and views around its
discovery, prompt, provider, harness, and evidence components. Their
boundary executors remain domain-specific because their settlement rules differ.

```text
                        +--------------------------+
                        | UI and read projections  |
                        | status / roadmap / app   |
                        +-------------+------------+
                                      |
                        +-------------v------------+
                        | multi-Home placement     |
                        | HomeId / lfd / lf ssh    |
                        +-------------+------------+
                                      |
                        +-------------v------------+
                        | Task delivery            |
                        | worktree / PR / CI       |
                        +-------------+------------+
                                      |
                        +-------------v------------+
                        | bounded Task advancement|
                        | exact invocation claim    |
                        +-------------+------------+
                                      |
                        +-------------v------------+
                        | tracked Work + Flows     |
                        | durable facts / inputs   |
                        +-------------+------------+
                                      |
                        +-------------v------------+
                        | one Skill run            |
                        | discover -> prompt       |
                        | -> route -> spawn        |
                        | -> record -> settle      |
                        +--------------------------+
```

- **Skill runner:** execute one reusable instruction set through one provider
  harness and leave local evidence.
- **Flow composition:** sequence Skill and mechanical Command nodes, route Xor
  branches, follow backward edges, and persist exact human boundaries.
- **Durable planning:** preserve Wave, Project, and Task intent across crashes;
  Task execution adds a versioned worker claim to its saved Flow invocation.
- **Task delivery:** attach one active remote branch, worktree, and PR to concrete Work.
- **Multi-Home placement:** run the same commands on a selected machine through
  `lfd` or explicit `lf ssh`.
- **Surfaces:** derive CLI and Mac views from planning facts, provider truth,
  local process observation, and Run records.

The complete system has four kinds of state:

```text
                         external truth
                  Linear       GitHub       providers
                     ^            ^              ^
                     |            |              |
user / automation -> lf -------- domain APIs ----+
                     |
          +----------+-----------+
          |                      |
          v                      v
 tracked Work + delivery    execution evidence
 Chapter x Wave -> Project -> Task        Run row + evidence files
       stable Work           manifest + JSONL + terminal
          |
          v
 repository + Git worktrees

exact local races use OS locks; remote execution uses lf ssh
```

- **Tracked Work** records purpose, input, convergence, and the exact next Flow
  boundary; advancement claims decide which Run may move it.
- **Execution evidence** records what one provider launch did.
- **Delivery** coordinates worktrees, commits, PRs, CI, and merge.
- **Machine authority** places Work and scopes local credentials, processes,
  files, and locks to a Home.

An identifier in one area is not authority in another. The detailed boundaries
below are the architecture's central constraint.

## Planning and execution vocabulary

```text
Repository --< Chapter                         one current for the repository
     |                                             |
     `--< Wave ------------------------------< Project  unique (Wave, Chapter)
                                                 |-- KRs, metric targets, Flow
                                                 `--< Task
                                                       |-- worktree and serial PRs
                                                       `--< Flow invocation
                                                              `--< Run -- Session?

Run.invocation_id => Run.task_id => Run.wave_id      nullable, filled upward
Flow invocation.parent_id                          runtime nesting only
```

The Run arrows above describe execution ownership. Independent Runs can belong
to a Task, only a Wave, or neither. Session has one Run and no separate Work parent.
Project is a plan within a Chapter, not a second name for Chapter. Work remains
`Wave | Project | Task`; Run parentage grants no Work or process-control authority.

| Concept | Stable identity | Owns |
| --- | --- | --- |
| Repository | Canonical repository identity | Chapter clock and Wave membership |
| Chapter | Repository-scoped Chapter ID | One synchronized planning boundary and its history |
| Wave | `WaveId` | Objective, memory, cadence, budget, chat, metric instruments |
| Project | `ProjectId` | One Wave's Tasks, KRs, metric targets, and Flow in one Chapter |
| Task | `TaskId`, linked to a Linear Issue | Worktree, serial PRs, Flow invocations |
| Flow | Template name and source | Authored Skill/Command/Xor/human nodes and backward edges |
| Flow invocation | Invocation ID | Captured expanded graph, cursor, return counts, runtime children |
| Run | `RunId` | One launch, current attribution, execution membership, provider and outcome |
| Session | Its `run_id` | One conversation's kind, name, readiness and completion |
| Home | `HomeId` | Store, artifacts, credentials, local process authority |
| Placement | `(WorkRef, HomeId)` | Where Work executes, not ownership of an observed process |
| Steer | Work event identity | Ordered authored correction |

## Core models and APIs

All constructors and mutation APIs validate before committing. A decoded struct
is input to validation, never a way around it. Read DTOs carry typed identities;
CLI names and issue identifiers resolve once at the boundary.

| Row | Authoritative fields and operations |
| --- | --- |
| `chapters` | Repository identity, Chapter ID, current status, dated boundary and transition state; rotate the repository, inspect history |
| `waves` | Stable identity and repository locator; authored goal/memory/instrument definitions stay in repository files |
| `projects` | `wave_id`, `chapter_id`, Flow template selection, provider Project identity; read/update the Linear-backed plan |
| `tasks` | `project_id`, provider Issue identity, Work state, worktree and delivery facts; derive Wave through Project |
| `flow_invocations` | Task, nullable runtime parent, captured Flow name/source and complete graph, local node cursor, loop return counts, status, claim/version/generation, pending boundary; create, claim, settle, interrupt, restart |
| `runs` | ID, causal parent Run, Home/repository/cwd, provider/model/skill, timestamps/outcome, nullable `invocation_id`, `task_id`, `wave_id`, `work_source`, node and iteration tuple; create, settle, bind, query |
| `sessions` | `run_id` unique FK and identity; `kind` = `interactive | flow_review | ask`; `title`, `title_source` = `generated | human`; `state` = `waiting | active | ready | closed`; `ready_summary`; open, ready, complete, rename |

A Session reads ancestry, cwd, provider, and Flow membership through its Run.
Session rows do not copy these fields. Headless Runs may have an optional name;
a conversational Run's displayed name is its Session title, not a synchronized
second title. Native provider identity and attachment receipts are Run-owned
operational data in SQLite; the provider still owns its transcript. Ask request,
caller, selected Skill, keyed retry identity and completed result are Session
kind-specific fields. There is no separate Ask file or Session registry.

### Three validator groups

| Validator | Enforced contract | Mutation boundary |
| --- | --- | --- |
| Planning ancestry | Project's Wave and Chapter name the same repository; `(wave_id, chapter_id)` is unique; a repository has one current Chapter; Task's Wave is derived from its Project | Plan writes and atomic chapter activation |
| Invocation structure | Invocation belongs to a Task; parent has the same Task; parent chain is acyclic and describes runtime entry; node exists in the captured expanded graph; one current root per Task | Invocation creation, child entry, cursor settlement, restart |
| Run ancestry | Invocation fills Task; Task fills Wave; supplied parents must match. A bound Run has a valid node/iteration tuple; an independent Run has neither. Membership never changes through bind | All Run creation, binding, import and ancestry-changing writes |

SQLite foreign keys, uniqueness and nullability constraints back these APIs.
Cross-row checks run in the same write transaction as the change. Task moves
within a Wave leave Run ancestry unchanged; a cross-Wave move validates and
updates dependent Run Wave values atomically. It cannot silently invalidate
historical execution. Current attribution and immutable launch evidence are
different facts.

`work_source` is nullable for no attribution, otherwise `declared`, `checkout`,
`inherited`, or `bound`. Explicit launch selectors win over checkout inference.
An Ask copies its caller's Task/Wave with `inherited`; it does not inherit the
caller's invocation just because it shares a checkout. Only a Flow driver sets
execution membership on Runs it launches.

Bind sets Task and its Wave, only Wave, or neither, with `bound` provenance for
a selected parent. Binding to a done or landed Task is allowed and does not
reopen it. An invocation-owned Run cannot be rebound to a different Task or
have its Task cleared: the operation reports the invariant and changes nothing.
Binding never rewrites the invocation, node, iteration tuple, launch artifact,
provider history, Session identity or name.

A Task's displayed `started` fact is an indexed existence query over its Runs.
Chapter retirement also examines authored work, PRs and active invocation
claims: absence of a Run alone never proves untouched backlog. Binding away the
last Run can change the displayed fact without erasing delivery history.

The shared readers select Session rows joined to Runs, and Runs by their typed
parents. `runs --task`, `session list --task`, usage, activity and the desktop
agree. No read calls the launch resolver or requires an active PR. A Session
with null Task is an orphan in the workspace, including a Wave-only Session;
a Task-bound Session missing from a visible roadmap remains bound and reachable.
Swift caches the workspace projection by its input readings and uses IDs for
forward and reverse lookup.

Adjacent APIs keep their own authority: Task PR operations own Git/GitHub;
provider routing owns credentials; Home placement owns routing; exact process
receipts plus OS evidence own signaling. None is derived from these FKs.

## Code territory and rough size

These are physical lines in the current branch, rounded to the nearest hundred.
They include inline tests and comments; migration SQL and external test trees
are listed separately. The counts are navigation aids, not quality metrics.

| Territory | Main paths | Approx. LOC | What lives there |
| --- | --- | ---: | --- |
| CLI and presentation | `rust/loopflow/src/lf/`, `src/bin/` | 31,700 | Clap grammar, command dispatch, status/read models, terminal output |
| Operational workflows | `rust/loopflow/src/ops/` | 25,800 | Task/chapter control, sessions, PR, Git, release, metrics, PM operations |
| Prompt and process engine | `rust/loopflow/src/engine/`, `src/harness/` | 29,300 | Skill/Flow discovery, prompt assembly, provider subprocesses and streams |
| Tracked Work | `work/`, `pm/` | — | Wave/Task facts, Task delivery identity, planning/provider models |
| Boundary execution | `controller/` | — | optional Wave service and claimed Task Flow boundaries |
| Storage and command journal | `store/`, `journal/` | 19,700 | SQLite access, migrations engine, durable rows, outer command receipts |
| Provider authority | `provider_auth/`, `provider_account/` | 7,500 | Login, encrypted tokens, account homes, routes and leases |
| Home daemon | `lfd/` | 2,900 | Home HTTP API, webhooks, Wave/service reconciliation |
| Shared root modules | top-level `src/*.rs` | 10,400 | Run records, artifact switching, repository identity, subscriptions |
| Released and draft SQL | `store/migrations/**/*.sql` | 4,900 | Immutable schema history and current draft frontier |
| Swift app production | `swift/Loopflow/`, `swift/LoopflowMac/` | 18,200 | Shared DTOs/services and macOS UI |
| External Rust/Python/Swift tests | `rust/loopflow/tests/`, `python/tests/`, `swift/LoopflowTests/` | 27,100 | Cross-module, wire, migration, CLI, and app proofs |

## Complete ownership map

The following inventory is both documentation and a checked ownership index.
Every top-level CLI family, live SQLite table, process entrypoint, HTTP route,
provider, and literal subprocess edge must appear exactly once.

<!-- architecture-map:start -->
| Concept | Truth and authority | Data structure | Persistence | Process owner | Public surface | External edge |
| --- | --- | --- | --- | --- | --- | --- |
| **User** — a person or external harness originating work | User-attributed actions author root input and decide effects that require user intervention. User is actor provenance, not a control credential. | [`Author`](../rust/loopflow/src/durable.rs) | Git supplies `user.name` unless personal Loopflow config overrides it; input records retain source author names. No User row; authored effects persist on the concept they change. | `lf` | `lf :`, `lf desktop`, `lf user` | `exec:open`, `exec:osascript`, `exec:pbpaste`, `exec:id` |
| **Skill** — one reusable prompt with assembled context | Repository/builtin Skill Markdown is authoritative; discovery selects one source. | [`Skill`](../rust/loopflow/src/engine/flow.rs), [`SkillSource`](../rust/loopflow/src/lf/discovery.rs) | `.lf/skills/`, builtin Skill files, installed vendor Skill directories | `lf-prompt` | `lf skill`, `lf sync-skills`, `lf list`, `lf help` (local command/definition discovery) | `exec:python3` |
| **Flow / Flow invocation** — template and one execution | Template expansion captures all Skill content and Xor paths; invocation transactions own cursor and claim settlement. | `Flow`, `FlowInvocation`, typed node ID | `.lf/flows/`; `flow_invocations` | CLI driver or hidden `lf task __worker` | `lf flow`, `lf run` (flow-first definition execution), `lf task run --flow` | — |
| **Wave** — durable operating context with goal, memory, cadence, chat, and project selection | The Wave UUID is durable identity; canonical repository plus normalized slug is its mutable readable locator. `wave/<name>/GOAL.md` and `MEMORY.md` own repository intent; the Linear Initiative owns shared planning membership. | [`Wave`](../rust/loopflow/src/work/wave/mod.rs), [`WaveLocator`](../rust/loopflow/src/work/wave/mod.rs), [`CanonicalRepo`](../rust/loopflow/src/repository.rs), [`WaveConfig`](../rust/loopflow/src/work/wave/config.rs) | `waves`, `observation_outbox` (deferred Wave observations); `wave/<name>/`; `.lf/journal/waves/<name>/journal.jsonl`; an in-flight relocation receipt under `.lf/tmp/wave-relocations/` | `lf __chat-connect` opens chat through the Home daemon; `lf __resident` behind the Wave listener; listener and relocation share the repository locator lock | `lf wave`, `lf chat`, `lf reply`, `lf wave list`, `lf wave status`, `lf roadmap`, `lf cron`, `lf wave relocate`; `wave GET /health`, `wave GET /channel`, `wave GET /conversation`, `wave GET /events`, `wave POST /messages`, `wave POST /observations`, `wave POST /stop`, `wave POST /resident/attach`, `wave POST /resident/deltas`, `wave GET /resident/context` | Discord when configured |
| **Chapter / Project** — repository clock and one Wave plan per Chapter | One repository-wide activation selects every Wave's Project; Linear owns shared plan content. | `Chapter`, `Project` | `chapters`, `projects`, `project_events`; Linear Project content | deterministic resumable rotation | `lf wave new-chapter`, `lf wave history` | Linear |
| **Live metric** — one reviewed measurement contract owned by exactly one Wave, plus revision-bound current evidence | `wave/<name>/metrics/*.md` owns meaning and Wave ownership; an accepted instrument observation owns its source-time fact; [`MetricPortfolioDto`](../rust/loopflow/src/controller/wave/metrics.rs) is the sole derived reading shared across surfaces. Metrics inform KRs but never complete them. | [`MetricContract`](../rust/loopflow/src/controller/wave/metrics.rs), [`MetricObservation`](../rust/loopflow/src/controller/wave/metrics.rs), [`MetricPortfolioDto`](../rust/loopflow/src/controller/wave/metrics.rs) | `wave/<name>/metrics/`, `metric_instruments`, `metric_observations` | Metric instruments write observations; foreground and resident Rust readers derive bounded portfolios. | Status/roadmap JSON, Wave and Task prompts, the shared Swift DTO, and Mac Wave detail expose the same `metric_portfolio`. | — |
| **Task** — concrete work inside exactly one Project | The Linear Issue owns directive/status. The one active remote branch is current checkout identity: a checkout tracking it identifies the Task, while the stored worktree path is placement. One Task worker at a time owns advancement of the selected Flow; helpers and delivery commands may mutate the worktree without that claim. Git owns commits/branch state; GitHub owns PR/check/merge truth. | [`Task`](../rust/loopflow/src/work/task/mod.rs), [`TaskPr`](../rust/loopflow/src/work/task/mod.rs) | `tasks`, `task_events`, `task_prs`, `task_pr_repair_incidents`, `task_linear_observations`, `task_linear_ingested_comments`; Linear Issue; Git worktree | hidden `lf task __worker` drives successive claimed boundaries until a stop; foreground operations record delivery evidence | `lf task`, `lf pr`, `lf wt`, `lf rebase`, `lf commit` | Linear |
| **PR landing** — one watched attempt to merge an exact PR head | GitHub is authoritative for the PR head, required checks, and merge. One landing generation owns one supervisor, which repairs current failures until resolved; CI incidents record evidence rather than admit execution. | [`PrLanding`](../rust/loopflow/src/pr_landing.rs), [`CiIncident`](../rust/loopflow/src/work/task/mod.rs) | `pr_landings`, `ci_incidents` | Healthy Home daemon when it claims the generation; otherwise the invoking `lf pr land` process | `lf pr arm`, `lf pr land`, `lf ci`; `lfd POST /landings/claim` | `provider:github`, model provider for `ci-fix`, `exec:git`, `exec:gh` |
| **PM projection** — locally readable current planning snapshot | Linear remains authoritative; the Wave UUID keys the projection so locator changes preserve it. Sync atomically replaces the projection and reads never author through it. Confirmed native deletions suppress stale items without rewriting historical workflow outcomes. | [`PmSnapshotRow`](../rust/loopflow/src/store/mod.rs), [`PmWave`](../rust/loopflow/src/pm/mod.rs) | `pm_snapshots` | Foreground PM sync or Home webhook reconciliation | `lf repo`, `lf wave sync` | `provider:linear` |
| **Steer** — correction to Task advancement | Linear comment id/revision; Task identity selects its advancing worker | [`Steer`](../rust/loopflow/src/durable.rs), [`TaskEventKind`](../rust/loopflow/src/work/task/mod.rs) | Linear Task comments; local Task events cache delivery | Task worker refreshes comments and attempts live input; successor workers refresh their seed | `lf task comment`, Linear issue comments | Linear |
| **Tool response** — one idempotent response to a Work-scoped tool request | Stable Work identity plus request id names the response slot; a second, different answer is rejected. | [`ToolResponseWrite`](../rust/loopflow/src/durable.rs), [`ToolResponseReceipt`](../rust/loopflow/src/durable.rs) | `tool_responses` | Store transaction | Internal Work store API | — |
| **Session** — one conversation on one Run | Session row owns name, readiness and completion. Run supplies ancestry and provider identity. Complete returns saved feedback; only the following deciding Run chooses navigation. | `SessionRecord`, `RunId` | `sessions` | `lf __provider-session` records native identity; Session operations own state | `lf session`, `lf ask`, interactive `lf` | — |
| **Home / Placement / Promotion** — stable machine identity, Work placement, and artifact selection | `HomeId` is identity; SSH route is mutable. Placement is planning state and never process ownership. Promotion owns immutable artifact selection, isolated schema proof, service replacement, and rollback only. Install selects the latest published release independently of caller Git state; the laptop schedule invokes that same command. Checkout updates belong to rebase. | [`Home`](../rust/loopflow/src/durable.rs), [`Placement`](../rust/loopflow/src/durable.rs), [`SwitchReceipt`](../rust/loopflow/src/machine_install.rs), [`published installation`](../rust/loopflow/src/lf/commands/install/published.rs) | `homes`, `work_placements`; Home-local SQLite; machine install selection and switch receipts; laptop refresh LaunchAgent | `lfd` starts eligible Wave listeners; the promotion command owns only its OS-locked switch transaction | `lf home`, `lf work`, `lf ssh`, `lf install`, `lf install schedule`; `lfd GET /health`, `lfd GET /status`, `lfd POST /waves/start`, `lfd POST /waves/stop`, `lfd POST /waves/reconcile`, `lfd POST /linear/webhook`, `lfd POST /github/webhook` | `exec:ssh`, `exec:launchctl`, `exec:systemctl`, `exec:/usr/bin/open`, `exec:/usr/bin/osascript`, `exec:brew`, `exec:/bin/sh`, `exec:tmux` |
| **Run** — one launch and current attribution | Run row owns identity, nullable parents, provider and lifecycle; immutable artifacts own original launch inputs and recorded provider evidence. Neither grants control authority. | `Run`, `RunSpec`, `RunManifest`, `RunSnapshot`, `RunUsage` | `runs`; Home-local `runs/<prefix>/<run-id>/` evidence artifacts | shared harness capture and settlement path | `lf runs`, `lf replay`, `lf usage`, `lf activity`; Work/status Run evidence | `exec:lf`, provider harnesses |
| **Browser capture** — one isolated, bounded screenshot transaction | The requested source, viewport, and output name the transaction; only a validated PNG replaces the output. The standalone shell identity and fresh process group keep capture separate from the user's browser and bound to its owner. | [`ScreenshotArgs`](../rust/loopflow/src/lf/mod.rs), [`ProcessGroupGuard`](../rust/loopflow/src/engine/process.rs) | Output PNG only; no control-store state | `lf __screenshot-supervisor` owns one `chrome-headless-shell` process group and observes the public command through a control pipe | `lf screenshot` | `exec:chrome-headless-shell` |
| **Local process observation** — outer command receipts joined to current OS facts | A live kernel process plus a matching local receipt is observation, not durable ownership. Registered orphan OpenCode groups may be reaped; unclaimed provider PIDs may not. | [`ActivitySnapshot`](../rust/loopflow/src/lf/commands/top.rs), [`ProcessPruneReport`](../rust/loopflow/src/lf/commands/top.rs) | `run_events`; Home-local Exec receipts and OpenCode server registry | The foreground observer samples the process table; no keeper asserts Run liveness | `lf ps`, `lf top`, `lf prune`, `lf doctor` | `exec:/bin/ps`, `exec:ps`, `exec:lsof`, `exec:kill`, `exec:which` |
| **Provider account / route** — credential authority and ordered provider selection on one Home | Provider token/account rows and Access Profiles own routing; credentials stay in provider homes, encrypted storage, Doppler, or forwarded foreground leases. | [`Provider`](../rust/loopflow/src/provider_auth/mod.rs), [`AccessProfile`](../rust/loopflow/src/profile.rs), [`ProviderRoute`](../rust/loopflow/src/profile.rs), [`ProviderAccount`](../rust/loopflow/src/store/mod.rs) | `access_profiles`, `account_access_profiles`, `provider_accounts`, `provider_account_limits`, `provider_routes`, `provider_session_accounts`, `provider_tokens`, `provider_deliveries` | The foreground auth command owns provider login process groups and passive browser handoff; durable processes use credentials installed on their Home | `lf auth`, `lf profile`, `lf route` | `provider:claude`, `provider:codex`, `provider:doppler`, `provider:opencodezen`, `exec:claude`, `exec:codex`, `exec:doppler`, `exec:opencode`, `exec:security`, `exec:secret-tool` |
| **Code-size measurement** — repository blobs measured in model tokens | Git blob identity owns content; token counts are deterministic memoized measurements, not Run usage. | [`CodeNode`](../rust/loopflow/src/lf/commands/tokens.rs), [`CodeSnapshot`](../rust/loopflow/src/lf/commands/tokens.rs) | `blob_tokens` | Foreground command only | `lf tokens` | — |
| **Schema frontier** — ordered definition of durable control storage | Released migration bytes are immutable authority; drafts join only through deterministic release materialization. | [`Migration`](../rust/loopflow/src/store/migration_catalog.rs), [`MigrationId`](../rust/loopflow/src/store/migration_catalog.rs) | `schema_migrations`; canonical and draft migration files | Store open validates/applies; release cut publishes | `lf release` | `exec:sh` (release hooks) |
<!-- architecture-map:end -->

The public API column covers top-level command families, not every subcommand or
Rust function. [`lf` reference](lf.md) owns argument-level detail. DTOs emitted
by `--json` are required-field projections; Rust/Swift fixture tests own their
wire parity.

## Persistence map

Loopflow deliberately uses several stores because no one store owns all truth.

```text
repository files + Git        authored goals, memory, Skills, Flows, code
Home SQLite                  planning, delivery, Run, Session and invocation rows
Wave journal JSONL           conversation and resident event history
Run record files             one Home's provider-launch evidence
provider-native homes        model credentials and resumable sessions
Linear / GitHub              shared planning and delivery truth
machine install directory    immutable binaries and switch receipts
kernel locks                 live local exclusion authority
```

### Live SQLite tables

| Work adjuncts | `tool_responses`, `work_placements` | Tool answers and Home placement |
| Ask | `ask_exchanges`, `ask_linear_comment_outbox` | Blocking requests, answering-attempt fence, typed results, Linear publication |
| PM projection | `pm_snapshots`, `observation_outbox` | Bounded Linear reads and deferred child-event delivery to the Wave |
