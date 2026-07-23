---
layout: default
title: Architecture
---

# Architecture

Start here. Each row names one Loopflow concept, the source allowed to decide
it, the structure that represents it, where it persists, the process holding
its pen, and the public door callers use. Later sections explain the sequences;
they do not introduce another ownership model.

```text
User intent
├── Skill ── Flow
└── Work: Wave ── Project ── Task
    ├── Steer
    ├── Wait
    └── Epoch / Basis
        └── Run / Containment
            └── AgentInvocation
                └── Turn
                    └── Ask ── Answer
```

## The map

<!-- architecture-map:start -->
| Concept | Truth and authority | Data structure | Persistence | Process owner | Public surface | External edge |
| --- | --- | --- | --- | --- | --- | --- |
| **User** — the authenticated human or external harness | User authority authors root input and approves external effects; an in-Run process cannot impersonate it. | [`Author`](../rust/loopflow/src/durable.rs), [`AuthenticatedRequest`](../rust/loopflow/src/durable.rs) | No User row; authored effects persist on the concept they change. | `lf` | `lf :`, `lf desktop` | `exec:open`, `exec:osascript`, `exec:pbpaste`, `exec:id` |
| **Skill** — one reusable prompt with assembled context | Repository/builtin Skill Markdown is authoritative; discovery selects one source. | [`Skill`](../rust/loopflow/src/engine/flow.rs), [`SkillSource`](../rust/loopflow/src/lf/discovery.rs) | `.lf/skills/`, builtin Skill files, installed vendor Skill directories | `lf-prompt` | `lf skill`, `lf sync-skills`, `lf list` (Skill/Flow catalog) | `exec:python3` |
| **Flow** — an ordered composition of Skills | Repository/builtin Flow YAML and the current playhead decide the next step. | [`Flow`](../rust/loopflow/src/engine/flow.rs), [`FlowPosition`](../rust/loopflow/src/durable.rs) | `.lf/flows/` | `lf __flow-step` | `lf flow` | — |
| **Wave** — durable operating context with goal, memory, cadence, chat, and project selection | The Wave UUID is durable identity; canonical repository plus normalized slug is its mutable human locator. `wave/<name>/GOAL.md` and `MEMORY.md` own repository intent; the Linear Initiative owns shared planning membership. | [`Wave`](../rust/loopflow/src/wave/types.rs), [`WaveLocator`](../rust/loopflow/src/wave/types.rs), [`CanonicalRepo`](../rust/loopflow/src/repository.rs), [`WaveConfig`](../rust/loopflow/src/engine/wave_config.rs) | `waves`; `wave/<name>/`; `.lf/journal/waves/<name>/journal.jsonl`; an in-flight relocation receipt under `.lf/tmp/wave-relocations/` | `lf __resident` behind the Wave listener; listener and relocation share the repository locator lock | `lf wave`, `lf start`, `lf stop`, `lf pause`, `lf resume`, `lf chat`, `lf ls`, `lf status`, `lf roadmap`, `lf cron`, `lf work relocate wave`; `wave GET /health`, `wave GET /conversation`, `wave GET /events`, `wave GET /playhead`, `wave POST /messages`, `wave POST /observations`, `wave POST /stop`, `wave POST /resident/attach`, `wave POST /resident/deltas`, `wave GET /resident/context` | Discord when configured |
| **Project** — one measured bet inside exactly one Wave | The Linear Project definition and KRs are planning truth; the Project Work row owns pursuit lifecycle only. | [`Project`](../rust/loopflow/src/project/mod.rs), [`PmProject`](../rust/loopflow/src/pm/mod.rs) | `projects`, `project_events`, `observation_outbox`; Linear Project content | Project runner inside its Run | `lf project` | Linear |
| **Task** — concrete work inside exactly one Project | The Linear Issue owns directive/status; Task Work owns execution lifecycle, one delivery worktree, and its serial PR chain. Git owns commits/branches; GitHub owns PR/check/merge truth. | [`Task`](../rust/loopflow/src/task/mod.rs), [`TaskPr`](../rust/loopflow/src/task/mod.rs), [`CiIncident`](../rust/loopflow/src/task/mod.rs) | `tasks`, `task_events`, `task_prs`, `ci_incidents`, `task_pr_repair_incidents`, `performance_evidence_authority`, `task_linear_observations`, `task_linear_ingested_comments`; Linear Issue; Git worktree | Task runner inside its Run; foreground mechanical operations and Home webhook reconciliation record delivery evidence. | `lf task`, `lf pr`, `lf wt`, `lf rebase`, `lf commit`, `lf ci` | Linear, `provider:github`, `exec:git`, `exec:gh` |
| **PM projection** — locally readable current planning snapshot | Linear remains authoritative; the Wave UUID keys the projection so locator changes preserve it. Sync atomically replaces the projection and reads never author through it. | [`PmSnapshotRow`](../rust/loopflow/src/store/mod.rs), [`PmWave`](../rust/loopflow/src/pm/mod.rs) | `pm_snapshots` | Foreground PM sync or Home webhook reconciliation | `lf pm` | `provider:linear` |
| **Epoch / Basis** — one attempt at Work truth and its authored-input revision | The Work controller opens/settles Epochs; only committed Steers advance Basis. Terminal Work outranks stale Run observations. | [`Epoch`](../rust/loopflow/src/durable.rs), [`Basis`](../rust/loopflow/src/durable.rs), [`DoneProposal`](../rust/loopflow/src/durable.rs) | `epochs`, `epoch_revisions`, `work_truth`, `work_flow_positions`, `done_proposals` | Current Work controller; active Run proposes completion against an exact Basis | `lf work`, `lf activity` | — |
| **Steer** — durable authored correction to one Work | User or authorized parent Run writes it; incorporation is proven at a later successful Basis boundary. Live send is latency only. | [`Steer`](../rust/loopflow/src/durable.rs), [`Send`](../rust/loopflow/src/durable.rs) | `steers`, `sends`, `tool_responses` | Store transaction, then best-effort provider delivery | Work-specific `steer` commands and `lf work steer` | Model provider when delivery is live |
| **Ask / Answer** — one Turn-local blocking question and immutable response | The route is derived from Work ancestry; the first authorized User or parent-Run answer wins. | [`AskExchange`](../rust/loopflow/src/durable.rs), [`Answer`](../rust/loopflow/src/durable.rs) | `ask_exchanges`, `ask_linear_comment_outbox` | Asking Turn blocks; authorized answerer commits; Linear comment outbox publishes later | `lf ask`, `lf work asks`, `lf work answer` | Linear comments for Task exchanges |
| **Wait** — durable reason Work is not Ready | The Work controller records the unresolved input/time/event/child/capability/effect condition. | [`Wait`](../rust/loopflow/src/durable.rs), [`WaitOn`](../rust/loopflow/src/durable.rs) | `waits` | Home scheduler resolves it and may reserve the next Run | Project/Task wait and status surfaces | — |
| **Home / Placement / Runtime generation** — stable execution authority, the Work-to-Home decision, and the one installed runtime generation | `HomeId` is identity; its SSH route is mutable evidence. Placement changes only while no Run is live. A published candidate owns one crash-recoverable upgrade receipt and advances the generation only after old containment drains. | [`Home`](../rust/loopflow/src/durable.rs), [`Placement`](../rust/loopflow/src/durable.rs), [`HomeUpgradeReceipt`](../rust/loopflow/src/lf/commands/install.rs) | `homes`, `work_placements`, `home_runtime_generations`, `home_upgrades`, `home_upgrade_work`; Home-local SQLite. `~/.lf/upgrades/*.json` is only the previous-schema compatibility/recovery bridge and is removed after durable settlement. | `lfd` owns eligible Wave listeners on one Home; hidden `lf install` transport lets the staged candidate own an upgrade transaction | `lf home`, `lf ssh`; `lfd GET /health`, `lfd GET /status`, `lfd POST /waves/start`, `lfd POST /waves/stop`, `lfd POST /waves/reconcile`, `lfd POST /linear/webhook`, `lfd POST /github/webhook` | `exec:ssh`, `exec:launchctl`, `exec:systemctl` |
| **Run / Containment** — one scheduler claim and physical execution boundary for an Epoch | The opaque Run lease is the sole Work-write capability. Run containment, never invocation order, is the interrupt/recovery target. | [`Run`](../rust/loopflow/src/durable.rs), [`Containment`](../rust/loopflow/src/durable.rs), [`RunLease`](../rust/loopflow/src/durable.rs) | `runs` | `lf __work` for Project/Task bodies; tmux or one process group contains the Run | Project/Task lifecycle surfaces | `exec:lf`, `exec:tmux`, `exec:kill`, `exec:lsof`, `exec:ps`, `exec:sh` |
| **AgentInvocation / Turn** — one replaceable provider conversation and one measured model boundary | The harness reports provider identity and boundaries; supervising Run is provenance, not authority. | [`AgentInvocation`](../rust/loopflow/src/durable.rs), [`Turn`](../rust/loopflow/src/durable.rs), [`InvocationRoute`](../rust/loopflow/src/durable.rs) | `agent_invocations`, `agent_turns` | Provider harness process supervised by a Run or explicit User launch | `lf invocation` | `provider:claude`, `provider:codex`, `provider:opencodezen`; `exec:claude`, `exec:codex`, `exec:opencode` |
| **Trace / Context / Usage** — append-only evidence of what processes and providers did | Provider request receipts accumulate into one Turn checkpoint stream; Rust derives generation, input/cache, cost, and context projections without another counter. Trace assets preserve provenance without becoming Work truth. | [`TurnUsage`](../rust/loopflow/src/chat/types.rs), [`TurnUsageSample`](../rust/loopflow/src/store/mod.rs), [`UsageSnapshot`](../rust/loopflow/src/usage.rs), [`ContextAsset`](../rust/loopflow/src/trace.rs) | `turn_usage_samples`, `run_events`, `context_assets`, `context_decisions`, `blob_tokens` | Each Loopflow/harness process records its own evidence; readers are read-only. The repository `telemetry-daily` Flow renders the maintainer scorecard. | `lf tokens`, `lf usage`, `lf ps`, `lf top`, `lf prune`, `lf context`, `lf doctor`, `lf runs`, `lf execs`, `lf trace` | `exec:which` |
| **Provider account / route** — credential authority and ordered provider selection on one Home | Provider token/account rows and Access Profiles own routing; credentials stay in provider homes, encrypted storage, Doppler, or forwarded foreground leases. | [`Provider`](../rust/loopflow/src/provider_auth/mod.rs), [`AccessProfile`](../rust/loopflow/src/profile.rs), [`ProviderRoute`](../rust/loopflow/src/profile.rs), [`ProviderAccount`](../rust/loopflow/src/store/mod.rs) | `access_profiles`, `account_access_profiles`, `provider_accounts`, `provider_account_limits`, `provider_routes`, `provider_session_accounts`, `provider_tokens`, `provider_deliveries` | Home-local auth/account broker; durable processes use credentials installed on their Home | `lf auth`, `lf profile`, `lf route` | `provider:doppler`, `exec:doppler`, `exec:security`, `exec:secret-tool` |
| **Schema frontier** — ordered definition of durable control storage | Released migration bytes are immutable authority; drafts join only through deterministic release materialization. | [`Migration`](../rust/loopflow/src/store/migrations.rs), [`MigrationId`](../rust/loopflow/src/store/migrations.rs) | `schema_migrations`; canonical and draft migration files | Store open validates/applies; release cut publishes | `scripts/install.py refresh`, `lf release` | — |
<!-- architecture-map:end -->

The public API column covers top-level command families, not every subcommand or
Rust function. [`lf` reference](lf.md) owns argument-level detail. DTOs emitted
by `--json` are required-field projections; Rust/Swift fixture tests own their
wire parity.

## Execution authority

One non-ended Run owns an Epoch's execution authority and physical containment.
Its opaque `LF_RUN_LEASE` is the only capability that permits a process to write
as that Work.

A Reserved Run has no containment. Active and Stopping Runs have one complete
tmux or process-group identity. An Ended Run retains any containment it
acquired. Interrupt and recovery target that containment directly.

An AgentInvocation records one provider conversation: provider, model, account,
surface, resume token, timestamps, and Turns. Its optional supervising Run is
provenance. Starting another conversation does not rotate the Run lease, reserve
another scheduler slot, or change the interrupt target.

If a provider conversation fails while its runner remains live, the Run may
start another AgentInvocation. If containment is lost, recovery ends the
observed Run and reserves a new Run whose recovery trigger points to it.

## Durable input

Steer changes authored Work input and advances Basis. Provider injection is a
best-effort fast path; a later successful boundary is the semantic receipt.

Ask/Answer is Turn-local tool I/O. It does not move Basis, enter the Steer queue,
or advance a Flow step.

```text
Ask -> Turn -> AgentInvocation -> Run -> Epoch -> Work
```

That chain prevents a question from claiming one Work while pointing to another
Work's Turn. One Turn has at most one unanswered Ask. Answer fields are written
together, only while unanswered, so concurrent writers cannot replace evidence.
A child routes to its immediate parent Work; an interactive root may route to
the User; a headless root with neither route fails instead of waiting forever.

Interactive Task phases are advisory. They launch once and advance without
waiting for a window or handback. A successful interactive surface is read-only
beside the next writable phase. Durable Ask is the sole human-input primitive
that can hold a Turn open.

## Processes

```text
Loopflow.app / external harness / shell
                    │
                    ▼
                   lf ─────────────── Linear · GitHub · provider auth
                    │
          Home-local SQLite
                    │
                    ▼
             lfd / WaveHost
                    │ starts eligible placed Waves
                    ▼
          Wave listener ── Wave resident
                    │             │
                    │       provider harness
                    ▼
           Project or Task Run
                    │
          Task worktree ── GitHub PR
```

The Wave listener owns journal, HTTP, discovery, and typed child observations.
The resident owns cadence and its provider process; it sends ordered deltas and
never writes the journal directly. `lfd` is Home keeper machinery, not an agent
or remote-control authority. Crossing Homes is an explicit foreground `lf ssh`
hop whose target proves its Home identity.

Wave selection always resolves `(canonical repository, slug)` to one UUID.
Bare-slug diagnostics fail when more than one repository owns the slug; no
read or mutation chooses one by order. A scoped lookup repairs an equivalent
legacy path spelling to the canonical repository in one transaction.
`lf work relocate wave <uuid>` is the only semantic locator mutation: it fences
the Wave chord, moves authored files and the journal, commits the new locator
transactionally, and leaves PM, Work, Run, and Home-placement rows joined to the
unchanged UUID. A target-local `.lf/tmp/wave-relocations/<uuid>.json` receipt
bridges the filesystem/SQLite commit boundary; retrying after a committed crash
finishes verified source cleanup, then removes the receipt. Repository moves
also require compatible configured PM Teams so relocation cannot impersonate
the separate `lf pm reteam` operation.

## Truth and projections

The map is the ownership index. Truth remains distributed across Home-local
SQLite, repository files and Git, the Wave journal, Linear, GitHub, and provider
homes or Doppler; none is a fallback authority for another.

Intentional copies stay read projections:

<!-- architecture-projections:start -->
| Projection | Authority copied | Freshness and consumer |
| --- | --- | --- |
| [`PmSnapshotRow`](../rust/loopflow/src/store/mod.rs) / `pm_snapshots` | Linear planning | Atomic sync or Project-phase refresh replacement; `lf status`, `lf roadmap`, and the Mac app read it but never author through it. |
| [`TaskLinearObservation`](../rust/loopflow/src/task/mod.rs) / `task_linear_observations` | Linear Issue state | Reconciliation records provider evidence before applying lifecycle changes. |
| [`GithubObservation`](../rust/loopflow/src/task/mod.rs) / `task_prs`, `ci_incidents` | GitHub PR/check state | Webhook or foreground reads update Task delivery evidence; GitHub remains merge truth. |
| `tests/fixtures/dto/` | Rust `lf --json` DTOs | Rust and Swift fixture tests reject required-field or enum drift. |
<!-- architecture-projections:end -->

## Compatibility seams

Compatibility survives only when it crosses immutable external history. Each
seam names its translation and deletion boundary; none is a second current
model.

<!-- architecture-shims:start -->
| Seam | Current concept | Source and removal boundary |
| --- | --- | --- |
| `shim:pre-run-promotion` | Session-era active leases become Run drain evidence during one-way store promotion. | [`read_legacy_active_runs`](../rust/loopflow/src/lf/commands/install.rs); remove only when no supported installed frontier predates Run. |
| `shim:legacy-chat-import` | Old journal turns become one immutable Wave conversation epoch. | [`ConversationEpochImport`](../rust/loopflow/src/wave/journal.rs); remove only when old journals are no longer supported. |
| `shim:retired-op` / `lf op` | Rejected namespace returns the surviving top-level command name. | [`Commands`](../rust/loopflow/src/lf/mod.rs); remove when external callers no longer need the diagnostic tombstone. |
| `shim:rams-alias` | Installed `rams/rams` command resolves to the Skill model. | [`SkillSource`](../rust/loopflow/src/lf/discovery.rs); remove when the external single-file command is no longer supported. |
| `shim:local-refresh-wrapper` | Old script entrypoint forwards to the single `scripts/install.py refresh` implementation. | [`pull-local-bin.sh`](../scripts/pull-local-bin.sh); remove after external automation uses the current command. |
| `shim:retired-app-replacement` | Promotion removes the previously shipped app bundle after the current app commits. | [`AppPromotion`](../rust/loopflow/src/lf/commands/install.rs); remove after the retired bundle name is outside supported installs. |
<!-- architecture-shims:end -->

## Historical-only vocabulary

The scanner matches exact phrases, not overloaded words. Provider resume
sessions, tmux sessions, and `session.launch` are current. The authored chat
reference `project:<slug>` is also current; it is not the old Linear-label PM
model.

<!-- architecture-vocabulary:start -->
| Retired term | Allowed scopes | Current language |
| --- | --- | --- |
| `Project Session`, `Task Session`, `project_sessions`, `task_sessions` | `rust/loopflow/src/store/migrations/`, `rust/loopflow/src/store/migrations.rs`, `rust/loopflow/src/store/tests/fixtures/`, `rust/loopflow/src/lf/commands/install.rs`, `release/` | Project/Task **Work**, Epoch, and Run. |
| `session context`, `LF_SESSION` | — | Run context and the exact Run lease/invocation variables. |
| `lf radio`, `agent bus` | `release/` | Typed Work observations, Steer, and Ask/Answer. |
| `pm.linear_project`, `projects/<slug>.md` | `release/` | `pm.linear_initiative`; Linear Initiative → Project → Issue. |
| `machine-local host`, `machine-global command`, `machine-global mutation`, `machine-global reservation` | — | Home-local keeper, command, mutation, or reservation. |
<!-- architecture-vocabulary:end -->

Canonical migrations, migration fixtures, and release notes retain historical
names because changing shipped evidence would rewrite history. Operational docs
and current runtime source do not.

## Invariants

- Wave → Project → Task is the complete planning hierarchy: no recursive or
  orphan Projects.
- A Wave UUID is stable across rename and repository rehome; repository-scoped
  locators are unique, and bare slugs are never mutation authority.
- Linear owns current Project/Task planning; SQLite projections never become an
  authoring fallback.
- Active Project Work adopts one complete refreshed Linear plan between provider
  turns; a refresh or ownership failure stops before the next turn rather than
  serving the prior plan.
- One non-ended Run exists per Epoch; only its current opaque lease writes as
  Work.
- Run containment, not invocation ordering, is the interrupt and recovery
  target.
- Every Turn belongs to one AgentInvocation; every Ask belongs to one Turn.
- Durable Ask is the only human-input primitive that blocks a Work Turn.
- Ask/Answer never allocates an Epoch revision or enters Steer delivery.
- An advisory interactive surface has no Task-worktree write authority.
- Controller evidence may settle Work without inventing an AgentInvocation or
  Run.
- DTO fields are required unless their type is explicitly optional.
- No Session-as-Work, control Launch, Feedback/Continue channel, reviewer flag,
  or invocation-attention column participates in the current runtime.

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
