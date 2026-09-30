---
layout: default
title: Architecture Reference
---

# Architecture Reference

This page owns the execution contract, its cutover status and the checked
current-source inventory. [Architecture](architecture.md) introduces the model;
[Execution](architecture/execution.md) and [Data](architecture/data.md) develop it.

## Cutover status

**Accepted contract, implementation in progress.** Exec is one actual lf process;
AgentSession is a continuable interactive or headless conversation; FlowSession
is a captured resumable Flow consuming exact boundary completions. Run has no
separate product lifetime in this contract. Jack requested the docs-first spec
before completing the remaining owner conversion.

The implementation currently has `execs`, `agent_sessions` and `flow_sessions`,
with immutable input evidence, capture files and transitional Run-named readers.
The old `runs` and `agent_session_inputs` tables are removed; unclassified
historical SQL remains in immutable `import_evidence`, without invented Sessions. Renaming Session/invocation tables did not complete the conversion. The checked inventory
below describes those current source dependencies, including historical wire
names; it must remain accurate until their consumers are removed together.
Other architecture prose describes the target contract, not a shipment claim.

Current CLI examples use supported spellings. `lf session connect` has `open` as
an alias; `--replace` replaces an owned client. The explicit conversation/engine
restart contract below has no public `--restart` flag yet. `session list` supports
`--interactive false`, `--history`, `--task` and `--search`; `--all` means all
repositories. `--page --json` uses stable ID pages; Desktop retains earlier
observations until enumeration succeeds. `lf flow list/show --sessions` discovers
saved FlowSessions through metadata and exact captured graphs. Historical `lf runs` and `lf replay` remain command spellings.
History exposes Session event and provider evidence; `RunSnapshot` and Session
`run_id` are removed across Rust, Swift and fixtures. Retaining these names in a command or
wire reference does not make Run a target owner or authorize breaking external
consumers without their coordinated migration.

### Spec to remaining implementation

| Contract | Current evidence and next dependency |
| --- | --- |
| One Exec per actual lf process | Ordinary commands record process ancestry and outcome. Help, rejected arguments, screenshot and installation entry use an existing compatible ledger without initialization; unavailable storage remains an explicit gap. Installed startup acceptance and final discovery/wire coverage remain open. |
| Stable AgentSession with separate driver and engine | Admission/publication and Session readers use AgentSession directly; ordinary automatic retry retains both outcomes with zero Run rows in the native fixture. Complete recorded account/native Home, public restart, stale-client exclusion and shared-engine preservation across every provider path. |
| Flow consumes exact successful native history | Selection/publication and exact consumption use Session/Exec/Flow history. Mechanical operations use Flow history. Structured native results close the earlier Codex decision-retry transport failure in the real-Codex/synthetic-Responses fixture; configured-provider acceptance remains unproven. OpenCode batch and managed launches share native user-message selection/history and permission ordering; its public retry fixture preserves earlier-caller rejection. All-provider recovery and final retired-owner deletion remain required. |
| One started Flow is one FlowSession | Jack Heart's 2026-09-30 decision replaces runtime child-pass Sessions with node/iteration positions. Current source still stores child passes. Fold their history and current selection into the root by forward migration, then remove child claims, parent links and root/deepest-child indirection while preserving retry and next-pass behavior. |
| Complete recovery | Six standalone native fixtures use real Codex with synthetic Responses/private Homes, including driver/engine loss and automatic retry. Managed dispatch has synthetic successor-history proof; configured managed provider/account continuity remains unproven. |
| Lossless import and final owner deletion | The offline importer exists but complete four-origin/headless/command import, repeated attempts, conflict/interruption preservation and final Run removal remain required. Ordinary reads must not import or reconstruct identity from old files. |
| Indexed discovery and usage | Runs/usage/telemetry/activity and landing conclusions select AgentSession input history before payload decoding, preserving per-input attribution and windows. Native receipts without a captured input/start remain discoverable with unknown ownership and partial usage coverage. Complete full historical import and dense cold/warm measurements. History uses captured event sequences and exact native references. |
| Desktop and wire agreement | Off-roadmap ancestry and pane/draft retention pass unit and mounted native-terminal fixtures. Graph and membership wire use captured numeric IDs in Rust/Swift. Complete coordinated history/usage conversion and configured Desktop proof remain outstanding. |
| Status-owned Chapters | Rotation/default-Flow implementation exists with focused fixtures. Complete historical adoption, partial/competing-plan preservation and second-Home proof. No Chapter table or packet belongs in the model. |
| Integrated acceptance | Affected checks, configured provider/Desktop, backed-up real-Home import and final consistency remain required. Branch fixture passes are neither installed acceptance nor permission to promote. |

The admission checkpoint preserves saved executable/Home/database handoff and
launch-failure diagnostics. Populated canonical migration, Ask/review recovery,
refusal before provider launch and automatic-retry fixtures cover their stated
boundaries. The forward migration removes `runs` after retaining every original
SQL row as immutable input evidence, including unknown conversation attachment.
Import reports unresolved inputs without inventing conversations or processes.
History lists and native thread/account lookup read Session observations. Fresh native publications outrank imported legacy sidecars;
JSONL fallback uses original input order. Import classification reads its parsed
source before writing SQL. Saved Ask/review continuation, input-prefix selection and
active process metadata now use that store without reopening manifests. Exact
PID/start receipts still establish process identity; input history and current
Session Work remain separate. Initial prepared launch and process receipts retain
file dependencies. Native-only usage and complete historical import still need
their final proof. Configured-provider recovery and the remaining import/owner-removal gaps prevent code-complete acceptance. Detailed
receipts and unresolved import obligations remain with the active Task; no
full-green hosted, configured-provider or installed-migration result is asserted.

## The system grows outward from a direct Skill launch

The direct Skill launch is the kernel of Loopflow. It remains useful with
no Wave, Project, Task, or daemon. Agent admission requires its Home's writable store. Higher layers
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
                        | HomeId / lf ssh          |
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
  explicit `lf ssh`.
- **Surfaces:** derive CLI and Mac views from planning facts, provider truth,
  local process observation, and command/conversation history.

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
 Wave -> Linear Project -> Task          Exec / AgentSession / FlowSession
       stable Work           indexed history + payloads
          |
          v
 repository + Git worktrees

exact local races use OS locks; remote execution uses lf ssh
```

- **Tracked Work** records purpose, input, convergence, and the exact next Flow
  boundary; the Flow claim and selected completion fence advancement.
- **Execution evidence** keeps command outcomes, provider turns and Flow results distinct.
- **Delivery** coordinates worktrees, commits, PRs, CI, and merge.
- **Machine authority** places Work and scopes local credentials, processes,
  files, and locks to a Home.

An identifier in one area is not authority in another. The detailed boundaries
below are the architecture's central constraint.

## Planning and execution vocabulary

These are the accepted product owners; see [cutover status](#cutover-status)
for the current implementation boundary.

```text
Wave --< Linear Project --< Task --< FlowSession
                             `-- managed_flow_session

Exec --< Exec                 one row per actual lf process, causal edges
AgentSession --> Exec         nullable current driver, with generation fence
AgentSession --< history      immutable provider outcomes and usage
FlowSession --< history       mechanical results or agent completion references
```

| Concept | Owns |
| --- | --- |
| Repository | Wave membership and repository-wide Project rotation |
| Chapter | No stored object: the shared name of each Wave's In Progress Project |
| Wave | Enduring objective, memory, cadence, budget and metric instruments |
| Project | Linear status, Tasks, KRs, targets and the default Flow |
| Task | Worktree, serial PRs, attributed FlowSessions and one managed selection |
| Flow | Reusable authored graph of agent/mechanical/router/review nodes |
| Exec | One lf process's immutable causal ancestry and command completion |
| AgentSession | Conversation identity, title, feedback, native thread and provider history |
| FlowSession | Captured graph, cursor, return counts, claim, review and completion |
| Home | Store, payloads, credentials and exact local process authority |
| Placement | Where Work executes; no authority over merely observed processes |
| Steer | Ordered authored correction to Work |

## Core models and APIs

Constructors and mutation APIs validate within the writing transaction. DTOs
carry typed ancestry; CLI names and issue identifiers resolve at the boundary.
There is no replacement Request, Execution, SkillInvocation, AgentExec or generic
attempt object. History entries have stable references, not independent lifecycles.

| Owner | Authoritative fields and operations |
| --- | --- |
| `execs` | ID, immutable parent Exec, incoming direct/agent bit and calling AgentSession/provider generation when known; command, cwd, start/end and observed outcome/exit/signal; admit, finish, filter/page |
| `agent_sessions` | Stable ID, purpose and independent interactive flag; title/provenance, request/feedback, typed Task/Wave/Flow ancestry, native identity, nullable driver Exec and separate driver/provider generations; reserve, connect, restart, bind, rename, ready, complete |
| `flow_sessions` | Nullable Task/Wave, captured graph and launch context, cursor/return counts, claim/version/generation, selected conversation/completion reference, pending review and status; capture, claim, checkpoint, settle, recover |
| `tasks` | Project, issue, durable disposition, worktree/delivery facts, managed FlowSession selection and set-once `started_at` |
| `projects` | Wave, stable Linear Project identity, status, shared chapter name, Flow and planning facts |
| `waves` | Stable repository identity; authored objective/memory/instruments stay in repository files |

### Conversation and driver lifetime

Every agent conversation is an AgentSession: skills, inline prompts, helpers,
Asks, reviews, interactive and headless work. Default views select interactive
conversations. Explicit filters expose headless and completed history; `--all`
continues to mean all repositories. Interactive mode grants neither review
completion nor Flow authority.

An AgentSession can have many historical driving Execs and at most one current
driver. Driver compare-and-set increments its driver generation. A continuing
engine keeps its provider generation and origin through handoff and a driverless
interval. Old clients may display events but cannot start/steer turns or write
Session state. Passive connection acquires no claim. Dispatch fences include
queued native RPCs and approval replies, not only database claim updates.

Explicit restart replaces the exact conversation owner and preserves the native
conversation and history. Graceful thread-specific stop precedes force termination
of an exclusively owned process. A shared engine must survive another thread's
restart. PID/start identity and native endpoint are operational evidence;
conversation identity, causality and elapsed time grant no signal authority.

AgentSession history records provider starts, successful/failed/interrupted
outcomes, retries, durations and usage, correlated with driving Exec and native
turn/receipt. Repeated receipts are idempotent. Missing measurements differ from
zero; cumulative samples are not added together. Later continuation never rewrites
an earlier completion already consumed by a Flow. Provider completion and Exec
completion are distinct: subsequent command work may fail after the agent succeeds.

### Exec lifetime and causality

One actual lf process gets one Exec, including nested direct and agent-issued
commands. In-process wrappers and Flow steps reuse it and cannot settle it early.
The incoming `via_agent` bit describes the caller, not whether the command later
launches an agent. Agent-issued children resolve stable Session/provider-generation
provenance to the current matching driver once, at admission. Delayed children
of a replaced provider retain their historical origin; existing parents never change.

Exec outcomes are succeeded, failed or interrupted when observed. Unobserved
completion, exit code and signal stay unknown. The outer command owns terminal
settlement, including interruption. Bootstrap/installation logging must not open
or migrate an incompatible installed store before its authority preflight.
Help, parser errors and installation/screenshot entry paths append process evidence
only to an existing compatible ledger, without creating a Home, seeding branch
data or applying migrations. An attached ledger stays pinned through command
completion. Missing or incompatible storage leaves an explicit recording gap;
inspection and screenshot cleanup still work. Agent admission requires its Exec
and conversation rows before provider launch, even when earlier observation failed.

### Flow outcomes and authority

A FlowSession is the existing invocation owner evolved, not a parallel cursor.
Task and taskless execution share it. A Task selects one managed FlowSession
without excluding other attributed Flows. Template composition expands the graph.
One started Flow is one FlowSession; loop passes are node/iteration positions and
lenses over its history. They have no separate claim or lifecycle. Captured
definitions survive source deletion or edits.

A boundary is the Session, node and iteration tuple. It may fail or be interrupted
several times before succeeding. Preserve each outcome in its owning history.
Agent steps consume an exact successful AgentSession history entry; mechanical
steps retain a correlated start/result in FlowSession history under their own
child `lf` Exec. The driver can fail after an earlier step succeeded; the step's
result stays intact. Mechanical steps create no AgentSession. Resume waits for a
surviving step and consumes its saved result without executing it again.

Settlement validates the FlowSession identity, version, claim, boundary and
selected completion. Record completion and consumption atomically when they share
the store. Old successes, stale drivers and helper conversations cannot advance
the current boundary. Human feedback is saved before teardown and consumed once;
readiness or provider exit never chooses a navigation edge. A conversation driver
handoff does not itself transfer the separate Flow orchestration claim.

### Attribution, binding and Started

Task implies Wave. Constructors fill omitted ancestors and reject disagreements.
A Flow-owned conversation shares the FlowSession's nullable Task. `work_source`
is declared, checkout, inherited or bound when known. Historical unknown membership
stays Unknown; absence of evidence does not become Independent.

Bind is write-once: null to a Task, preserving any existing Wave. Same-target bind
is a no-op; reassignment and clearing are unavailable. Done/landed Tasks remain
valid without reopening them. CLI states the permanent target and writes the assignment; Desktop confirms
the exact target before writing. CLI adds no confirmation prompt. The writer compares the selected
conversation/driver and ancestry atomically. Binding cannot alter Flow membership
or bind one member of a taskless Flow inconsistently with its owner.

Jack Heart's 2026-09-30 decision retains prospective usage
attribution: bind records assignment time; earlier usage retains its owner.
The history reader owns this single choice; a separate Intelligence Task may
re-evaluate it. Preserve active-turn start/assignment evidence; unknown allocation remains unknown rather than inventing a token split.
Authorized Project/Task moves preserve immutable historical attribution while
validating current ancestry.

`tasks.started_at` is set once when actual agent work or a mechanical Flow boundary
first belongs to the Task, including first bind. General command observation
never starts a Task. Preserve existing Started timestamps and supporting history
on import; imported inferred times are labeled. No later launch or bind moves or
clears the timestamp. Chapter retirement also checks authored work, PRs and active
claims: absent execution evidence alone cannot prove untouched backlog.

### Readers and import

One indexed reader per object selects and pages identity, ancestry, command,
skill, title and status before opening payload files. No inventory scans manifests,
sidecars or live PRs to reconstruct identity. Bound conversations remain bound even
when absent from the visible roadmap. Desktop panes key on AgentSession identity,
so bind, rename and driver replacement retain the surface and draft.

One-time import preserves four conversational origins, headless history, command
journal evidence, captures, feedback, old IDs, repeated failures/successes and
unknown facts. It never invents an Exec from a Run without process evidence or
merges unrelated conversations by title/path/provider. Conflict/interruption
recovery and idempotence must preserve the source evidence. Ordinary reads neither
import nor fall back to old files. Runtime Run ownership disappears after migration.

### Chapters

Linear statuses are the owner: one In Progress Project per Wave, sharing a chapter
name across the repository. Planned Projects express future plans; Completed
Projects retain history. Project `flow:` is required and supplies new Tasks' default.
There is no Chapter row, packet or local switch. Rotation uses an explicit target
and stable Project identities, converges after partial mutations, preserves active
Task identity/worktree/PR/FlowSession, and refuses unrelated competing plans.
A second Home adopts the same state through ordinary synchronization.

Adjacent APIs keep their own authority: Task PR operations own Git/GitHub;
provider routing owns credentials; Home placement owns routing; exact process
receipts and current OS evidence own signaling. None follows from causal ancestry.

## Code territory and rough size

These are historical navigation estimates from before the execution-owner conversion,
rounded to the nearest hundred; they are not a measurement of the current diff.
They include inline tests and comments; migration SQL and external test trees
are listed separately. The counts are navigation aids, not quality metrics.

| Territory | Main paths | Approx. LOC | What lives there |
| --- | --- | ---: | --- |
| CLI and presentation | `rust/loopflow/src/lf/`, `src/bin/` | 31,700 | Clap grammar, command dispatch, status/read models, terminal output |
| Operational workflows | `rust/loopflow/src/ops/` | 25,800 | Task/chapter control, sessions, PR, Git, release, metrics, PM operations |
| Prompt and process engine | `rust/loopflow/src/engine/`, `src/harness/` | 29,300 | Skill/Flow discovery, prompt assembly, provider subprocesses and streams |
| Tracked Work | `work/`, `pm/` | — | Wave/Task facts, Task delivery identity, planning/provider models |
| Boundary execution | `controller/` | — | shared Flow driver and claimed Task boundaries |
| Storage and command journal | `store/`, `journal/` | 19,700 | SQLite access, migrations engine, durable rows, outer command receipts |
| Provider authority | `provider_auth/`, `provider_account/` | 7,500 | Login, encrypted tokens, account homes, routes and leases |
| Shared root modules | top-level `src/*.rs` | 10,400 | Run records, artifact switching, repository identity, subscriptions |
| Released and draft SQL | `store/migrations/**/*.sql` | 4,900 | Immutable schema history and current draft frontier |
| Swift app production | `swift/Loopflow/`, `swift/LoopflowMac/` | 18,200 | Shared DTOs/services and macOS UI |
| External Rust/Python/Swift tests | `rust/loopflow/tests/`, `python/tests/`, `swift/LoopflowTests/` | 27,100 | Cross-module, wire, migration, CLI, and app proofs |

## Complete ownership map

The following inventory describes **current source during the cutover** and is a
checked ownership index. Transitional input projections and wire names below are deletion
dependencies, not additional target product objects.
Every top-level CLI family, live SQLite table, process entrypoint, HTTP route,
provider, and literal subprocess edge must appear exactly once.

<!-- architecture-map:start -->
| Concept | Truth and authority | Data structure | Persistence | Process owner | Public surface | External edge |
| --- | --- | --- | --- | --- | --- | --- |
| **User** — a person or external harness originating work | User-attributed actions author root input and decide effects that require user intervention. User is actor provenance, not a control credential. | [`Author`](../rust/loopflow/src/durable.rs) | Git supplies `user.name` unless personal Loopflow config overrides it; input records retain source author names. No User row; authored effects persist on the concept they change. | `lf` | `lf :`, `lf desktop`, `lf user` | `exec:open`, `exec:osascript`, `exec:pbpaste`, `exec:id` |
| **Skill** — one reusable prompt with assembled context | Repository/builtin Skill Markdown is authoritative; discovery selects one source. | [`Skill`](../rust/loopflow/src/engine/flow.rs), [`SkillSource`](../rust/loopflow/src/lf/discovery.rs) | `.lf/skills/`, builtin Skill files, installed vendor Skill directories | `lf-prompt` | `lf skill`, `lf sync-skills`, `lf list`, `lf help` (local command/definition discovery) | `exec:python3` |
| **Flow / Flow invocation** — template and one execution | Template expansion captures all Skill content and Xor paths; invocation transactions own cursor and claim settlement. | `Flow`, `FlowSession`, typed node ID | `.lf/flows/`; `flow_sessions`, `flow_events` | CLI driver or hidden `lf task __worker`; `lf __flow-step` owns each mechanical effect | `lf flow`, `lf run` (flow-first definition execution), `lf task run --flow` | — |
| **Wave** — durable operating context with goal, memory, cadence, chat, and project selection | The Wave UUID is durable identity; canonical repository plus normalized slug is its mutable readable locator. `wave/<name>/GOAL.md` and `MEMORY.md` own repository intent; the Linear Initiative owns shared planning membership. | [`Wave`](../rust/loopflow/src/work/wave/mod.rs), [`WaveLocator`](../rust/loopflow/src/work/wave/mod.rs), [`CanonicalRepo`](../rust/loopflow/src/repository.rs), [`WaveConfig`](../rust/loopflow/src/work/wave/config.rs) | `waves`; `wave/<name>/`; an in-flight relocation receipt under `.lf/tmp/wave-relocations/` | Finite Wave-attributed conversations; relocation owns the repository locator lock | `lf wave`, `lf wave list`, `lf wave status`, `lf roadmap`, `lf cron`, `lf discord` | Discord when configured |
| **Chapter / Project** — shared current plan name and one Linear Project per Wave | Linear Project status owns planned/current/completed plans; the repository chapter name is derived from its Waves' In Progress Projects. | `Project`, `ProjectStatus` | `projects`, `project_events`; Linear Project status/content | deterministic convergent rotation from fresh provider facts | `lf repo new-chapter`, `lf repo reteam` | Linear |
| **Live metric** — one reviewed measurement contract owned by exactly one Wave, plus revision-bound current evidence | `wave/<name>/metrics/*.md` owns meaning and Wave ownership; an accepted instrument observation owns its source-time fact; [`MetricPortfolioDto`](../rust/loopflow/src/work/wave/metrics.rs) is the sole derived reading shared across surfaces. Metrics inform KRs but never complete them. | [`MetricContract`](../rust/loopflow/src/work/wave/metrics.rs), [`MetricObservation`](../rust/loopflow/src/work/wave/metrics.rs), [`MetricPortfolioDto`](../rust/loopflow/src/work/wave/metrics.rs) | `wave/<name>/metrics/`, `metric_instruments`, `metric_observations` | Metric instruments write observations; foreground Rust readers derive bounded portfolios. | Status/roadmap JSON, Wave and Task prompts, the shared Swift DTO, and Mac Wave detail expose the same `metric_portfolio`. | — |
| **Task** — concrete work inside exactly one Project | The Linear Issue owns directive/status. The checked-out branch identifies the Task through its active PR; the stored worktree path is placement. Git upstream tracking does not select Task identity. One Task worker at a time owns advancement of the selected Flow; helpers and delivery commands may mutate the worktree without that claim. Git owns commits/branch state; GitHub owns PR/check/merge truth. | [`Task`](../rust/loopflow/src/work/task/mod.rs), [`TaskPr`](../rust/loopflow/src/work/task/mod.rs) | `tasks`, `task_issue_identities`, `task_deletions`, `task_events`, `task_prs`, `task_pr_repair_incidents`, `task_linear_observations`, `task_linear_ingested_comments`; Linear Issue; Git worktree | hidden `lf task __worker` drives successive claimed boundaries until a stop; foreground operations record delivery evidence | `lf task`, `lf pr`, `lf wt`, `lf rebase`, `lf commit` | Linear |
| **PR landing** — one watched attempt to merge an exact PR head | GitHub is authoritative for the PR head, required checks, and merge. One landing generation owns one supervisor, which repairs current failures until resolved; CI incidents record evidence rather than admit execution. | [`PrLanding`](../rust/loopflow/src/pr_landing.rs), [`CiIncident`](../rust/loopflow/src/work/task/mod.rs) | `pr_landings`, `ci_incidents` | The invoking local PR supervisor | `lf pr arm`, `lf pr land`, `lf ci` | `provider:github`, model provider for `ci-fix`, `exec:git`, `exec:gh` |
| **PM projection** — locally readable current planning snapshot | Linear remains authoritative; the Wave UUID keys the projection so locator changes preserve it. Sync atomically replaces the projection and reads never author through it. Confirmed native deletions suppress stale items without rewriting historical workflow outcomes. | [`PmSnapshotRow`](../rust/loopflow/src/store/mod.rs), [`PmWave`](../rust/loopflow/src/pm/mod.rs) | `pm_snapshots` | Foreground PM sync and Task polling | `lf repo`, `lf wave sync` | `provider:linear` |
| **Steer** — correction to Task advancement | Linear comment id/revision; Task identity selects its advancing worker | [`Steer`](../rust/loopflow/src/durable.rs), [`TaskEventKind`](../rust/loopflow/src/work/task/mod.rs) | Linear Task comments; local Task events cache delivery | Task worker refreshes comments and attempts live input; successor workers refresh their seed | `lf task comment`, Linear issue comments | Linear |
| **Tool response** — one idempotent response to a Work-scoped tool request | Stable Work identity plus request id names the response slot; a second, different answer is rejected. | [`ToolResponseWrite`](../rust/loopflow/src/durable.rs), [`ToolResponseReceipt`](../rust/loopflow/src/durable.rs) | `tool_responses` | Store transaction | Internal Work store API | — |
| **AgentSession** — one conversation | Session row owns name, ancestry, readiness, completion and publication. Its current capture references an immutable history event written at reservation. Earlier events retain caller and Work attribution. Complete returns saved feedback; the following typed decision chooses navigation. | `SessionRecord`, `SessionId` | `agent_sessions`, `session_events` | Native turn observation retains start/usage/completion; `lf __provider-session` records native identity; Session operations own state | `lf session`, `lf ask`, interactive `lf` | — |
| **Home / Placement / Promotion** — stable machine identity, Work placement, and artifact selection | `HomeId` is identity; SSH route is mutable. Placement is planning state and never process ownership. Promotion owns immutable artifact selection, isolated schema proof, app replacement, and rollback only. Install selects the latest published release independently of caller Git state; the laptop schedule invokes that same command. Checkout updates belong to rebase. | [`Home`](../rust/loopflow/src/durable.rs), [`Placement`](../rust/loopflow/src/durable.rs), [`SwitchReceipt`](../rust/loopflow/src/machine_install.rs), [`published installation`](../rust/loopflow/src/lf/commands/install/published.rs) | `homes`, `work_placements`; Home-local SQLite; machine install selection and switch receipts; laptop refresh LaunchAgent | The promotion command owns its OS-locked switch transaction | `lf home`, `lf ssh`, `lf install`, `lf install schedule` | `exec:ssh`, `exec:launchctl`, `exec:systemctl`, `exec:/usr/bin/open`, `exec:/usr/bin/osascript`, `exec:brew`, `exec:/bin/sh`, `exec:tmux` |
| **Session history projections** — captured events and exact provider evidence | AgentSession and FlowSession own outcomes; immutable import evidence retains historical SQL with unknown membership explicitly. Original payload and exact process receipts confer no Flow authority. | `RunSpec`, `RunManifest`, `SessionHistory`, `ProviderHistory`, `SessionUsage` | Projects AgentSession-owned input/history; Home-local `runs/<prefix>/<run-id>/` immutable payload and process receipts | shared conversation admission and history | `lf runs`, `lf replay`, `lf usage`, `lf activity`; Work/status history | `exec:lf`, provider harnesses |
| **Browser capture** — one isolated, bounded screenshot transaction | The requested source, viewport, and output name the transaction; only a validated PNG replaces the output. The standalone shell identity and fresh process group keep capture separate from the user's browser and bound to its owner. | [`ScreenshotArgs`](../rust/loopflow/src/lf/mod.rs), [`ProcessGroupGuard`](../rust/loopflow/src/engine/process.rs) | Output PNG only; no control-store state | `lf __screenshot-supervisor` owns one `chrome-headless-shell` process group and observes the public command through a control pipe | `lf screenshot` | `exec:chrome-headless-shell` |
| **Exec** — one actual lf process | The journal transaction records command completion and fixes each child's causal parent at admission. Agent provenance grants no control authority. | [`ExecId`](../rust/loopflow/src/id.rs), [`AgentCaller`](../rust/loopflow/src/exec.rs) | `execs`, `run_events` | Outermost foreground command; installation/bootstrap coverage remains a cutover obligation | `lf exec`; ordinary parsed CLI commands | — |
| **Local process observation** — outer command receipts joined to current OS facts | A live kernel process plus a matching local receipt is observation, not durable ownership. Registered orphan OpenCode groups may be reaped; unclaimed provider PIDs may not. | [`ActivitySnapshot`](../rust/loopflow/src/lf/commands/top.rs), [`ProcessPruneReport`](../rust/loopflow/src/lf/commands/top.rs) | Home-local Exec receipts and OpenCode server registry | The foreground observer samples the process table; no keeper asserts Run liveness | `lf ps`, `lf top`, `lf prune`, `lf doctor` | `exec:/bin/ps`, `exec:ps`, `exec:lsof`, `exec:kill`, `exec:which` |
| **Provider account / route** — credential authority and ordered provider selection on one Home | Provider token/account rows and Access Profiles own routing; credentials stay in provider homes, encrypted storage, Doppler, or forwarded foreground leases. | [`Provider`](../rust/loopflow/src/provider_auth/mod.rs), [`AccessProfile`](../rust/loopflow/src/profile.rs), [`ProviderRoute`](../rust/loopflow/src/profile.rs), [`ProviderAccount`](../rust/loopflow/src/store/mod.rs) | `access_profiles`, `auth_browser_bindings`, `provider_accounts`, `provider_account_limits`, `provider_routes`, `provider_session_accounts`, `provider_tokens` | The foreground auth command owns provider login process groups and passive browser handoff; durable processes use credentials installed on their Home | `lf auth` | `provider:claude`, `provider:codex`, `provider:doppler`, `provider:opencodezen`, `exec:claude`, `exec:codex`, `exec:doppler`, `exec:opencode`, `exec:security`, `exec:secret-tool` |
| **Code-size measurement** — repository blobs measured in model tokens | Git blob identity owns content; token counts are deterministic memoized measurements, not Run usage. | [`CodeNode`](../rust/loopflow/src/lf/commands/tokens.rs), [`CodeSnapshot`](../rust/loopflow/src/lf/commands/tokens.rs) | `blob_tokens` | Foreground command only | `lf tokens` | — |
| **Schema frontier** — ordered definition of durable control storage | Released migration bytes are immutable authority; drafts join only through deterministic release materialization. | [`Migration`](../rust/loopflow/src/store/migration_catalog.rs), [`MigrationId`](../rust/loopflow/src/store/migration_catalog.rs) | `schema_migrations`; canonical and draft migration files; immutable `import_evidence` preserves original SQL and unresolved selectors through cutover | Store open validates/applies; release cut publishes | `lf release` | `exec:sh` (release hooks) |
<!-- architecture-map:end -->

The public API column covers top-level command families, not every subcommand or
Rust function. [`lf` reference](lf.md) owns argument-level detail. DTOs emitted
by `--json` are required-field projections; Rust/Swift fixture tests own their
wire parity.

## Persistence map

Loopflow deliberately uses several stores because no one store owns all truth.

```text
repository files + Git        authored goals, memory, Skills, Flows, code
Home SQLite                  planning, delivery, Exec and Session owners
immutable input storage      retained input evidence and captured payload files
provider-native homes        model credentials and resumable sessions
Linear / GitHub              shared planning and delivery truth
machine install directory    immutable binaries and switch receipts
kernel locks                 live local exclusion authority
```

### Live SQLite tables

| Owner | Tables | Purpose |
| --- | --- | --- |
| Planning | `waves`, `projects`, `project_events`, `tasks`, `task_issue_identities`, `task_deletions`, `task_events` | Linear Project statuses, Wave plans, Work identity, corrections, historical evidence |
| Execution | `flow_sessions`, `flow_events`, `agent_sessions`, `session_events` | Saved execution, selected captures and native completions, conversations and turn receipts |
| Historical import | `import_evidence` | Immutable original SQL, selectors and unknown membership retained through migration; no runtime reservation or lifecycle writer |
| CLI processes | `execs` | Indexed command lifecycle and immutable causal ancestry, written with the journal event transaction |
| Task delivery | `task_prs`, `task_pr_repair_incidents`, `task_linear_observations`, `task_linear_ingested_comments` | Serial PR chain and provider observations |
| Work adjuncts | `tool_responses`, `work_placements` | Tool answers and Home placement |
| Historical Ask | `ask_exchanges`, `ask_linear_comment_outbox` | Retained earlier exchange/publication facts; current Ask conversation state is in `agent_sessions` |
| PM projection | `pm_snapshots` | Bounded Linear reads |
| Metrics | `metric_instruments`, `metric_observations` | Registered producers and accepted measurements |
| PR landing | `pr_landings`, `ci_incidents` | Exact PR-head supervision and repair generations |
| Home and provider authority | `homes`, `access_profiles`, `auth_browser_bindings`, `provider_accounts`, `provider_account_limits`, `provider_routes`, `provider_session_accounts`, `provider_tokens` | Machine routes, credentials, selection, limits, delivery receipts |
| Local observation/cache | `run_events`, `blob_tokens` | Outer command events and deterministic Git-blob token counts |
| Schema | `schema_migrations` | Applied migration identity and checksum frontier |

Released migration files are immutable history. A forward migration creates the
new schema. A one-time Home import moves historical Run, Session, invocation and
planning facts into it before ordinary readers switch. Chapter IDs are discarded;
existing Linear Projects and their Task evidence survive. Imported records retain
identity and evidence; ambiguous mappings remain explicit migration findings.
Runtime reads never fall back to historical files. Retired files are removed
only after verified import and a recoverable backup.

### Filesystem state

| Location | Contents | Write pattern |
| --- | --- | --- |
| `.lf/skills/`, `.lf/flows/`, `.lf/config.yaml` | Repository-owned execution definitions | Authored and reviewed with code |
| `wave/<name>/GOAL.md`, `MEMORY.md`, `metrics/` | Wave intent, curated memory, metric contracts | Authored and reviewed with code |
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
 SQLite       AgentSession history

Task CLI -> managed FlowSession claim -> exact boundary completion
Wave operation -> finite planning AgentSession
Task/Wave-bound helpers -------------> shared conversation admission
```

| Surface | Responsibility | Scope |
| --- | --- | --- |
| `lf <skill>` and `lf flow` | Direct Skill execution and Flow composition | Current process and Home |
| `lf wave`, `repo`, `task` | Durable planning and Work coordination | Work resolved in the current planning store |
| `lf ask`, `session` | Sessions and explicit resolution | Current Home AgentSession and FlowSession state |
| `lf wt`, `commit`, `rebase`, `pr`, `ci` | Worktree and delivery operations | Exact repository/Task/GitHub object |
| `lf exec`, `runs`, `usage`, `ps`, `top`, `prune`, `doctor` | Execution and process observation | Current Home only |
| `lf home`, `lf wave place` | Home identity and Work placement | Current Home unless routed explicitly |
| `lf ssh <home-id> <args...>` | Run the target Home's `lf` | Explicit remote Home; no implicit fan-out |
| Loopflow.app | Swift projections and user interaction | Queries the same DTOs and remote routes; owns no lifecycle |

Most commands are local by default. `lf ssh` is transport, not a second API:
the inner `lf` and separator are implicit, the target re-resolves its own Home
state, and durable processes scrub foreground-forwarded secrets before
detaching.

## Harness launch and Run records

The heading remains an inbound documentation anchor; Run is historical vocabulary.
The execution cutover uses one AgentSession admission and capture path for Task,
Wave, Ask, helper and direct callers.

1. Admit the actual lf Exec; resolve typed work without granting Flow authority.
2. Reserve the AgentSession and its initial history/capture reference before
   provider launch. Claim its driver and record exact publication state.
3. Publish immutable input atomically. An unpublished reservation is recoverable;
   uncertain publication/spawn evidence never permits a blind duplicate launch.
4. Start or reconnect the native engine. Record its identity and endpoint, distinct
   from the client's process and the conversation's driver.
5. Append correlated provider outcomes and usage; retain missingness. Settle a
   selected Flow completion only under its separate boundary claim.
6. Settle the actual command's Exec when the process completes, independently of
   whether its conversation or parked Flow remains open.

Payloads may remain large immutable files. SQLite owns identity, attribution,
current control and searchable history. Old `runs/` manifests, JSONL, terminal
receipts and sidecars are import inputs with their original bytes preserved until
verified migration; ordinary readers do not consult them as alternate identity.
The retained operational process evidence is not a new lifecycle object.

## Tracked Work and bounded Task advancement

```text
Task trigger -> claim current invocation -> execute captured node
                                         -> settle, stop, or await Session
```

A Task selects one managed FlowSession; repeated passes retain that identity.
Claims fence session identity, cursor version, worker generation
and exact selected native completion. A late result cannot advance a replacement
even if numeric versions repeat. Successful native history and its consumption
are retained; failed turns cannot donate navigation to a successful retry.

Recovery uses saved Skills, Xor branches and review policy after source changes
or disappears. Missing process evidence stays uncertain. Explicit restart retains
the previous FlowSession as history and captures another. Completion clears the
managed selection without completing Task Work or choosing a successor.

Independent helpers may carry the same Task and separately authorized Git/PR
operations. Attribution does not acquire its managed Flow claim. Binding to done
Work assigns a conversation without reopening it. Unresolved reviews remain
visible when their associated Work is terminal.

Chapter rotation preserves Task, FlowSession, claim, AgentSession, PR and worktree
identity when moving started unfinished Tasks. Linear status changes converge
through fresh provider reads; there is no atomic repository-wide Chapter switch.

## Durable communication

Task steering posts Linear comments. The Task's advancement claim identifies
the live recipient. Independent attributed conversations do not receive steering.
Workers refresh comments into local delivery events and starting context;
publication, seed inclusion, and provider acceptance are distinct evidence.
Steering an idle Task starts nothing. Wave guidance travels as extra
instructions to `wave/operate`.

```bash
lf task comment INF-123 "keep the public name"
```

### Questions and sessions

These examples use current command spellings; the lifecycle contract is above.

```bash
lf -b implement
lf session list --interactive false --task INF-123 --json
lf session connect SESSION
lf session rename SESSION "Migration review"
lf session bind SESSION --task INF-123
lf ask "Review this migration with me"
lf session ready "Ready for review"
lf session complete SESSION
```

Every conversation has one AgentSession regardless of launch surface. Connect
uses the existing engine where possible; restart is explicit. Name, feedback,
native identity and history survive both. A suggested title cannot overwrite a
human-assigned title. CLI and Desktop use the same action and availability reason.

Ready saves feedback and keeps the conversation open. Complete persists its
closed state and exact feedback before teardown. A keyed Ask retry returns the
saved result without launching another conversation. A Flow review returns
feedback for the following decision; it never selects that decision's edge.
Pane close, provider exit and readiness do not complete the review.

The desktop terminal pool keys on stable AgentSession identity. Rename, bind,
reconnect and replacement retain the pane and draft when reusing that surface.
Provider restart does not claim preservation of text never submitted to Loopflow
without separate UI evidence.

## Flow execution

```bash
lf task run INF-123                  # Project's Flow
lf task run INF-124 --flow incident  # explicit override
lf flow example                     # with or without Task attribution
lf flow resume FLOW_SESSION          # captured progress
```

FlowSession captures the fully expanded graph, every Skill/router/Xor alternative,
cursor and return counts. Definition changes affect new Sessions, never saved
execution. Backward edges stay finite graph structure; future loop passes are
not preallocated. Node IDs are local typed identities, not path strings.

Taking an Iterate edge updates the cursor and return counters in the same
FlowSession. Its node and iteration tuple identify the pass in AgentSession and
Flow history, including overlapping backward edges. Retry retains that position;
another Iterate selects the next pass. Neither loop passes nor authored subflows
create FlowSessions, claims or lifecycles.

Exact completion and continuation settle in the existing version/claim
transaction. Execution, review and driver locking use the same FlowSession that
the Task selects. Task and taskless execution use one driver and need no synthetic
planning records. The cutover-status table tracks removal of the current child
representation; existing history must survive that forward migration.

A decision records Advance or Iterate for its selected successful agent completion.
Failure or interruption cannot submit a verdict. A keyed unblock Ask returns
feedback for reassessment at that same boundary. Review definitions and feedback
survive source deletion, restart and repeated completion. There is no alternate
file-backed cursor. Recovery of an uncertain mechanical effect still requires
inspection; cursor settlement alone cannot establish exactly-once external effects.

Finite Wave planning uses ordinary attributed agent conversations. Historical
Wave continuations remain preservation evidence and confer no new execution or
process-control authority.

## Task delivery algorithm

A Task binds planning to one active PR branch and managed Git worktree. A
checkout on that branch identifies the Task regardless of its upstream; the
stored path supplies placement for explicit Task selection. The current delivery
implementation can rotate a settled Task onto a later serial branch. Once that happens, the old branch no
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

1. `lf task checkout` resolves one Linear Issue inside one Project and creates
   or reuses Task Work, its worktree, and its serial PR identity. It starts no
   agent work.
2. Independent `lf --task ...` conversations may work in that substrate directly.
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
6. Landing with `-c` completes the Task; bare landing keeps it open. Serial PR
   rotation and separately stacked dependent Tasks retain their own identities.

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

Agent conversations receive no worktree writer token. Commit, PR mutation, restart
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
Wave locator locks serialize relocation filesystem ownership. A
`<store>.migration.lock` serializes backup plus schema application. The current
promotion operation holds `$HOME/.lf/promotion.lock` exclusively for its full
upgrade transaction. These locks do not turn a conversation ID into authority.

## Process ownership and control

Causal ancestry and conversation identity are not process ownership. Local child
handles permit the spawning process to control its child. Cross-process control
requires exact PID/start identity and the appropriate native scope, claim and
provider generation. Revalidate that evidence before every signal.

A driver can die while its engine continues. A saved endpoint alone is not
liveness; a missing endpoint alone is not engine death. Recovery reads surviving
native history and preserves unknown command outcomes. Explicit restart stops
only the selected conversation; a shared engine's sibling conversations survive.

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
                                      AgentSession native history
```

Wave operations are finite conversations. Tasks drive their selected invocation; local
PR supervision watches delivery. Crossing Homes is an explicit `lf ssh` hop
whose target proves its Home identity.

### Multi-Home placement and execution

`HomeId` is stable identity; its SSH route is replaceable. `Placement` maps
Work to a Home and stores eligibility, never liveness or signal authority.

```bash
lf home observe <home-id> ssh://jack@mini.local
lf wave place <wave-id> <home-id>
lf ssh <home-id> --wave product wave/operate
```

The target uses its own store, repository, provider homes, OS locks and payload
directory. Reads remain Home-local. Foreground SSH may explicitly forward
selected account authority; detached processes use installed credentials.

Wave selection always resolves `(canonical repository, slug)` to one UUID.
Bare-slug diagnostics fail when more than one repository owns the slug; no
read or mutation chooses one by order. A scoped lookup repairs an equivalent
legacy path spelling to the canonical repository in one transaction.
`lf wave relocate <uuid>` is the only semantic locator mutation: it fences
the Wave chord, moves authored files, commits the new locator
transactionally, and leaves PM, Work, and Home-placement rows joined to the
unchanged UUID. A target-local `.lf/tmp/wave-relocations/<uuid>.json` receipt
bridges the filesystem/SQLite commit boundary; retrying after a committed crash
finishes verified source cleanup, then removes the receipt. Repository moves
also require compatible configured PM Teams so relocation cannot impersonate
the separate `lf repo reteam` operation.

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
replacement. It does not discover, drain, stop, or settle conversations and it is not
held by ordinary harnesses. Store cloning remains useful because preview can
prove a candidate schema without mutating the selected store.

An already-running `lf` retains its executable and selected store path.
An execution-schema cutover must account for those writers before activation. On the first published-to-development switch, the process may keep
writing successfully to the prior production store after new commands select
the cloned development store; those writes become invisible to the new
selection. A later development-to-development promotion may reuse and migrate
the selected store, so an old writer may instead fail against changed schema.
Promotion pauses and replaces the known services it owns but does not discover
every shell or provider process. The clone proves candidate readability; it
does not provide cross-store write continuity or old-schema compatibility.

## Truth and projections

The map is the ownership index. Truth remains distributed across Home-local
SQLite, repository files and Git, the command journal, Linear, GitHub, and provider
homes or Doppler; none is a fallback authority for another.

Intentional copies stay read projections:

<!-- architecture-projections:start -->
| Projection | Authority copied | Freshness and consumer |
| --- | --- | --- |
| [`PmSnapshotRow`](../rust/loopflow/src/store/mod.rs) / `pm_snapshots` | Linear planning | Atomic PM sync replacement; `lf wave status`, `lf roadmap`, and the Mac app read it but never author through it. |
| [`TaskLinearObservation`](../rust/loopflow/src/work/task/mod.rs) / `task_linear_observations` | Linear Issue state | Reconciliation records provider evidence before applying lifecycle changes. |
| [`GithubObservation`](../rust/loopflow/src/work/task/mod.rs) / `task_prs`, `ci_incidents` | GitHub PR/check state | Webhook or foreground reads update Task delivery evidence; GitHub remains merge truth. |
| `tests/fixtures/dto/` | Rust `lf --json` DTOs | Rust and Swift fixture tests reject required-field or enum drift. |
| `tests/fixtures/migrations/` | Ordinal-free migration drafts and the Python canonicalizer | Rust build/runtime and Python release tests reject ordering, body-byte, checksum, and graph-error drift. |
<!-- architecture-projections:end -->

`lf wave status` and `lf roadmap` derive Task conditions from Work, invocation and
PR facts. AgentSession and FlowSession own ancestry; immutable history keeps
earlier attribution. The transitional joins are listed in the cutover status. No projection acquires launch, Work-mutation,
credential or signal authority. UI grouping is cached presentation, not another
attribution store.

## Extension rules

| Area | Safe extension | Architectural constraint |
| --- | --- | --- |
| Execution queries | Add an index for a measured Exec, AgentSession or FlowSession query | Filter before payload IO; identity/ancestry come from the owning row, never fallback files |
| Multi-Home views | Fan out read-only commands through `lf ssh` | Do not centralize Home-local execution ownership or silently mix local and remote scope |
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
| `Project Session`, `Task Session`, `project_sessions`, `task_sessions` | `rust/loopflow/src/store/migrations/`, `rust/loopflow/src/store/migrations.rs`, `rust/loopflow/src/store/tests/fixtures/`, `release/` | Stable Project/Task **Work**; AgentSession owns the conversation and native history; FlowSession owns captured progression. |
| `session context`, `LF_SESSION` | — | Typed Work ancestry and Exec/AgentSession provenance; transitional launch environment names are listed in cutover status. |
| `lf radio`, `agent bus` | `release/` | Typed Work observations, Steer, synchronous questions, and review FlowSteps. |
| `pm.linear_project`, `projects/<slug>.md` | `release/` | `pm.linear_initiative`; Linear Initiative → Project → Issue. |
| `machine-local host`, `machine-global command`, `machine-global mutation`, `machine-global reservation` | — | Home-local keeper, command, mutation, or reservation. |
<!-- architecture-vocabulary:end -->

The following map gives the destination of retired or transitional representations.
Cutover status identifies which runtime consumers remain; the map does not claim
their deletion is finished.

| Historical or transitional representation | Target owner |
| --- | --- |
| Wave-scoped Chapter / `wave_chapters` | Linear Project status; Chapter is the shared In Progress name, with no stored object |
| Recommended Flow / `flows.recommended` | Project's `flow` template selection |
| `FlowPosition`, `PinnedTaskFlow`, `task_flow_positions` | FlowSession: captured graph and execution state together |
| `FlowRun`, `flows/<id>/position.json` | FlowSession |
| Subject selector list on a Run | Typed AgentSession/FlowSession ancestry and immutable event attribution |
| Four Session projections, Ask files, composite boundary Session IDs | `agent_sessions`, keyed by stable Session ID, with driver Exec and native history |
| Session name/resolution and provider attachment sidecars | AgentSession attributes, native identity and exact process evidence |
| Step occurrence / path-string node key | Invocation ID, local node ID, captured iteration tuple |

Canonical migrations, migration fixtures, and release notes retain historical
names because changing shipped evidence would rewrite history. Operational docs
and current runtime source do not. Chapter archives under `.lf/chapters/` are
dated evidence, excluded from live vocabulary and compatibility-seam discovery.

## Authority and failure invariants

- Linear owns Project status and shared planning. A Chapter is the shared name of
  one In Progress Project per Wave; partial rotation is visible and retryable.
- Wave instruments and observations survive Chapter changes. Missing target
  planning is unknown, not proof that an instrument is untargeted; it cannot
  erase an observed reading.
- An agent launch needs no planning parent, but requires writable admission and
  immutable captured input before provider side effects.
- Exec is an actual lf process. AgentSession history owns native outcomes and
  usage; FlowSession owns captured progression and mechanical boundary results.
- Command outcome, native completion, current liveness and Work disposition are
  separate facts. Missing terminal evidence stays unknown.
- Causal parentage and Task attribution confer neither process control nor Flow
  settlement. Exact process identity and native scope govern signaling.
- An authoritative conversation driver is singular; provider generation differs
  from driver generation. Passive readers acquire neither claim.
- A Flow consumes its selected successful native completion under version/claim
  fencing. Failed turns cannot donate verdicts or routes to successful retries.
- Complete releases an Ask or returns review feedback. Readiness and provider
  exit never choose an edge; a following decision owns its own navigation.
- Bind is write-once, same-target idempotent and valid for done Tasks. CLI states
  the permanent target and writes; Desktop confirms. Historical attribution and
  set-once Started survive assignment and driver changes.
- OS locks cover their named critical sections. Rebase admits only its exact
  recovery child; causal identity does not bypass Git or PR mutation authority.
- Store copies preserve evidence without acquiring process authority. Promotion
  and historical-writer continuity remain separate proof obligations.
- Reads are Home-local unless explicitly routed by `lf ssh`. Indexed summaries
  precede payload IO; historical files are import inputs, never identity fallback.
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
