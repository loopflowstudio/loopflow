# goals wave memory

Steers Loopflow toward persistent Goal-driven Waves: goals as the authored loop prompt, Asana as the live roadmap, and Concerto as the session surface.

## Shipped (runtime model foundation)

- **Two-file wave surface** — `wave/<name>/` is `GOAL.md` (intent) + `MEMORY.md` (this file). Both are injected into the wave loop's assembled context, so the agent reads its intent and memory each iteration.
- **Wave / Run / Session** — the lfd runtime model was reduced to three product nouns. `WaveRun` + `AgentRun` collapsed into `Run` (execution/result lineage, flattened — no more `WaveRunSnapshot`). `TerminalSession` + the old conversation session collapsed into `Session` (attachable live control surface). `AgentLaunch` and the launch-envelope DTOs are gone: launching returns the durable `Session`.
- **Session `use`** — `wave_agent | worker | palette` lives on the session, not inferred from a nullable task/run. Role is read off `Session.use`, not `(source, wave_run_id)`.
- **lfq as the runtime surface** — `lfq wave run` ensures a wave-agent `Session`; `lfq worker run` creates a `Run` + linked worker `Session` and spawns the work; `lfq sessions` / `lfq attach <id>` list and attach live sessions over tmux. This replaces the old `/dispatch` route and `lf op dispatch`.
- **Wave carries `goal`** — the durable `Wave` type has a required `goal: String` field (defaulting to `ship-roadmap`) alongside `primary_flow`. The field exists; the `.lf/goals/` resolver and goal-as-loop-body do not yet (item `1-goal-primitive`).
- **Demo** — `scripts/demo_waveagent.sh` renders the goals wave prompt and shows MEMORY.md reaching context.

## Open regression

- **Wave ancestry dropped.** The reduction removed the parent-wave field from the durable `Wave` type, so `WaveAgentTree.child_waves` is always empty and the chord structure is invisible. The store still has `parent_wave_id` columns. Reintroduce ancestry before chords/child-waves can appear in the tree — this blocks the goals-as-chord model. See item `2-wave-ancestry`.

## Next (not yet built)

- `.lf/goals/` resolver + loop body that runs `wave.goal` as the iteration prompt (item `1-goal-primitive`).
- Close-the-loop: feed in-flight worker runs + PR state into re-measure.
- Attention as the loop's human-escalation channel for parked interactive steps.
- The canonical always-on wave-agent session; supervisor + heartbeat.
- Fan-out branch/PR isolation before lifting `workers > 1`.
