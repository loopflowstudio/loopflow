# goals wave memory

Steers Loopflow toward persistent Goal-driven Waves: goals as the authored loop prompt, Asana as the live roadmap, and Concerto as the session surface.

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
