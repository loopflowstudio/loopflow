# goals wave memory

Steers Loopflow toward persistent Goal-driven Waves: goals as the authored loop prompt, Asana as the live roadmap, and Concerto as the session surface.

## Shipped (runtime model foundation)

- **Two-file wave surface** — `wave/<name>/` is `GOAL.md` (intent) + `MEMORY.md` (this file). Both are injected into the wave loop's assembled context, so the agent reads its intent and memory each iteration.
- **Wave / Run / Session** — the lfd runtime model was reduced to three product nouns. `WaveRun` + `AgentRun` collapsed into `Run` (execution/result lineage, flattened — no more `WaveRunSnapshot`). `TerminalSession` + the old conversation session collapsed into `Session` (attachable live control surface). `AgentLaunch` and the launch-envelope DTOs are gone: launching returns the durable `Session`.
- **Session `use`** — `wave_agent | worker | palette` lives on the session, not inferred from a nullable task/run. Role is read off `Session.use`, not `(source, wave_run_id)`.
- **lfq as the runtime surface** — `lfq wave run` ensures a wave-agent `Session`; `lfq worker run` creates a `Run` + linked worker `Session` and spawns the work; `lfq sessions` / `lfq attach <id>` list and attach live sessions over tmux. This replaces the old `/dispatch` route and `lf op dispatch`.
- **Goal primitive** — `goal` is the third prompt primitive (step/flow/**goal**). The durable `Wave` carries a required `goal: String` (default `ship-roadmap`) alongside `primary_flow`. `load_goal` resolves `.lf/goals/<name>.md` repo→home→builtin (legacy singular `.lf/goal/` and repo-root `goal/` do not resolve); the wave loop body (`lfd/executor/wave/mod.rs`) runs `wave.goal` as its iteration prompt via `render_goal`, which exposes available flows, a roadmap handle, metrics, memory, and in-flight dispatches — so the goal prompt decides its next move and dispatches inner work through `lfq worker run`.
- **Demo** — `scripts/demo_waveagent.sh` renders the goals wave prompt and shows MEMORY.md reaching context.

## Open regression

- **Wave ancestry dropped.** The reduction removed the parent-wave field from the durable `Wave` type, so `WaveAgentTree.child_waves` is always empty and the chord structure is invisible. The store still has `parent_wave_id` columns. Reintroduce ancestry before chords/child-waves can appear in the tree — this blocks the goals-as-chord model. See item `2-wave-ancestry`.

## Next (not yet built)

- **Wave one level out** — split singular wave *identity* (GOAL/MEMORY/agent) from per-repo *execution* (`repos: [RepoWork]`); repo becomes a filter, not a container (item `3-wave-repo-split`). Forks with the chord-spanning cross-repo model in `2-wave-ancestry`.
- **`lf goal` thin-call cleanup** — the local `lf goal` command still renders (`render_goal`) and launches the session locally; reduce it to a thin call into the lfd-backed wave-agent session API (`lfq wave run`) so the runtime owns rendering/launch. Minor.
- Close-the-loop: feed in-flight worker runs + PR state into re-measure.
- Attention as the loop's human-escalation channel for parked interactive steps.
- The canonical always-on wave-agent session; supervisor + heartbeat.
- Fan-out branch/PR isolation before lifting `workers > 1`.
