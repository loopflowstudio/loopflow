# goals wave memory

Steers Loopflow toward persistent Goal-driven Waves: goals as the authored loop prompt, Asana as the live roadmap, and Concerto as the session surface.

## The wave agent + waves outward (2026-07-04)

**Governing principle (Jack): "waves outward" — radically zero centralized
control.** The wave is the unit of sovereignty; nothing sits above the waves.
The wave process is the LISTENER unifying sovereign `lf` runs (PUBLISHERS)
by subscription; coordination is shared fact (the store IS the registry) and
notification, never command. Concerto is a viewer; `lfd serve`'s only future
is relay listener + access gate (it execs lf — constitutional clause). Test
every feature: "does this create a center?"

**Shipped (branch `jack-heart.goals.20260703_1511`, PR #796 — the full
demo):** `lf wave <name>` = a sovereign per-wave server: persistent codex
app-server mind (0.142.5, live-proven), append-only journal as runtime truth
(thread/state/queue are folds; restart-safe), steer/interrupt with an
anti-wedge deadline, open-turn streaming, store-direct registration +
one-brain, `lf q worker run` (placement fresh/pool/stack;
`<repo>.<wave>.<id>` worktrees), `lf chat`/`lf memory` (emissions are
messages with bylines; server holds MEMORY.md's pen at the origin), ambient
context (`<lf:wave-chat-recent>` + `<lf:wave-memory>` in every lf run; empty
when waveless). Demo performed live: mind dispatched a worker; the worker
sent four attributed `lf chat` reports mid-run; the mind reacted and ran
`lf memory add` unprompted; observation journaled dispatch/finish; thread
survived restart. `lf goal` deleted; the old loop_ticker path stands down
for served waves.

**Hard-won learnings:**
- **Schema-drift class:** the ontology collapse renamed tables/columns by
  editing historical CREATE migrations in place — fresh dbs fine, recorded
  dbs stranded (`wave_run_id`, `wave_runs`, `agents/fork_runs.wave_run_id`).
  Healed by rename migrations 048/049 + a rename-convergence tolerance
  list, and `open_existing_store` now migrates on direct open. When code
  and a live db disagree, diff a fresh-migrated schema against the live one
  before whacking single moles. NEVER edit an applied migration.
- **Vendor drift discipline:** conformance traces catch mapping bugs; only a
  live smoke catches protocol drift (codex 0.142.5: app-server subcommand,
  clientInfo required, client-sent `initialized`, `turn/start {threadId,
  input:[...]}`, steer carries `expectedTurnId`, usage via
  `thread/tokenUsage/updated`) and process-tree bugs (nvm shim makes the
  real binary a grandchild — process-group kill; reader/writer shutdown
  deadlock; tmux kill-session sends SIGHUP which bypassed SIGINT-only
  cleanup hooks).
- **The emission vocabulary is exec, one door:** `lf chat`/`lf memory`/`lf q`
  — the only door every process on the machine has; worker reports ride it,
  which solved report thinness for free. Speak locally, escalate
  deliberately (`--parent` walks store ancestry to the parent's registered
  endpoint).
- **Free-energy brief** (scratch/research/softmax-free-energy.md, in git
  history): the design has the tradition's structure, not its dynamics —
  and "unattended iterations" as a metric rewards the dark-room failure
  mode; pair it with a progress setpoint. Roadmap item "Wave dynamics"
  carries the adopt-nows.

**Roadmap consolidated in Asana (2026-07-04):** 11 open → 8, priorities on
the custom field (Urgent: demo PR; High: lf language, Concerto viewer; Med:
dynamics, spend cap, prove-the-language; Low: backends a/b), every item
reframed for waves-outward (spend-cap enforcement moved out of the daemon;
backend b = sovereign waves behind the gate, not a hosted daemon).

## Shipped (runtime model foundation)

- **Two-file wave surface** — `wave/<name>/` is `GOAL.md` (intent) + `MEMORY.md` (this file). Both are injected into the wave loop's assembled context, so the agent reads its intent and memory each iteration.
- **Wave / Run / Session** — the lfd runtime model was reduced to three product nouns. `WaveRun` + `AgentRun` collapsed into `Run` (execution/result lineage, flattened — no more `WaveRunSnapshot`). `TerminalSession` + the old conversation session collapsed into `Session` (attachable live control surface). `AgentLaunch` and the launch-envelope DTOs are gone: launching returns the durable `Session`.
- **Session `use`** — `wave_agent | worker | palette` lives on the session, not inferred from a nullable task/run. Role is read off `Session.use`, not `(source, wave_run_id)`.
- **lfq as the runtime surface** — `lfq wave run` ensures a wave-agent `Session`; `lfq worker run` creates a `Run` + linked worker `Session` and spawns the work; `lfq sessions` / `lfq attach <id>` list and attach live sessions over tmux. This replaces the old `/dispatch` route and `lf op dispatch`.
- **Goal primitive** — `goal` is the third prompt primitive (step/flow/**goal**). The durable `Wave` carries a required `goal: String` (default `ship-roadmap`) alongside `primary_flow`. `load_goal` resolves `.lf/goals/<name>.md` repo→home→builtin (legacy singular `.lf/goal/` and repo-root `goal/` do not resolve); the wave loop body (`lfd/executor/wave/mod.rs`) runs `wave.goal` as its iteration prompt via `render_goal`, which exposes available flows, a roadmap handle, metrics, memory, and in-flight dispatches — so the goal prompt decides its next move and dispatches inner work through `lfq worker run`.
- **Demo** — `scripts/demo_waveagent.sh` renders the goals wave prompt and shows MEMORY.md reaching context.

## Vocabulary decision (2026-07-03, Jack)

Drop the **"chord"** concept. A parent wave with child waves is just a **wave tree** — parent / child / sibling, that's the whole model. There is no separate chord entity, no "member" wave. #781 already built exactly this (a `parent_wave_id` tree, no `wave_type`/`Chord`), so the code is right; the cleanup is vocabulary. Goals-wave docs use "wave tree" now. The govern/garden surface still says "chord-wave"/"member wave" (`flow.rs:2771`, `govern/step/scan.md`, etc.) — a separate reframe owned by the root/govern surface. `naming.rs`'s "chord" is the musical wordlist, keep it.

## High-priority tier — research outcomes (2026-07-03)

The four `priority: high` Asana items (01–04) were assessed/designed this loop. Docs in `scratch/2-*.md`.

- **01 Asana live roadmap — essentially shipped.** No local mirror (removed `c113ef04b`); the loop reads Asana live each iteration via `lf op pm show` → `AsanaClient::list_items`; dispatches work; moves tasks to Done. Only gap was the "with a PR link" clause: `AsanaClient::comment()` existed but was never exposed to the CLI. Closed by adding `--pr <url>` to `lf op pm update` (posts PR link as a comment; with `--status done` closes in one call). **PR #780**, branch `jack-heart.asana-roadmap`. Close the tracking task once it lands.
- **03 Wave ancestry — BUILT, PR #781** (branch `jack-heart.wave-ancestry`). Migration `044_wave_parent.sql` (new — see stale premise below), `parent_wave_id: Option<LfdId>` on `Wave` with `with_parent` builder, `children_of()` store query, `child_waves` populated in the tree route, DTO mirrored Rust/Python/Swift with `tests/fixtures/wave.json`. All runnable tests green (833 Rust lib + integration, 95 pytest, Swift ContractTests). Follow-on not in PR: end-to-end executor test looping a wave tree (one live WaveAgent session per repo) and any production code that actually *constructs* child waves via `with_parent` — nothing builds trees yet. **Item premise was STALE.** The item says "the store already has the column"; it does not. Migration `013_remove_chord_tree.sql` dropped `parent_wave_id`/`wave_type`/`position`; `028` dropped the fallback tables. Fix = *add* a migration (~044) + one `parent_wave_id: Option<LfdId>` on `Wave` (`types/wave.rs:229`) + `children_of()` store query + populate `child_waves` (currently hardcoded `Vec::new()` at `routes/waves.rs:837`). Recommend deriving chord-ness from "has children" rather than reviving `wave_type`/`position`. Touches all five wave query templates in `catalog.rs` + DTO mirror + `tests/fixtures/wave.json`.
- **04 Wave budget — open question resolved at enforcement authority.** Core owns a hard floor (`spend_cap` field, per-run cost accrual, at-cap pause + block→human, parent/child rollup); user-land owns policy below the ceiling via an exposed cost signal + the existing pause/block primitive. Cost is already parsed then discarded (`stream.rs:278` drops Codex usage; Claude/OpenCode cost printed to stderr only); dormant `CostRates`/`lookup_cost_rates` in `lfd/providers.rs`. Enforcement slots beside the max-iterations valve at `loop_ticker.rs:90`. Parent/child (wave-tree) rollup is gated on 03 (ancestry); single-wave cap ships independently. Needs a `Money` cents newtype + `tests/fixtures/dto/wave.json`.
- **02 Cloud backend — recommend A2 (lfd scaffolds; vendor owns the loop).** Vendor research is decisive: Claude **Routines** are the only true server-side schedule (machine-off), but there's **no create-routine API** — created only via web/Desktop/`/schedule` CLI; `/fire` only triggers an existing one. Codex has cloud task launch (`codex cloud exec`) but **no server-side schedule** and no public REST task API. So A1 (lfd registers a recurring trigger via API) is impossible uniformly today. A2 design reuses `render_goal` + `sync_skills` + `lf op`, behind a new `lf op cloud <vendor>`; the one net-new piece is a `.mcp.json` Asana emitter (a fresh cloud clone has no `lf`/local OAuth, so roadmap access goes over MCP). Deep-link back = persist `cloud_session_url` on the Wave, Concerto opens it.

**Cross-item dependency:** 03 (ancestry) unblocks the parent/child rollup in 04 and the whole wave-tree/cross-repo model. It's the leading edge.

## Open regression

- **Wave ancestry dropped.** The reduction removed the parent-wave field from the durable `Wave` type, so `WaveAgentTree.child_waves` is always empty and the wave-tree structure is invisible. Reintroduce ancestry before child waves can appear in the tree — this blocks the goals-as-wave-tree model. **RESOLVED by #781** (see research outcomes above).

## Next (not yet built)

- **Wave one level out** — split singular wave *identity* (GOAL/MEMORY/agent) from per-repo *execution* (`repos: [RepoWork]`); repo becomes a filter, not a container (item `3-wave-repo-split`). Forks with the tree-spanning cross-repo model in `2-wave-ancestry`.
- Close-the-loop: feed in-flight worker runs + PR state into re-measure.
- Attention as the loop's human-escalation channel for parked interactive steps.
- The canonical always-on wave-agent session; supervisor + heartbeat.
- Fan-out branch/PR isolation before lifting `workers > 1`.
