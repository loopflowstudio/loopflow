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
no Wave, Project, Task, daemon, or readable planning database. Higher layers
supply composition, durable context, delivery, placement, and views around its
discovery, prompt, provider, harness, and evidence components. Their
controllers remain distinct because their recovery and settlement rules differ.

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
                        | end-to-end controllers   |
                        | Wave / Project / Task    |
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
- **Flow composition:** sequence Skill and mechanical Op nodes, route Xor
  branches, and stop at typed human boundaries.
- **Tracked Work:** preserve Wave, Project, and Task identity, inputs, status,
  and delivery facts without requiring a long-lived agent.
- **Controllers:** compose Work, Flows, execution, and delivery into an
  end-to-end automation layer above the substrate.
- **Task delivery:** attach one worktree and serial PR chain to concrete Work.
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
 Wave -> Project -> Task     Home-local Run record
       stable Work           manifest + JSONL + terminal
          |
          v
 repository + Git worktrees

exact local races use OS locks; remote execution uses lf ssh
```

- **Tracked Work** records purpose, input, and convergence; controllers decide
  what automation should happen next.
- **Execution evidence** records what one provider launch did.
- **Delivery** coordinates worktrees, commits, PRs, CI, and merge.
- **Machine authority** places Work and scopes local credentials, processes,
  files, and locks to a Home.

An identifier in one area is not authority in another. The detailed boundaries
below are the architecture's central constraint.

## Planning and execution vocabulary

```text
Wave
  `-- Project
        `-- Task
              `-- serial PRs

Flow = ordered Skill | Op | Xor | human boundaries
Run  = evidence for one mediated harness launch

WorkRef = Wave | Project | Task
WorkStatus = Ready | Done | Abandoned
```

| Concept | Meaning | Stable identity | Primary truth |
| --- | --- | --- | --- |
| Wave | Durable operating context: objective, memory, cadence, chat, and project selection | `WaveId` | Repository Wave files plus the Wave row and Linear Initiative membership |
| Project | One measured bet with definition, KRs, metrics, and Tasks | `ProjectId` | Linear Project, projected locally for bounded reads |
| Task | One concrete implementation, investigation, or document with one worktree and serial PRs | `TaskId` plus Linear identifier | Linear Issue, local delivery state, Git, and GitHub |
| Work | Shared planning state and input surface for a Wave, Project, or Task | `WorkRef` | The selected Wave/Project/Task row and domain facts |
| Skill | Reusable prompt instructions | Skill name and source | Repository override, builtin, or installed Skill file |
| Flow | Ordered Skill/Op nodes, Xor routing, and human boundaries | Flow name and stable node ids | Repository or builtin Flow YAML; the invoking process or controller owns its playhead |
| Run | Evidence from one mediated provider launch | `RunId` | One Home-local append-only record |
| Ask | One durable blocking request with typed result | `AskId` | Ask row, active answering Run fence, and result |
| Home | Stable machine authority whose network route may change | `HomeId` | Home row and observed SSH route |
| Placement | Assignment of Work to one Home | `(WorkRef, HomeId)` | `work_placements` |
| Steer | Ordered authored correction to one Work | `SteerId` | `steers` |

A provider-backed Flow boundary launches or continues a harness and therefore
produces Run evidence. Mechanical, routing, and human boundaries need not
create a Run.

## Core models and APIs

| Model | Main Rust types and APIs | Responsibility | Durability |
| --- | --- | --- | --- |
| Work | `WorkRef`, `WorkStatus`, `Steer`; Work completion/reopen/abandon and Steer APIs | Current tracked state and ordered domain input | SQLite rows keyed directly by Wave/Project/Task |
| Controller automation | Project/Task controller `State` and in-process `Playhead` | End-to-end playhead, provider continuation, and policy over current Work | Controller-owned SQLite rows keyed by Work id |
| Run record | [`RunSpec`](../rust/loopflow/src/run_record.rs), [`CaptureHandle`](../rust/loopflow/src/run_record.rs), [`RunManifest`](../rust/loopflow/src/run_record.rs), [`TerminalReceipt`](../rust/loopflow/src/run_record.rs) | Publish-before-spawn identity, append evidence, settle once | `$LF_HOME/runs/` |
| Run read model | [`RunSnapshot`](../rust/loopflow/src/run_record.rs), [`RunUsage`](../rust/loopflow/src/run_record.rs), `scan_runs_since` | Disposable local projection over record evidence | Rebuilt from files; no authoritative index |
| Ask | `Ask`, `AskClaim`, `AskResult`, `AskSession`; `claim_ask`, `release_ask`, `settle_ask` | Queue, attach route, exact answering attempt, first terminal result | `ask_exchanges` and comment outbox |
| Planning providers | `PmWave`, `PmProject`, `PmItem`, observation DTOs | Read and reconcile Linear/GitHub truth without turning projections into authority | Provider plus bounded local projections |
| Task delivery | `Task`, `TaskPr`, `PrLanding`, CI observations | Worktree identity, serial PR chain, checks, repair, merge disposition | SQLite + Git + GitHub |
| Home and placement | `Home`, `Placement`; Home observe/place/enable APIs | Stable machine identity and execution placement | `homes`, `work_placements` |
| Provider routing | `Provider`, `ProviderAccount`, `AccessProfile`, `ProviderRoute` | Credential authority, account selection, rate-limit/failover policy | Encrypted/local provider state and routing tables |
| Machine install | `ArtifactSet`, `SwitchReceipt`, promotion lock | Immutable artifact selection, service replacement, rollback | Versioned artifacts and switch receipts |
| Local process view | `ActivitySnapshot`, `ProcessPruneReport`, Exec receipts | Join current OS facts to local command receipts for observation and bounded cleanup | Process table + outer command ledger/receipt files |
| Git exclusion | rebase owner, Task PR mutation lock | Serialize only exact Git/worktree critical sections | Kernel-held file locks plus readable receipts |

## Code territory and rough size

These are physical lines in the current branch, rounded to the nearest hundred.
They include inline tests and comments; migration SQL and external test trees
are listed separately. The counts are navigation aids, not quality metrics.

| Territory | Main paths | Approx. LOC | What lives there |
| --- | --- | ---: | --- |
| CLI and presentation | `rust/loopflow/src/lf/`, `src/bin/` | 31,700 | Clap grammar, command dispatch, status/read models, terminal output |
| Operational workflows | `rust/loopflow/src/ops/` | 25,800 | Task/Project control, Ask, PR, Git, release, metrics, PM operations |
| Prompt and process engine | `rust/loopflow/src/engine/`, `src/harness/` | 29,300 | Skill/Flow discovery, prompt assembly, provider subprocesses and streams |
| Tracked Work | `work/`, `pm/` | — | Wave/Project/Task facts, Task delivery identity, planning/provider models |
| End-to-end controllers | `controller/` | — | Wave listener/chat, Project pursuit, Task playheads and automation |
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
| **User** — the human or external harness perspective | User-attributed actions author root input and decide effects that require human intervention. User is actor provenance, not a control credential. | [`Author`](../rust/loopflow/src/durable.rs) | No User row; authored effects persist on the concept they change. | `lf` | `lf :`, `lf desktop` | `exec:open`, `exec:osascript`, `exec:pbpaste`, `exec:id` |
| **Skill** — one reusable prompt with assembled context | Repository/builtin Skill Markdown is authoritative; discovery selects one source. | [`Skill`](../rust/loopflow/src/engine/flow.rs), [`SkillSource`](../rust/loopflow/src/lf/discovery.rs) | `.lf/skills/`, builtin Skill files, installed vendor Skill directories | `lf-prompt` | `lf skill`, `lf sync-skills`, `lf list` (Skill/Flow catalog) | `exec:python3` |
| **Flow** — an ordered composition of Skills | Repository/builtin Flow YAML defines the graph; the invoking process or controller owns its playhead. | [`Flow`](../rust/loopflow/src/engine/flow.rs), [`Playhead`](../rust/loopflow/src/controller/wave/playhead.rs) | `.lf/flows/`; controller cursor fields when used for end-to-end automation | `lf __flow-step` | `lf flow` | — |
| **Wave** — durable operating context with goal, memory, cadence, chat, and project selection | The Wave UUID is durable identity; canonical repository plus normalized slug is its mutable human locator. `wave/<name>/GOAL.md` and `MEMORY.md` own repository intent; the Linear Initiative owns shared planning membership. | [`Wave`](../rust/loopflow/src/work/wave/mod.rs), [`WaveLocator`](../rust/loopflow/src/work/wave/mod.rs), [`CanonicalRepo`](../rust/loopflow/src/repository.rs), [`WaveConfig`](../rust/loopflow/src/work/wave/config.rs) | `waves`; `wave/<name>/`; `.lf/journal/waves/<name>/journal.jsonl`; an in-flight relocation receipt under `.lf/tmp/wave-relocations/` | `lf __resident` behind the Wave listener; listener and relocation share the repository locator lock | `lf wave`, `lf start`, `lf stop`, `lf pause`, `lf resume`, `lf chat`, `lf ls`, `lf status`, `lf roadmap`, `lf cron`, `lf work relocate wave`; `wave GET /health`, `wave GET /conversation`, `wave GET /events`, `wave GET /playhead`, `wave POST /messages`, `wave POST /observations`, `wave POST /stop`, `wave POST /resident/attach`, `wave POST /resident/deltas`, `wave GET /resident/context` | Discord when configured |
| **Project** — one measured bet inside exactly one Wave | The Linear Project definition and KRs are planning truth; Project Work owns identity and facts, never a current execution slot. | [`Project`](../rust/loopflow/src/work/project.rs), [`PmProject`](../rust/loopflow/src/pm/mod.rs) | `projects`, `project_events`, `observation_outbox`; controller state in `project_controller_state`; Linear Project content | `lf __work` launches ordinary harness Runs from a deterministic controller session | `lf project`, `lf --project ...` | Linear; `exec:sh`, `exec:tmux` |
| **Live metric** — one reviewed measurement contract owned by exactly one Project, plus revision-bound current evidence | `wave/<name>/metrics/*.md` owns meaning and Project ownership; an accepted instrument observation owns its source-time fact; [`MetricPortfolioDto`](../rust/loopflow/src/controller/wave/metrics.rs) is the sole derived reading shared across surfaces. Metrics inform KRs but never complete them. | [`MetricContract`](../rust/loopflow/src/controller/wave/metrics.rs), [`MetricObservation`](../rust/loopflow/src/controller/wave/metrics.rs), [`MetricPortfolioDto`](../rust/loopflow/src/controller/wave/metrics.rs) | `wave/<name>/metrics/`, `metric_instruments`, `metric_observations` | Metric instruments write observations; foreground and resident Rust readers derive bounded portfolios. | Status/roadmap JSON, Wave and Project prompts, the shared Swift DTO, and Mac Wave detail expose the same `metric_portfolio`. | — |
| **Task** — concrete work inside exactly one Project | The Linear Issue owns directive/status; Task Work owns planning progress, one delivery worktree, and its serial PR chain. Git owns commits/branches; GitHub owns PR/check/merge truth. | [`Task`](../rust/loopflow/src/work/task/mod.rs), [`TaskPr`](../rust/loopflow/src/work/task/mod.rs) | `tasks`, `task_events`, `task_prs`, `task_pr_repair_incidents`, `task_linear_observations`, `task_linear_ingested_comments`; controller state in `task_controller_state`; Linear Issue; Git worktree | Independent `--task` Runs or a deterministic built-in controller may use the same Work; foreground operations record delivery evidence | `lf task`, `lf task prepare`, `lf task run`, `lf --task ...`, `lf pr`, `lf wt`, `lf rebase`, `lf commit` | Linear |
| **PR landing** — one watched attempt to merge an exact PR head | GitHub is authoritative for the PR head, required checks, and merge. One landing generation admits one supervisor and one repair per failed-head identity. | [`PrLanding`](../rust/loopflow/src/pr_landing.rs), [`CiIncident`](../rust/loopflow/src/work/task/mod.rs) | `pr_landings`, `ci_incidents` | Healthy Home daemon when it claims the generation; otherwise the invoking `lf pr land` process | `lf pr arm`, `lf pr land`, `lf ci`; `lfd POST /landings/claim` | `provider:github`, model provider for `ci-fix`, `exec:git`, `exec:gh` |
| **PM projection** — locally readable current planning snapshot | Linear remains authoritative; the Wave UUID keys the projection so locator changes preserve it. Sync atomically replaces the projection and reads never author through it. | [`PmSnapshotRow`](../rust/loopflow/src/store/mod.rs), [`PmWave`](../rust/loopflow/src/pm/mod.rs) | `pm_snapshots` | Foreground PM sync or Home webhook reconciliation | `lf pm` | `provider:linear` |
| **Steer** — durable authored correction to one Work | Stable Work identity names the destination; user or generic Run provenance names the author. Steers are ordered facts, not a global revision protocol. The Run id is never resolved as a capability. | [`Steer`](../rust/loopflow/src/durable.rs), [`Author`](../rust/loopflow/src/durable.rs) | `steers`, `tool_responses` | Store transaction; Task and Project controllers read at a boundary | Work-specific `steer` commands and `lf work` | — |
| **Ask** — one durable blocking request, typed result, and generic answering attempt | The target selects answering perspective. Ask claim mints an active generic Run id; that exact id fences presentation, release, and first terminal result. | [`Ask`](../rust/loopflow/src/durable.rs), [`AskClaim`](../rust/loopflow/src/durable.rs), [`AskResult`](../rust/loopflow/src/durable.rs) | `ask_exchanges`, `ask_linear_comment_outbox` | The asking command blocks without consuming turns; an Ask-specific session claims and settles it; Linear comments publish later | `lf ask` | Linear comments for Task exchanges |
| **Home / Placement / Promotion** — stable machine identity, Work placement, and artifact selection | `HomeId` is identity; SSH route is mutable. Placement is planning state and never process ownership. Promotion owns immutable artifact selection, isolated schema proof, service replacement, and rollback only. | [`Home`](../rust/loopflow/src/durable.rs), [`Placement`](../rust/loopflow/src/durable.rs), [`SwitchReceipt`](../rust/loopflow/src/machine_install.rs) | `homes`, `work_placements`; Home-local SQLite; machine install selection and switch receipts | `lfd` starts eligible Wave listeners; the promotion command owns only its OS-locked switch transaction | `lf home`, `lf ssh`, `lf install`; `lfd GET /health`, `lfd GET /status`, `lfd POST /waves/start`, `lfd POST /waves/stop`, `lfd POST /waves/reconcile`, `lfd POST /linear/webhook`, `lfd POST /github/webhook` | `exec:ssh`, `exec:launchctl`, `exec:systemctl`, `exec:/usr/bin/open`, `exec:/usr/bin/osascript` |
| **Run evidence** — one immutable harness record and one disposable projection | Run identity names launch evidence only. A replayable manifest records the exact prompt, agent/model, non-secret account identity, and tool boundary; replay creates an ordinary child Run. An unterminated record is unknown, not proven live, and may not authorize a Work mutation or signal. Provider usage remains cumulative direct evidence with explicit omissions, gaps, and provider finality. | [`RunManifest`](../rust/loopflow/src/run_record.rs), [`RunLaunchRequest`](../rust/loopflow/src/run_record.rs), [`RunSnapshot`](../rust/loopflow/src/run_record.rs), [`RunUsage`](../rust/loopflow/src/run_record.rs) | Home-local `runs/<prefix>/<run-id>/` | Harness launch creates the record; no central keeper repairs it | `lf runs`, `lf replay`, `lf usage`, `lf activity`; Work/status Run evidence | `exec:lf`, provider harnesses |
| **Browser capture** — one isolated, bounded screenshot transaction | The requested source, viewport, and output name the transaction; only a validated PNG replaces the output. The standalone shell identity and fresh process group keep capture separate from the user's browser and bound to its owner. | [`ScreenshotArgs`](../rust/loopflow/src/lf/mod.rs), [`ProcessGroupGuard`](../rust/loopflow/src/engine/process.rs) | Output PNG only; no control-store state | `lf __screenshot-supervisor` owns one `chrome-headless-shell` process group and observes the public command through a control pipe | `lf screenshot` | `exec:chrome-headless-shell` |
| **Local process observation** — outer command receipts joined to current OS facts | A live kernel process plus a matching local receipt is observation, not durable ownership. Registered orphan OpenCode groups may be reaped; unclaimed provider PIDs may not. | [`ActivitySnapshot`](../rust/loopflow/src/lf/commands/top.rs), [`ProcessPruneReport`](../rust/loopflow/src/lf/commands/top.rs) | `run_events`; Home-local Exec receipts and OpenCode server registry | The foreground observer samples the process table; no keeper asserts Run liveness | `lf ps`, `lf top`, `lf prune`, `lf doctor` | `exec:/bin/ps`, `exec:ps`, `exec:lsof`, `exec:kill`, `exec:which` |
| **Provider account / route** — credential authority and ordered provider selection on one Home | Provider token/account rows and Access Profiles own routing; credentials stay in provider homes, encrypted storage, Doppler, or forwarded foreground leases. | [`Provider`](../rust/loopflow/src/provider_auth/mod.rs), [`AccessProfile`](../rust/loopflow/src/profile.rs), [`ProviderRoute`](../rust/loopflow/src/profile.rs), [`ProviderAccount`](../rust/loopflow/src/store/mod.rs) | `access_profiles`, `account_access_profiles`, `provider_accounts`, `provider_account_limits`, `provider_routes`, `provider_session_accounts`, `provider_tokens`, `provider_deliveries` | The foreground auth command owns provider login process groups and passive browser handoff; durable processes use credentials installed on their Home | `lf auth`, `lf profile`, `lf route` | `provider:claude`, `provider:codex`, `provider:doppler`, `provider:opencodezen`, `exec:claude`, `exec:codex`, `exec:doppler`, `exec:opencode`, `exec:security`, `exec:secret-tool` |
| **Code-size measurement** — repository blobs measured in model tokens | Git blob identity owns content; token counts are deterministic memoized measurements, not Run usage. | [`CodeNode`](../rust/loopflow/src/lf/commands/tokens.rs), [`CodeSnapshot`](../rust/loopflow/src/lf/commands/tokens.rs) | `blob_tokens` | Foreground command only | `lf tokens` | — |
| **Schema frontier** — ordered definition of durable control storage | Released migration bytes are immutable authority; drafts join only through deterministic release materialization. | [`Migration`](../rust/loopflow/src/store/migrations.rs), [`MigrationId`](../rust/loopflow/src/store/migrations.rs) | `schema_migrations`; canonical and draft migration files | Store open validates/applies; release cut publishes | `scripts/install.py refresh`, `lf release` | — |
<!-- architecture-map:end -->

The public API column covers top-level command families, not every subcommand or
Rust function. [`lf` reference](lf.md) owns argument-level detail. DTOs emitted
by `--json` are required-field projections; Rust/Swift fixture tests own their
wire parity.

## Persistence map

Loopflow deliberately uses several stores because no one store owns all truth.

```text
repository files + Git        authored goals, memory, Skills, Flows, code
planning SQLite              local durable planning and delivery facts
Wave journal JSONL           conversation and resident event history
Run record files             one Home's provider-launch evidence
provider-native homes        model credentials and resumable sessions
Linear / GitHub              shared planning and delivery truth
machine install directory    immutable binaries and switch receipts
kernel locks                 live local exclusion authority
```

### Live SQLite tables

Grouping the current application tables by owner makes the database easier to
navigate:

| Owner | Tables | Purpose |
| --- | --- | --- |
| Tracked Work | `waves`, `projects`, `project_events`, `tasks`, `task_events` | Stable Wave/Project/Task identity, status, progress, and history |
| Controller automation | `project_controller_state`, `task_controller_state` | End-to-end playheads, provider continuation, and controller observations |
| Task delivery | `task_prs`, `task_pr_repair_incidents`, `task_linear_observations`, `task_linear_ingested_comments` | Serial PR chain and provider observations |
| Work input | `steers`, `tool_responses`, `work_placements` | Ordered corrections, tool answers, and Home placement |
| Ask | `ask_exchanges`, `ask_linear_comment_outbox` | Blocking requests, answering-attempt fence, typed results, Linear publication |
| PM projection | `pm_snapshots`, `observation_outbox` | Bounded Linear reads and deferred provider publication |
| Metrics | `metric_instruments`, `metric_observations` | Registered producers and accepted measurements |
| PR landing | `pr_landings`, `ci_incidents` | Exact PR-head supervision and bounded repair generations |
| Home and provider authority | `homes`, `access_profiles`, `account_access_profiles`, `provider_accounts`, `provider_account_limits`, `provider_routes`, `provider_session_accounts`, `provider_tokens`, `provider_deliveries` | Machine routes, credentials, selection, limits, and delivery receipts |
| Local observation/cache | `run_events`, `blob_tokens` | Outer command events and deterministic Git-blob token counts |
| Schema | `schema_migrations` | Applied migration identity and checksum frontier |

Released migration files are immutable history. Draft migrations form an
ordered development frontier and become released bytes only during the release
workflow.

### Filesystem state

| Location | Contents | Write pattern |
| --- | --- | --- |
| `.lf/skills/`, `.lf/flows/`, `.lf/config.yaml` | Repository-owned execution definitions | Authored and reviewed with code |
| `wave/<name>/GOAL.md`, `MEMORY.md`, `metrics/` | Wave intent, curated memory, metric contracts | Authored and reviewed with code |
| `.lf/journal/waves/<name>/journal.jsonl` | Wave conversation/resident events | Append-only with crash-tail repair |
| `$LF_HOME/runs/<prefix>/<run-id>/` | Run manifest, event streams, terminal receipt | Publish once, append streams, settle once |
| Home provider directories | Provider-native login and resume state | Owned by provider adapters |
| Git directory `loopflow/` receipts | writer/rebase/PR mutation coordination | Kernel-locked receipt files |
| machine install root | Versioned artifact sets and switch receipts | Stage immutably, select atomically |

### External systems

Linear owns Initiative/Project/Issue planning shared with humans. GitHub owns
PR heads, checks, and merge. Git owns commits and worktrees. Model providers own
their session and usage semantics. Local rows cache or record observations from
those systems; they never silently become substitute authority.

## Processes and public APIs

```text
interactive shell / automation / Loopflow.app
                  |
                  v
                 lf
       +----------+-----------+
       |          |           |
       v          v           v
 planning APIs   Skill run    Git/PR operations
       |          |           |
       |          v           +---- Linear / GitHub
       |       provider
       |          |
       v          v
 SQLite       Run record

lfd -> Wave listener -> resident -> Project controllers -> Task controllers
Task/Project/Wave-bound one-shot Runs -------> shared Skill execution components
```

| Surface | Responsibility | Scope |
| --- | --- | --- |
| `lf <skill>` and `lf flow` | Direct Skill execution and Flow composition | Current process and Home |
| `lf wave`, `project`, `task`, `work`, `ask` | Durable planning and communication | Work resolved in the current planning store |
| `lf wt`, `commit`, `rebase`, `pr`, `ci` | Worktree and delivery operations | Exact repository/Task/GitHub object |
| `lf runs`, `usage`, `ps`, `top`, `prune`, `doctor` | Execution and process observation | Current Home only |
| `lf home`, `start`, `stop`, `pause`, `resume` | Home identity and Wave service lifecycle | Current Home unless routed explicitly |
| `lf ssh <home-id> <args...>` | Run the target Home's `lf` | Explicit remote Home; no implicit fan-out |
| `lfd` HTTP API | Start/stop/reconcile Waves, receive webhooks, claim landings | One Home |
| Wave HTTP API | Conversation, events, playhead, messages, observations, resident attachment | One Wave listener |
| Loopflow.app | Swift projections and human interaction | Queries the same DTOs and remote routes; owns no lifecycle |

Most commands are local by default. `lf ssh` is transport, not a second API:
the inner `lf` and separator are implicit, the target re-resolves its own Home
state, and durable processes scrub foreground-forwarded secrets before
detaching.

## Harness launch and Run records

Every Loopflow-mediated provider launch creates one Run record. Task, Project,
Wave, Ask, direct CLI, and internal operations may assemble different prompts,
but they use the same `CaptureHandle` recorder and evidence format.

```text
$LF_HOME/runs/<first-two-uuid-chars>/<run-id>/
  manifest.json       required, immutable, published before spawn
  events.jsonl        optional append-only lifecycle, conversation, tool, provider, and usage evidence
  terminal.json       optional immutable terminal proof, exclusive-create
```

The launch sequence is deliberately short:

1. Build `RunSpec` from the launch facts available now.
2. Write `manifest.json` in a private staging directory, sync it, atomically
   rename the directory into place, and sync its parent.
3. Export `LF_RUN_ID`, `LF_RUN_DIR`, and a verified `LF_PARENT_RUN_ID` when one
   exists.
4. Spawn the provider and append evidence without waiting for shared storage.
5. Create exactly one `terminal.json` with `completed`, `failed`, or
   `interrupted`.

Manifest publication is the only new pre-spawn persistence requirement. The
manifest records launch facts: Run and optional parent ids, creation time,
harness/model/surface, cwd/repository/worktree, skill, subject attributions,
runtime path/digest when available, host, and boot id. It contains no mutable
state, liveness, current Work revision, credential, or signal target.

`CaptureHandle::record_*` methods enqueue bounded best-effort writes. A full or
broken queue warns once and the harness continues. JSONL append does not sync
per event. Terminal creation is synchronous and durable; a conflicting second
outcome loses without rewriting the winner. Settlement then permits a bounded
250 ms telemetry drain, but a missing event cannot change the terminal result.

A provider retry or account failover is another `attempt_key` inside the same
Run. A new provider Turn creates a new `usage_stream_id`. Usage points preserve
provider-authored cumulative counters, omissions, sequence, and
`final_receipt`; Run settlement never synthesizes provider finality and readers
must not sum cumulative checkpoints.

Planning enrichment is optional. Raw `--task LOO-123 implement` records its
declared selector even when planning SQLite is unreadable, warns if enrichment
fails, and launches from the available repository/cwd. Multiple hierarchical
selectors require planning state because Loopflow must prove they match.
Selector resolution can add context; it cannot reserve Work or authorize a
mutation.

## Tracked Work and end-to-end controllers

Tracked Work is a complete substrate: Wave, Project, and Task own objectives,
KRs/input, progress, terminal state, and delivery evidence. Their stable
identity joins inputs and observations. Any caller may launch zero, one, or
many Work-bound Runs and use delivery operations without installing a
controller.

Above that substrate, Wave, Project, Task, and Ask keep distinct controller
loops because their recovery and settlement contracts differ. They reuse Skill
discovery, prompt assembly, provider routing, harnesses, Work, and delivery
rather than introducing a second execution system.

```text
Wave listener
  |-- maintain conversation and portfolio view
  `-- choose Project work
        |
        v
Project controller
  |-- refresh Linear definition, KRs, metrics, and Tasks
  |-- run clarify/pursue/mutate Skills
  `-- create or supervise Task work
        |
        v
Task controller in managed worktree
  |-- run first / loop / finally Flows
  |-- consume Steers, Asks, provider observations, and PR facts
  `-- publish one serial PR at a time
```

Each controller rebuilds its next prompt from current durable facts at a
boundary, invokes the shared execution path when that boundary is a Skill, and
records the resulting domain transition. A controller crash loses in-memory
work but not the Work identity, inputs, worktree, or provider observations.
Controller playhead and provider continuation survive in controller-owned rows
separate from Task Work. One-shot Task-bound Runs neither need nor advance
those rows.

Planning uses the boundary matching each real race:

- The built-in Task controller has one stable local session. Restart addresses
  that exact session, stops it before replacing controller state, and starts a
  new provider. Task attribution on generic Runs is never process-control or
  mutation authority.
- Ask claims and terminal results use the Ask's active generic Run id.
- PR publication, repair, range healing, merge request, settlement, and serial
  rotation resolve the managed Task worktree and take the Task PR mutation
  lock around the filesystem/provider boundary.
- Wave relocation uses the live listener/locator lock and a crash-recovery
  receipt across the filesystem/SQLite boundary.
- Linear and GitHub observations remain provider evidence for the transaction
  that consumes them.

`WorkStatus` is `Ready`, `Done`, or `Abandoned`. Runtime activity is a separate
observation surface. Reopen clears transient input and returns the same Work
identity to `Ready`; complete and abandon settle current planning state.

Controller policy remains useful without becoming Run authority. Project and
Task controllers use deterministic tmux session names. Task restart interrupts
and replaces that registered controller session; if it is absent, restart
starts one. Multiple off-script one-shot Runs may still concern the same Task.
A Wave listener owns the resident process it directly spawned. These are local
placement/supervision facts, not durable cross-process Run ownership, and they
are never published as `owner.json`.

When a provider or Task body disappears, the controller records resumable
planning failure and returns judgment to the Project. A later controller
decision can launch a fresh Run from current planning state.

## Durable communication

Steer is ordered Work input. `Author::Run` stores a generic Run id byte-for-byte
as provenance; the store never requires a matching Run record.
Live provider delivery is an optimization. A later successful planning
boundary is the semantic receipt.

```bash
lf task steer INF-123 "keep the public name"
lf work steer task task_... "show the failing fixture"
```

### Ask creation and terminal result

Ask is its own durable protocol. Creation captures origin Work, optional source
Run provenance, Home, cwd, target, and request. It does not enter the Steer queue
or rewrite Work state.

```bash
lf ask "Which behavior should this proof cover?"  # block without spending turns
lf ask wait                                       # recover after shell loss

lf ask list --outgoing                            # this Work's unresolved requests
lf ask list --user --json                         # User attention projection
lf ask open ask_...                                # claim or reattach one Ask session
```

The Ask keeps its identity while generic answering Runs come and go:

```text
Ask(id, origin Work/source Run/Home/cwd, target, request,
    state, active_run_id, ready/presented timestamps, result)
```

Claiming mints one active generic Run id and starts an Ask-specific tmux session
in the captured cwd. Presentation, release, and settlement must name that exact
Ask/Run pair. The first typed terminal result wins.

The origin is captured when the Ask is created. Its cwd comes from the current
execution context when available, not a path reconstructed later. One Work or
source Run may create several unresolved Asks; duplicated request text still
mints distinct ids. Flow-step identity is its expanded flow, stable node id,
and skill.

The target is selected when the Ask is created:

- a child routes to its immediate parent Work;
- `--user` routes explicitly to the User perspective;
- a root without `--user` fails instead of silently spending User attention.

The target selects the perspective and context used to answer the Ask; it does
not authorize one Work over another. Source Run id is provenance. Only the
Ask's active answering Run id fences that Ask's attempt and terminal result.

An unresolved Ask remains actionable while its Work is open. Completing,
abandoning, or reopening the Work cancels unresolved transient state directly.
Provider, shell, waiter, or runner loss never invents success.

`lf ask` commits before it wakes the parent, polls without consuming model
tokens, retries the wake, and prints the typed terminal result to stdout. The
provider sees an ordinary long-running shell command; Loopflow needs no
provider-specific injected tool or mid-turn message transport.

Each Task Ask creation and terminal result also enqueues a Linear issue comment
in the same transaction. Linear publishes afterward: failures remain in the
durable outbox for retry and cannot roll back or delay settlement. Ask
attempts and presentation failures do not create comments.

Opening an Ask claims one answering Run and starts it in the captured cwd.
`Ready` means the Ask session's exact attach route exists; presentation moves it
to active attention. Resolve or decline completes it. Release, ordinary exit,
or proven local disappearance requeues the same Ask. Unreachable remote
liveness remains claimed instead of being guessed absent.

Loopflow.app is a projection over the same queue and Ask session route. Swift
owns no Ask lifecycle, attempt identity, or queue state.

## Flow execution

Flows expand to serial Skill nodes, mechanical Op nodes, Xor routing, and typed
human boundaries. Skills are the judgment kernel, but not every executable
Flow node needs a provider.

Task flows run serially. A provider blocked inside `lf ask` keeps its current
shell call while planning shows the outstanding Ask, not a `Running` Work
lease. A headless Task that reaches `human: true` records the playhead and
queues one User `FlowStep` Ask without starting a provider merely to wait.
Resolve completes that node; decline returns to the preceding autonomous step
with the reason; release or incomplete exit requeues without advancing.

Project and Wave use a separate Ask lane for child questions. The lane claims
parent-targeted Asks and starts narrow answering Runs without disturbing the
core conversation.

Direct TTY flows use their present conversation for human nodes. Headless Task
flows use the Ask session above. The launch surface, rather than Skill
frontmatter, selects which human boundary applies.

## Task delivery algorithm

A Task binds planning to one managed Git worktree and one serial PR chain.

```text
Linear Issue
    |
    v
Task row ----> managed worktree ----> commits
    |                                  |
    |                                  v
    +-----------------------------> GitHub PR
                                       |
                              checks / repair / merge
                                       |
                         complete Task or rotate next PR
```

1. `lf task prepare` resolves one Linear Issue inside one Project and creates
   or reuses Task Work, its worktree, and its serial PR identity. It records no
   controller lifecycle.
2. Independent `lf --task ...` Runs may work in that substrate directly.
   `lf task run` additionally installs or resumes the built-in controller and
   launches its current Flow through the same execution components.
3. `lf commit` snapshots the worktree. `lf pr publish` creates or refreshes the
   current PR without opening a browser.
4. Managed Task worktrees refuse `lf pr submit`: the `finally` review is their
   one human shipping decision. `lf pr arm` requests exact-head auto-merge and
   returns; `lf pr land` declares the reviewed outcome and watches through
   merge. These operations follow durable Task delivery state and require no
   live controller.
5. PR landing is fenced by exact PR head and landing generation. A failing head
   may admit one repair; a moved head requires fresh evidence.
6. Merge either completes the Task or rotates its serial chain to a new branch
   from fetched main. Simultaneously open dependent work uses a separate Task
   stacked on the parent's PR.

GitHub remains merge truth. SQLite stores the observed PR/head/check/disposition
needed to resume safely; it cannot declare an unmerged PR merged.

## OS locks and allowed contention

Loopflow uses advisory OS file locks for exact local critical sections. The
open file descriptor holds authority; the JSON file is a readable receipt.
Process death releases the kernel lock even if the receipt remains, so the next
operation can clean or explicitly adopt stale metadata.

### Git mutation and rebase locks

For a Git worktree, `absolute_git_dir` selects the real Git directory, including
the linked-worktree case. Rebase coordination lives beneath it:

```text
<absolute-git-dir>/loopflow/rebase-owner.json
```

Provider Runs receive no worktree writer token. Commit, PR mutation, restart
checkpointing, and land take short OS-held locks only around their exact Git
mutation. Independent agents may edit and run concurrently; the shared
worktree remains the durable blackboard.

A rebase takes an exclusive lock on `rebase-owner.json` for the complete Git
sequencer lifetime. New agent launches refuse while that rebase lock is live.
The exact `LF_GIT_OPERATION_ID` lets only the operation's recovery child
continue or abort inside the fence. A raw or crashed rebase can be adopted only
through the explicit adoption path, which mints a new id.

Therefore:

- agent + agent is allowed;
- reader/build/test + agent is allowed;
- rebase + independent agent is blocked in both start orders;
- rebase + its exact recovery child is allowed;
- stale JSON with no OS lock is not a live owner;
- raw Git outside Loopflow is not compelled by these advisory locks.

### Short mutation and machine locks

`<absolute-git-dir>/lf-pr-mutation.lock` serializes only Task PR/head mutation
sections; a second such operation fails fast while the first guard is alive.
Wave locator locks serialize listener/relocation filesystem ownership. A
`<store>.migration.lock` serializes backup plus schema application. The current
promotion operation holds `$HOME/.lf/promotion.lock` exclusively for its full
upgrade transaction. These locks do not turn a Run id into authority.

## Process ownership and control

Run records contain no process owner. A PID, tmux name, ambient process group,
Work identity, parent Run, writer lock, or telemetry row is insufficient signal
authority. The process that directly spawned a child may cancel its own child
handle; that local capability is not recoverable cross-process control.

Generic cross-process Work/Task interrupt therefore refuses without exact
ownership evidence. If durable Run control is added, the launcher must create a
fresh process scope and publish an owner receipt containing PID plus kernel
birth identity, boot/Home identity, and the exact process group/session/native
scope. Every later signal must revalidate it. File inbox envelopes can provide
durable stop/steer; a same-version socket may optimize latency but cannot be the
required protocol.

## Homes and process topology

```text
Loopflow.app / shell / external harness
                 |
                 v
                lf ---------------- Linear / GitHub / provider auth
                 |
       planning SQLite + repository/Git
                 |
                 v
          lfd / Wave listener -------- deterministic controllers
                                              |
                                              v
                                     provider harness
                                              |
                                              v
                                     Home-local Run record
```

`lfd` starts eligible placed Wave listeners, reconciles services and webhook
deliveries, and serves Home endpoints. The Wave listener owns HTTP, discovery,
journal, and the resident child it directly spawned. Crossing Homes is an
explicit `lf ssh` hop whose target proves its Home identity.

### Multi-Home placement and execution

`HomeId` is stable machine identity. Its observed SSH route may change without
moving Work. `Placement` maps one `WorkRef` to one Home and can independently
enable or disable automatic startup.

```text
origin Home                         target Home
-----------                         -----------
lf work place ... home_B  ------->  HomeId = home_B

lf ssh home_B start wave_X
        |
        `--- SSH transport --------> target `lf start wave_X`
                                      |
                                      v
                                     lfd
                                      |
                                      v
                                Wave listener/resident
                                      |
                                      v
                               local provider + Run record
```

The operating sequence is:

1. `lf home observe` records a stable Home id and current route.
2. `lf work place` records where a Wave/Project/Task belongs.
3. A local command acts on the current Home. `lf ssh <home> ...` runs the same
   command on the target Home after verifying identity.
4. `lfd` starts only eligible, enabled Work placed on that Home.
5. The target uses its own planning store, repository/worktree, provider homes,
   service manager, OS locks, and Run directory.

Run records and process observations are not replicated. To inspect another
Home, run the reader there through `lf ssh`. Foreground SSH may explicitly
forward selected account authority; detached processes must use credentials
installed on the target.

Wave selection always resolves `(canonical repository, slug)` to one UUID.
Bare-slug diagnostics fail when more than one repository owns the slug; no
read or mutation chooses one by order. A scoped lookup repairs an equivalent
legacy path spelling to the canonical repository in one transaction.
`lf work relocate wave <uuid>` is the only semantic locator mutation: it fences
the Wave chord, moves authored files and the journal, commits the new locator
transactionally, and leaves PM, Work, and Home-placement rows joined to the
unchanged UUID. A target-local `.lf/tmp/wave-relocations/<uuid>.json` receipt
bridges the filesystem/SQLite commit boundary; retrying after a committed crash
finishes verified source cleanup, then removes the receipt. Repository moves
also require compatible configured PM Teams so relocation cannot impersonate
the separate `lf pm reteam` operation.

## Promotion and long-running old processes

1. Verify and install immutable versioned artifacts.
2. Copy the selected planning store, apply the candidate schema to that
   isolated copy, and prove the candidate can read it.
3. Atomically repoint the launcher used by future top-level processes.
4. Let already-running processes continue with their selected executable.
5. Restart only the Home services and app surfaces actually being replaced.
6. Recover or roll back from the persisted artifact-selection receipt.
7. Garbage-collect old artifacts separately from activation.

The machine-wide promotion lock serializes artifact selection and service
replacement. It does not discover, drain, stop, or settle Runs and it is not
held by ordinary harnesses. Store cloning remains useful because preview can
prove a candidate schema without mutating the selected store.

A long-running old `lf` process is isolated from new Run recording because each
launch writes its own Run record. It retains both its executable and selected store
path. On the first published-to-development switch, the process may keep
writing successfully to the prior production store after new commands select
the cloned development store; those writes become invisible to the new
selection. A later development-to-development promotion may reuse and migrate
the selected store, so an old writer may instead fail against changed schema.
Promotion pauses and replaces the known services it owns but does not discover
every shell or provider process. The clone proves candidate readability; it
does not provide cross-store write continuity or old-schema compatibility.

## Truth and projections

The map is the ownership index. Truth remains distributed across Home-local
SQLite, repository files and Git, the Wave journal, Linear, GitHub, and provider
homes or Doppler; none is a fallback authority for another.

Intentional copies stay read projections:

<!-- architecture-projections:start -->
| Projection | Authority copied | Freshness and consumer |
| --- | --- | --- |
| [`PmSnapshotRow`](../rust/loopflow/src/store/mod.rs) / `pm_snapshots` | Linear planning | Atomic sync or Project-phase refresh replacement; `lf status`, `lf roadmap`, and the Mac app read it but never author through it. |
| [`TaskLinearObservation`](../rust/loopflow/src/work/task/mod.rs) / `task_linear_observations` | Linear Issue state | Reconciliation records provider evidence before applying lifecycle changes. |
| [`GithubObservation`](../rust/loopflow/src/work/task/mod.rs) / `task_prs`, `ci_incidents` | GitHub PR/check state | Webhook or foreground reads update Task delivery evidence; GitHub remains merge truth. |
| `tests/fixtures/dto/` | Rust `lf --json` DTOs | Rust and Swift fixture tests reject required-field or enum drift. |
| `tests/fixtures/migrations/` | Ordinal-free migration drafts and the Python canonicalizer | Rust build/runtime and Python release tests reject ordering, body-byte, checksum, and graph-error drift. |
<!-- architecture-projections:end -->

`lf status` and `lf roadmap` derive planning lifecycle from Work and concrete
Ask/flow/PR facts. Pending User attention is a projection over queued/claimed
Asks and their Ask-specific session route. Run and Work-activity surfaces
reduce Home-local Run records directly. No projection may become launch,
Work-mutation, credential, or signal authority.

## Extension rules

| Area | Safe extension | Architectural constraint |
| --- | --- | --- |
| Run queries | Add a disposable local or multi-Home index after measured need | Run-record files remain evidence truth; index failure cannot gate launch |
| Multi-Home views | Fan out read-only commands through `lf ssh` | Do not centralize Run ownership or silently mix local and remote scope |
| Process control | Publish birth-validated ownership at the launcher spawn seam | No PID/tmux/Work/telemetry inference |
| Planning input | Add a naturally keyed fact or provider observation | Do not create a global input revision protocol |
| Provider support | Add a provider adapter, account route, and normalized stream mapping | Provider credentials/finality remain provider-authored |
| Promotion | Add artifact roles or service adapters within the locked switch transaction | Artifact activation does not depend on Run discovery |

A new writer must have one authoritative model. Avoid backend dispatch between
old and new representations, synchronized SQLite/filesystem commits, mandatory
collector daemons, or planning capabilities derived from observation data.

## Appendix: compatibility seams

Compatibility survives only when it crosses immutable external history. Each
seam names its translation and deletion boundary; none is a second current
model.

<!-- architecture-shims:start -->
| Seam | Current concept | Source and removal boundary |
| --- | --- | --- |
| `shim:legacy-chat-import` | Old journal turns become one immutable Wave conversation epoch. | [`ConversationEpochImport`](../rust/loopflow/src/controller/wave/journal.rs); remove only when old journals are no longer supported. |
| `shim:retired-op` / `lf op` | Rejected namespace returns the surviving top-level command name. | [`Commands`](../rust/loopflow/src/lf/mod.rs); remove when external callers no longer need the diagnostic tombstone. |
| `shim:rams-alias` | Installed `rams/rams` command resolves to the Skill model. | [`SkillSource`](../rust/loopflow/src/lf/discovery.rs); remove when the external single-file command is no longer supported. |
| `shim:local-refresh-wrapper` | Old script entrypoint forwards to the single `scripts/install.py refresh` implementation. | [`pull-local-bin.sh`](../scripts/pull-local-bin.sh); remove after external automation uses the current command. |
| `shim:retired-app-replacement` | Promotion removes the previously shipped app bundle after the current app commits. | [`AppPromotion`](../rust/loopflow/src/lf/commands/install.rs); remove after the retired bundle name is outside supported installs. |
<!-- architecture-shims:end -->

## Appendix: historical-only vocabulary

The scanner matches exact phrases, not overloaded words. Provider resume
sessions, tmux sessions, and `session.launch` are current. The authored chat
reference `project:<slug>` is also current; it is not the old Linear-label PM
model.

<!-- architecture-vocabulary:start -->
| Retired term | Allowed scopes | Current language |
| --- | --- | --- |
| `Project Session`, `Task Session`, `project_sessions`, `task_sessions` | `rust/loopflow/src/store/migrations/`, `rust/loopflow/src/store/migrations.rs`, `rust/loopflow/src/store/tests/fixtures/`, `release/` | Stable Project/Task **Work** plus generic Run evidence. |
| `session context`, `LF_SESSION` | — | Stable Work identity plus `LF_RUN_ID`/`LF_RUN_DIR` execution evidence. |
| `lf radio`, `agent bus` | `release/` | Typed Work observations, Steer, and Ask. |
| `pm.linear_project`, `projects/<slug>.md` | `release/` | `pm.linear_initiative`; Linear Initiative → Project → Issue. |
| `machine-local host`, `machine-global command`, `machine-global mutation`, `machine-global reservation` | — | Home-local keeper, command, mutation, or reservation. |
<!-- architecture-vocabulary:end -->

Canonical migrations, migration fixtures, and release notes retain historical
names because changing shipped evidence would rewrite history. Operational docs
and current runtime source do not.

## Authority and failure invariants

- Wave → Project → Task is the complete planning hierarchy: no recursive or
  orphan Projects.
- A Wave UUID is stable across rename and repository rehome; repository-scoped
  locators are unique, and bare slugs are never mutation authority.
- Linear owns current Project/Task planning; SQLite projections never become an
  authoring fallback.
- Active Project Work adopts one complete refreshed Linear plan between provider
  turns; a refresh or ownership failure stops before the next turn rather than
  serving the prior plan.
- An ad-hoc Skill run can launch with its repository/cwd and declared subject
  even when planning storage is unavailable.
- Every Loopflow-mediated harness launch publishes one immutable manifest
  before spawn and creates at most one immutable terminal receipt.
- JSONL telemetry is best effort and never gates launch or settlement.
- Run parentage, subject attribution, outcome, and usage are evidence only.
- Work status describes planning convergence; process and Run activity are
  separate observations. One Work may concern zero, one, or many Runs.
- Generic Run ids stored in Steer/Ask/Task history are opaque provenance unless
  a reader independently resolves their Run record; resolution never grants
  authority.
- No `owner.json` means no durable cross-process Run signal authority.
- OS file-lock ownership is scoped to its documented local critical section;
  it never grants Work or credential authority.
- Multiple independent agent writers may coexist. A live rebase excludes them;
  only its exact recovery child may enter that sequencer.
- Durable Ask is the only human-input primitive that blocks a headless flow
  boundary. One Work or source Run may own several unresolved Asks; explicit Ask
  ids select precise mutations.
- An Ask result is typed, authorized, immutable, and first-writer-wins.
- Ask and Steer are separate input protocols: Ask blocks for a typed answer;
  Steer appends an authored correction.
- Terminal Work exposes no actionable Ask attention.
- Promotion preview may migrate an isolated store clone. Activation of a new
  artifact and writes by older planning binaries are separate concerns.
- Commands that observe Runs, usage, or processes are Home-local unless the
  caller explicitly routes them through `lf ssh`.
- DTO fields are required unless their type is explicitly optional.

## Drift proof

```bash
uv run python scripts/check_architecture.py
```

The bounded check materializes the live schema (including drafts), discovers
root CLI families, binaries/internal process commands, both local HTTP routers,
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
