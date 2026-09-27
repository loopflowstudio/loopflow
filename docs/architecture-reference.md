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
                                                              `--< Run

Session --< Run                                    Run.session_id is nullable
Session.current_run_id -> Run                      points to one of its Runs

Run.invocation_id                                  nullable; invocation Task is optional
Run.task_id => Run.wave_id                         nullable, filled upward
Flow invocation.parent_id                          runtime nesting only
```

The Run arrows above describe execution ownership. Independent Runs can belong
to a Task, only a Wave, or neither. Session owns a Run history and selects its
current Run; it has no separate Work parent.
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
| Session | `SessionId` | One conversation's kind, name, readiness, completion, Runs and current Run |
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
| `tasks` | `project_id`, provider Issue identity, Work state, worktree, delivery facts and validated set-once `started_at`; derive Wave through Project |
| `flow_invocations` | Nullable Task, nullable runtime parent, captured Flow name/source and complete graph, local node cursor, loop return counts, status, claim/version/generation, pending boundary; create, claim, settle, interrupt, restart |
| `runs` | ID, nullable `session_id` FK, causal parent Run, Home/repository/cwd, provider/model/skill, timestamps/outcome, nullable `invocation_id`, `task_id`, `wave_id`, `work_source`, node, iteration tuple and `membership_known`; create, settle, bind, query |
| `sessions` | Stable `id`, `current_run_id` FK; `kind` = `interactive | flow_review | ask`; `title`, `title_source` = `generated | human`; `state` = `waiting | active | ready | closed`; `ready_summary`; query Runs, open, ready, complete, rename |

A Session reads ancestry, cwd, provider, and Flow membership through its current Run.
Session rows do not copy these fields. Headless Runs may have an optional name;
a conversational Run's displayed name is its Session title, not a synchronized
second title. Native provider identity and attachment receipts are Run-owned
operational data in SQLite; the provider still owns its transcript. Ask request,
caller, selected Skill, keyed retry identity and completed result are Session
kind-specific fields. There is no separate Ask file or Session registry.

`Session.runs` queries `runs.session_id`; it is not a second stored list.
`current_run_id` must refer to a Run whose `session_id` is this Session.
Creation reserves both records and the current pointer in one transaction;
replacement appends a Run and changes the pointer atomically, comparing the
expected previous Run. No committed Session has a missing or foreign current
Run. Closed Sessions retain the pointer and their complete Run history.
Session identity, title and feedback survive replacement. Run outcomes, usage,
native identities and process receipts stay attached to their original Runs.

A step position is `(invocation, node, iteration tuple)`. It has zero or more
Runs, ordered as attempts, and one current attempt once execution is reserved.
That position key is an index, never a unique Run identity. Headless steps and
human reviews share the current-attempt selection and version fence. A Session
is the conversational instance of this rule: its current Run agrees with the
invocation's current attempt while that review is pending.

Failure, interruption or abandonment retains the attempt without advancing the
cursor. One successful current attempt completes the step; a superseded Run
cannot submit a decision or settle it. Retries stay at the same node and loop
tuple. Returning through a loop creates a different position, not a retry.
Position readers and DTOs expose the attempt list and selected Run. Node details
and Session membership show all attempts. Usage and execution duration sum over
attempts; a running status line measures only its current attempt. Offline import
preserves repeated attempts rather than deduplicating by position.

### Three validator groups

| Validator | Enforced contract | Mutation boundary |
| --- | --- | --- |
| Planning ancestry | Project's Wave and Chapter name the same repository; `(wave_id, chapter_id)` is unique; a repository has one current Chapter; Task's Wave is derived from its Project | Plan writes and atomic chapter activation |
| Invocation structure | Invocation has an optional Task; parent has the same nullable Task; parent chain is acyclic and describes runtime entry; node exists in the captured expanded graph; one current root per Task when Task-owned | Invocation creation, child entry, cursor settlement, restart |
| Run ancestry | Invocation supplies its nullable Task; a present Task fills Wave; supplied parents must match. An invocation-owned Run has a valid node/iteration tuple even when taskless; an independent Run has neither. Membership never changes through bind; Task assignment is write-once and existing Wave is retained; Started timestamp presence equals Task Run existence | All Run creation, binding, import and ancestry-changing writes |

SQLite foreign keys, uniqueness and nullability constraints back these APIs.
Cross-row checks run in the same write transaction as the change. Task moves
within a Wave leave Run ancestry unchanged; a cross-Wave move validates and
updates dependent Run Wave values atomically. It cannot silently invalidate
historical execution. Current attribution and immutable launch evidence are
different facts.

Every new Run records known membership: either a validated invocation location
or an independent launch. Imported history without execution evidence has
`membership_known = false` and no claimed invocation location. Readers show
Unknown, never Independent, for those rows. A known invocation link requires
known membership. Binding cannot manufacture missing execution evidence.

`work_source` is nullable for no attribution, otherwise `declared`, `checkout`,
`inherited`, or `bound`. Explicit launch selectors win over checkout inference.
An Ask copies its caller's Task/Wave with `inherited`; it does not inherit the
caller's invocation just because it shares a checkout. Only a Flow driver sets
execution membership on Runs it launches.

Bind fills missing ancestry with `bound` provenance. A Run with no parent may
receive a Wave or a Task; a Wave-only Run may receive a Task in that same Wave.
Once set, its Task cannot change or be cleared. Bind never changes an existing
Wave. Binding to a done or landed Task is allowed and does not reopen it. The
operation and every UI calling it state the exact target and require one
confirmation before the permanent assignment. There is no unbind operation.
The transaction validates the selected current Run and ancestry together;
concurrent binding cannot redirect the confirmed target.

Invocation membership still constrains nullable Task equality: a taskless
invocation's Run cannot independently acquire a Task. Binding never rewrites
the invocation, node, iteration tuple, launch artifact, provider history,
Session identity or name. Authorized cross-Wave Task moves retain their separate
operation and update dependent Run Wave values atomically.

A Task is started when any Run has that Task. `tasks.started_at` records when
it first receives a Run: the shared Run writer sets it only if null, in the
same transaction as launch or bind. A first bind uses the bind time, not the
Run's creation time. Later launches and binds leave the timestamp unchanged,
even for older Runs. It never moves earlier, later, or back to null.

The write validator checks `started_at IS NOT NULL` exactly when a Run with
that Task exists; it does not compare timestamps with `MIN(created_at)`. Offline
import sets it for every Task with imported Runs and leaves other Tasks null.
The sidebar and roadmap read this column through one Started reader. This
replaces the Started event writer once all readers use the validated column.
Write-once binding keeps the evidence monotonic; usage and history never move
between Tasks. Chapter retirement also examines authored work, PRs and active
invocation claims: absence of a Run alone never proves untouched backlog.

The shared readers select Session rows joined to their current Runs, and Runs by their typed
parents. `runs --task`, `session list --task`, usage, activity and the desktop
agree. No read calls the launch resolver or requires an active PR. A Session
with null Task is an orphan in the workspace, including a Wave-only Session;
a Task-bound Session missing from a visible roadmap remains bound and reachable.
Swift caches the workspace projection by its input readings and uses IDs for
forward and reverse lookup.

Session lifecycle is explicit: `closed` requires completion; `ready` retains
saved feedback. `active` and `waiting` reflect exact provider-client evidence,
reconciled before returning a current reading. A stale state field cannot
authorize signaling or turn provider exit into Session completion.

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
| **Session** — one conversation across Runs | Session row owns name, readiness and completion. Current Run supplies ancestry and provider identity; Session owns the Run history. Complete returns saved feedback; only the following deciding Run chooses navigation. | `SessionRecord`, `SessionId`, `RunId` | `sessions` | `lf __provider-session` records native identity; Session operations own state | `lf session`, `lf ask`, interactive `lf` | — |
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

| Owner | Tables | Purpose |
| --- | --- | --- |
| Planning | `chapters`, `waves`, `projects`, `project_events`, `tasks`, `task_events` | Repository clock, Wave plans, Work identity, corrections, historical evidence |
| Execution | `flow_invocations`, `runs`, `sessions` | Saved execution, indexed launch records, conversations |
| Task delivery | `task_prs`, `task_pr_repair_incidents`, `task_linear_observations`, `task_linear_ingested_comments` | Serial PR chain and provider observations |
| Work adjuncts | `tool_responses`, `work_placements` | Tool answers and Home placement |
| Ask | `ask_exchanges`, `ask_linear_comment_outbox` | Blocking requests, answering-attempt fence, typed results, Linear publication |
| PM projection | `pm_snapshots`, `observation_outbox` | Bounded Linear reads and deferred child-event delivery to the Wave |
| Metrics | `metric_instruments`, `metric_observations` | Registered producers and accepted measurements |
| PR landing | `pr_landings`, `ci_incidents` | Exact PR-head supervision and repair generations |
| Home and provider authority | `homes`, `access_profiles`, `account_access_profiles`, `provider_accounts`, `provider_account_limits`, `provider_routes`, `provider_session_accounts`, `provider_tokens`, `provider_deliveries` | Machine routes, credentials, selection, limits, delivery receipts |
| Local observation/cache | `run_events`, `blob_tokens` | Outer command events and deterministic Git-blob token counts |
| Schema | `schema_migrations` | Applied migration identity and checksum frontier |

Released migration files are immutable history. A forward migration creates the
new schema. A one-time Home import moves historical Run, Session, invocation and
chapter facts into it before ordinary readers switch. Imported records retain
identity and evidence; ambiguous mappings remain explicit migration findings.
Runtime reads never fall back to historical files. Retired files are removed
only after verified import and a recoverable backup.

### Filesystem state

| Location | Contents | Write pattern |
| --- | --- | --- |
| `.lf/skills/`, `.lf/flows/`, `.lf/config.yaml` | Repository-owned execution definitions | Authored and reviewed with code |
| `wave/<name>/GOAL.md`, `MEMORY.md`, `metrics/` | Wave intent, curated memory, metric contracts | Authored and reviewed with code |
| `.lf/journal/waves/<name>/journal.jsonl` | Wave conversation/resident events | Append-only with crash-tail repair |
| `.lf/releases/<tag>/<commit>-<run>/` | Prepared release bytes and their candidate receipt | Replace one exact candidate atomically; retain through retry, remove after publication |
| `$LF_HOME/runs/<prefix>/<run-id>/` | Run manifest, event streams, terminal receipt | Publish once, append streams, settle once |
| Home provider directories | Provider-native login and resume state | Owned by provider adapters |
| Git directory `loopflow/` receipts | writer/rebase/PR mutation coordination | Kernel-locked receipt files |
| machine install root | Versioned artifact sets and switch receipts | Stage immutably, select atomically |

### External systems

Linear owns Initiative/Project/Issue planning shared with the team. GitHub owns
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

Task CLI -> exact Task advancement claim -> one worker boundary Run
Wave operation -> one finite wave/operate Run
Task/Wave-bound helper Runs ---------> shared Skill execution components
```

| Surface | Responsibility | Scope |
| --- | --- | --- |
| `lf <skill>` and `lf flow` | Direct Skill execution and Flow composition | Current process and Home |
| `lf wave`, `project`, `task`, `work` | Durable planning and Work coordination | Work resolved in the current planning store |
| `lf ask`, `session` | Sessions and explicit resolution | Current Home Session rows joined to Runs |
| `lf wt`, `commit`, `rebase`, `pr`, `ci` | Worktree and delivery operations | Exact repository/Task/GitHub object |
| `lf runs`, `usage`, `ps`, `top`, `prune`, `doctor` | Execution and process observation | Current Home only |
| `lf home`, `lf work`, `start`, `stop`, `pause`, `resume` | Home identity and Wave service lifecycle | Current Home unless routed explicitly |
| `lf ssh <home-id> <args...>` | Run the target Home's `lf` | Explicit remote Home; no implicit fan-out |
| `lfd` HTTP API | Start/stop/reconcile Waves, receive webhooks, claim landings | One Home |
| Wave HTTP API | Channel, conversation, events, messages, observations, resident attachment | One Wave listener |
| Loopflow.app | Swift projections and user interaction | Queries the same DTOs and remote routes; owns no lifecycle |

Most commands are local by default. `lf ssh` is transport, not a second API:
the inner `lf` and separator are implicit, the target re-resolves its own Home
state, and durable processes scrub foreground-forwarded secrets before
detaching.

## Harness launch and Run records

Every Loopflow-mediated launch uses the same Run constructor and recorder.
Task, Wave, Ask and ordinary CLI callers supply typed context, not different
persistence formats. Project-scope planning Runs carry their Wave; Project is
not an extra Run parent.

```text
Home SQLite: sessions -> runs (Run.session_id is null without a conversation)
             sessions.current_run_id -> one of that Session's Runs
$LF_HOME/runs/<prefix>/<run-id>/
  manifest.json       immutable launch inputs, prompt/context references
  context.json        captured prompt context when present
  events.jsonl        append-only provider and usage evidence
  terminal.json       immutable terminal evidence
```

1. Resolve the Home once for the store and artifact root. Resolve optional
   Work selectors without granting execution authority; validate Run ancestry.
2. Reserve the Run and create or reuse its Session in one transaction, in
   prepared state. Set the Session's current Run and bind the review boundary's
   exact Run in the same transaction. Headless Runs need no Session.
3. Publish final launch inputs atomically in its artifact directory. Mark the
   Run launchable only after publication. No provider starts before both exist.
4. Spawn and record the exact native provider/attachment information in the
   store. Append telemetry without putting event writes on the critical path.
5. Publish terminal evidence and settle the Run row. A recovery pass can finish
   this interrupted settlement from that exact receipt, never from elapsed time.

Prepared rows without launch artifacts remain recoverable preparation failures.
Publication without a spawn receipt does not authorize a blind second spawn;
recovery uses the existing exact process evidence or reports uncertainty.
Completion of a provider Run does not close its Session. Session readiness and
completion have their own transaction boundary.

The store owns current Work attribution. Launch artifacts retain original
inputs and cannot answer current ancestry after bind. Renaming/binding never
rewrites them. Native provider IDs, client attachment and stop receipts live
with the Run's operational state in SQLite, not mutable files beside it. These
receipts describe exact processes; a stored `active` label alone is not liveness.

Run listing filters indexed row fields before its result limit. Detailed output,
replay and usage may read the selected Run's evidence files. Usage attribution
joins current Run parents, so it follows bind; provider counters remain original
observations. Retry/failover attempts stay within one Run and use distinct usage
streams. Missing counters remain unknown; settlement never invents provider
finality or sums cumulative checkpoints.

Run/Session storage is required locally. No live PM request, Wave listener or
daemon is required for an unbound launch. Database failures are reported before
spawn rather than manufacturing an unindexed file-only Run.

## Tracked Work and bounded Task advancement

```text
Task trigger -> claim current invocation -> execute captured node
                                         -> settle, stop, or await Session
```

A Task's current root invocation owns its runtime child invocations. Claims
fence invocation ID, cursor version, worker generation and the exact Run. A
late result cannot advance a replacement invocation even if numeric versions
repeat. Settlement validates the original successful Run before consuming a
navigation candidate, then atomically updates the cursor and domain evidence.

Recovery uses the saved graph, including unchosen Xor branches and human policy,
after source files change or disappear. Missing process evidence is uncertainty,
not proof of death. Interruption retains execution history; restart closes the
previous invocation and creates another. Finishing retains the invocation with
terminal status and clears the Task's current root, without completing Work or
selecting a successor Flow.

Independent helpers may carry the same Task and use separately authorized
Git/PR operations. Their Runs have no invocation membership and cannot settle
its cursor. Wave service availability is not a prerequisite for Task progress.

Work status remains `Ready | Done | Abandoned`. Binding an independent Run to
terminal Work is attribution, not a reopen or a new worker claim. Unresolved
Sessions remain visible even when their associated Work is terminal.

Chapter rotation shares the Task write boundary for transfers and retirement.
It preserves active invocation, claim, Run, Session, PR and worktree identity
when moving started unfinished Tasks to a successor Project in the same Wave.
The repository's current Chapter changes atomically after every participating
Wave's plan is prepared; external provider updates are resumable operations,
not a distributed SQL transaction.

## Durable communication

Task steering posts Linear comments. The Task's advancement claim identifies
the live recipient. Independent attributed Runs do not receive steering.
Workers refresh comments into local delivery events and starting context;
publication, seed inclusion, and provider acceptance are distinct evidence.
Steering an idle Task starts nothing. Wave guidance travels as extra
instructions to `wave/operate`.

```bash
lf task steer INF-123 "keep the public name"
```

### Questions and sessions

```bash
lf ask "Review this migration with me"
lf session list --task INF-123 --json
lf session open <session-id> --json
lf session rename <session-id> "Migration review"
lf session bind <session-id> --task INF-123 --json
lf session ready "Ready for review"
lf session complete <session-id>
```

All three Session kinds are rows selected by the same query. Session identity
is independent of Run identity. Flow review preparation creates its Run and Session before
provider launch; an Ask creates a child Run with the caller's Task/Wave and a
Session holding the request. Standalone interactive launches use the same rows.
There is no concatenation of file scans and runtime boundaries to build a list.

Open resumes native history on the same Run. Retrying an unpublished launch
keeps that reserved identity. If an unrecoverable published launch requires a
replacement, the old Run remains in the Session's history. A new Run belongs to
the same Session and becomes its current Run. An exact boundary transaction
updates the pointer and pending attempt together, retaining the relationship to
the previous attempt. Late results from the old Run cannot settle the new one.
Session ID, name and feedback stay on the same row; no name copy is needed.

Rename updates the title and provenance atomically; an agent's generated title
cannot overwrite a human title. Bind delegates to the current Run's ancestry validator.
The same operation and Rust-owned availability reason serve every UI surface.

Ready saves the summary and leaves the Session open. Complete persists the
closed state and final feedback before provider teardown or continuation.
An Ask returns that saved result to its waiting caller, including keyed retries.
A Flow review supplies feedback to the next step; the following deciding Run
chooses an edge. Provider exit, pane close and readiness do not complete reviews.
Completed rows remain history and default lists select open rows.

The terminal pool belongs to the desktop window. Bind and rename retain the
pane, provider client, scrollback and draft. A detached PTY cradle can host a
client before the UI arrives; it owns neither Session identity nor completion.

## Flow execution

```bash
lf task run INF-123                  # invoke the Project's Flow
lf task run INF-124 --flow incident  # explicit template override
lf task advance INF-123             # drive the saved invocation
lf task restart INF-123 --flow feature
```

A Flow is an authored template. Expansion resolves composed sub-Flows and
captures every Skill, router and Xor alternative before execution. An invocation
stores that fully expanded graph plus cursor, return counts and claims in one
row. Changing the Project's Flow affects future invocations only. Templates may
be displayed folded; the running invocation exposes its expanded nodes.
Fully expanded means no unresolved template references; backward edges and
unchosen Xor alternatives remain finite graph structure. It does not mean
preallocating an unbounded number of future loop passes.

Flow invocations can run without a Task. `lf flow <name>` creates a taskless
invocation when no Task is selected or inferred; Task launch uses the selected
Task. Both use the same invocation records and execution machinery, including
captured recovery and human boundaries. Taskless execution needs no synthetic
Task, Project, or Wave. Its Runs may carry Wave-only attribution or none.

Nodes have local typed IDs. Template composition does not create invocation
parents. Runtime entry into a nested loop body creates a child invocation for
that pass, linked to the parent's entry node. Child and parent belong to the
same nullable Task. The parent records which child it awaits; child completion and parent
resumption settle together. Retrying reuses the same child; a new pass creates
a new child. There is no path-string node identity or separate occurrence row.

Each invocation keeps local loop return counts. A Run captures the iteration
tuple along its invocation's ancestry at launch; later cursor movement cannot
change that tuple. The constructor validates it against the locked invocation
chain. An execution location is invocation ID, node ID and tuple; it is a value,
not an independently stored object.

Backward edges name earlier nodes in the captured graph. Counts measure progress
without imposing a pass budget. `loop-decide` records Advance or Iterate under
its exact Run authority. A candidate becomes effective only after that Run
succeeds; failed/interrupted Runs cannot decide. Mechanical operation retries
require checking prior effects; cursor recovery alone cannot prove exactly-once
external effects.

`lf flow blocked` opens a keyed Ask for the decision boundary. Completing it
returns evidence for reassessment and never chooses a verdict. Human nodes use
`flow_review` Sessions. Their saved feedback survives restart, source deletion
and repeated completion calls. Claims and pending review links settle under the
same invocation transaction; there is no file-backed cursor adapter.

The Wave scheduler continues to run finite `wave/operate` Runs. Wave journal
history and process ownership remain separate. Historical Wave continuations
are preserved recovery evidence; inspect with `lf wave recover <name>` and
explicitly disposition unresolved work. Missing termination evidence never
permits replacement of an unknown active attempt.

## Task delivery algorithm

A Task binds planning to one active remote branch, managed Git worktree, and
PR. Any checkout tracking that origin branch identifies the Task; the stored
path is placement, not identity. The current delivery implementation can rotate
a settled Task onto a later serial branch. Once that happens, the old branch no
longer identifies the Task. Collapsing the Task lifetime to one Linear-associated
branch remains a separate delivery simplification.

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
                              complete Task
```

1. `lf task prepare` resolves one Linear Issue inside one Project and creates
   or reuses Task Work, its worktree, and its serial PR identity. It starts no
   boundary Run.
2. Independent `lf --task ...` Runs may work in that substrate directly.
   `lf task run` selects a Flow when none is active and ensures its exact
   invocation has one worker through the same execution components.
3. `lf commit` snapshots the worktree. `lf pr publish` creates or refreshes the
   current PR without opening a browser.
4. `lf pr submit` leaves the exact-head merge click to a person. `lf pr arm`
   requests exact-head auto-merge and returns; `lf pr land` watches through
   merge. All three operate on Task delivery state when it exists and require
   no Flow-driving claim or execution receipt.
5. PR landing is fenced by landing generation. The supervisor repairs current
   failing checks; a moved head requires fresh evidence. An unchanged head may
   pass or merge, and explicit land resumes a previously blocked operation.
6. Merge completes the Task. Follow-up or simultaneously dependent work uses a
   separate Task, optionally stacked on the parent's PR.

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
          Task CLI ----- exact invocation claim
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
2. `lf work place` records where a Wave/Task belongs.
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

An already-running `lf` retains its executable and selected store path.
A Run-schema cutover must account for those writers before activation. On the first published-to-development switch, the process may keep
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
| [`PmSnapshotRow`](../rust/loopflow/src/store/mod.rs) / `pm_snapshots` | Linear planning | Atomic PM sync replacement; `lf status`, `lf roadmap`, and the Mac app read it but never author through it. |
| [`TaskLinearObservation`](../rust/loopflow/src/work/task/mod.rs) / `task_linear_observations` | Linear Issue state | Reconciliation records provider evidence before applying lifecycle changes. |
| [`GithubObservation`](../rust/loopflow/src/work/task/mod.rs) / `task_prs`, `ci_incidents` | GitHub PR/check state | Webhook or foreground reads update Task delivery evidence; GitHub remains merge truth. |
| `tests/fixtures/dto/` | Rust `lf --json` DTOs | Rust and Swift fixture tests reject required-field or enum drift. |
| `tests/fixtures/migrations/` | Ordinal-free migration drafts and the Python canonicalizer | Rust build/runtime and Python release tests reject ordering, body-byte, checksum, and graph-error drift. |
<!-- architecture-projections:end -->

`lf status` and `lf roadmap` derive Task conditions from Work, invocation and
PR facts. Sessions are one query joining Session and Run rows; all Run filters
use those same ancestry fields. No projection acquires launch, Work-mutation,
credential or signal authority. UI grouping is cached presentation, not another
attribution store.

## Extension rules

| Area | Safe extension | Architectural constraint |
| --- | --- | --- |
| Run queries | Add an index or projection for a measured query | The Run row owns identity and attribution; files hold launch/provider evidence, never fallback ancestry |
| Multi-Home views | Fan out read-only commands through `lf ssh` | Do not centralize Run ownership or silently mix local and remote scope |
| Process control | Publish birth-validated ownership at the launcher spawn seam | No PID/tmux/Work/telemetry inference |
| Planning input | Add a naturally keyed fact or provider observation | Do not create a global input revision protocol |
| Provider support | Add a provider adapter, account route, and normalized stream mapping | Provider credentials/finality remain provider-authored |
| Promotion | Add artifact roles or service adapters within the locked switch transaction | Artifact activation does not depend on Run discovery |

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
| `shim:legacy-chat-import` | Old journal turns become one immutable Wave conversation epoch. | [`ConversationEpochImport`](../rust/loopflow/src/controller/wave/journal.rs); remove only when old journals are no longer supported. |
| `shim:retired-op` / `lf op` | Rejected namespace returns the surviving top-level command name. | [`Commands`](../rust/loopflow/src/lf/mod.rs); remove when external callers no longer need the diagnostic tombstone. |
| `shim:rams-alias` | Installed `rams/rams` command resolves to the Skill model. | [`SkillSource`](../rust/loopflow/src/lf/discovery.rs); remove when the external single-file command is no longer supported. |
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
| `Project Session`, `Task Session`, `project_sessions`, `task_sessions` | `rust/loopflow/src/store/migrations/`, `rust/loopflow/src/store/migrations.rs`, `rust/loopflow/src/store/tests/fixtures/`, `release/` | Stable Project/Task **Work**; conversational Session owns Runs and a current Run; it is not a Work executor. |
| `session context`, `LF_SESSION` | — | Stable Work identity plus `LF_RUN_ID`/`LF_RUN_DIR` execution evidence. |
| `lf radio`, `agent bus` | `release/` | Typed Work observations, Steer, synchronous questions, and review FlowSteps. |
| `pm.linear_project`, `projects/<slug>.md` | `release/` | `pm.linear_initiative`; Linear Initiative → Project → Issue. |
| `machine-local host`, `machine-global command`, `machine-global mutation`, `machine-global reservation` | — | Home-local keeper, command, mutation, or reservation. |
<!-- architecture-vocabulary:end -->

The following storage vocabulary names retired representations, not alternate
read paths. Their bytes survive only as migration input or historical evidence.

| Historical representation | Current owner |
| --- | --- |
| Wave-scoped Chapter / `wave_chapters` | Repository Chapter and unique (Wave, Chapter) Project |
| Recommended Flow / `flows.recommended` | Project's `flow` template selection |
| `FlowPosition`, `PinnedTaskFlow`, `task_flow_positions` | Flow invocation row: captured graph and execution state together |
| `FlowRun`, `flows/<id>/position.json` | Flow invocation row |
| Subject selector list on a Run | Typed Run parents and `work_source` |
| Four Session projections, Ask files, composite boundary Session IDs | `sessions`, keyed by Session ID, with a current Run and Run history |
| Session name/resolution and provider attachment sidecars | Session attributes and Run operational data in SQLite |
| Step occurrence / path-string node key | Invocation ID, local node ID, captured iteration tuple |

Canonical migrations, migration fixtures, and release notes retain historical
names because changing shipped evidence would rewrite history. Operational docs
and current runtime source do not. Chapter archives under `.lf/chapters/` are
dated evidence, excluded from live vocabulary and compatibility-seam discovery.

## Authority and failure invariants

- Repository Chapter × Wave identifies one Project; Task belongs to that
  Project. No recursive or orphan Projects.
- A Wave UUID is stable across rename and repository rehome; repository-scoped
  locators are unique, and bare slugs are never mutation authority.
- Linear owns current Project/Task planning; SQLite projections never become an
  authoring fallback.
- Wave operation reads its Project in the repository's current Chapter.
  Rotation changes the repository boundary together; partial external work is
  visible and resumable.
- An ad-hoc Skill Run needs no planning parent or PM service; it still records
  a validated row in its Home's SQLite store before launch.
- Every Loopflow-mediated harness launch publishes one immutable manifest
  before spawn and creates at most one immutable terminal receipt.
- JSONL telemetry is best effort and never gates launch or settlement.
- Run parentage, Work attribution, outcome, and usage are evidence only.
- Work status describes planning convergence; process and Run activity are
  separate observations. One Work may concern zero, one, or many Runs.
- Generic Run ids stored in Steer and Task history are opaque provenance unless
  a reader independently resolves their Run record; resolution never grants
  authority.
- Cross-process signaling requires an exact process ownership receipt and
  current OS birth identity; a Run or Session row alone is not that receipt.
- OS file-lock ownership is scoped to its documented local critical section;
  it never grants Work or credential authority.
- Multiple independent agent writers may coexist. A live rebase excludes them;
  only its exact recovery child may enter that sequencer.
- A Flow invocation or an Ask caller may await a Session's explicit completion.
- Agent readiness and process exit are never resolutions. Complete releases an
  Ask or returns review feedback to the next Flow step. A deciding Run
  records Advance/Iterate under its own exact Run authority.
- Another Work perspective is an ordinary `lf --as` Run; Steer appends durable
  correction.
- Terminal Work does not hide or complete its unresolved Sessions.
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
