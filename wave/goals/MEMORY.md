# goals wave memory

Steers Loopflow toward persistent Goal-driven Waves: goals as the authored loop prompt, Asana as the live roadmap, and Concerto as the session surface.

## Shipped (runtime model foundation)

- **Two-file wave surface** — `wave/<name>/` is `GOAL.md` (intent) + `MEMORY.md` (this file). Both are injected into the wave loop's assembled context, so the agent reads its intent and memory each iteration.
- **Wave / Run / Session** — the lfd runtime model was reduced to three product nouns. `WaveRun` + `AgentRun` collapsed into `Run` (execution/result lineage, flattened — no more `WaveRunSnapshot`). `TerminalSession` + the old conversation session collapsed into `Session` (attachable live control surface). `AgentLaunch` and the launch-envelope DTOs are gone: launching returns the durable `Session`.
- **Session `use`** — `wave_agent | worker | palette` lives on the session, not inferred from a nullable task/run. Role is read off `Session.use`, not `(source, wave_run_id)`.
- **lfq as the runtime surface** — `lfq wave run` ensures a wave-agent `Session`; `lfq worker run` creates a `Run` + linked worker `Session` and spawns the work; `lfq sessions` / `lfq attach <id>` list and attach live sessions over tmux. This replaces the old `/dispatch` route and `lf op dispatch`.
- **Goal primitive** — `goal` is the third prompt primitive (step/flow/**goal**). The durable `Wave` carries a required `goal: String` (default `ship-roadmap`) alongside `primary_flow`. `load_goal` resolves `.lf/goals/<name>.md` repo→home→builtin (legacy singular `.lf/goal/` and repo-root `goal/` do not resolve); the wave loop body (`lfd/executor/wave/mod.rs`) runs `wave.goal` as its iteration prompt via `render_goal`, which exposes available flows, a roadmap handle, metrics, memory, and in-flight dispatches — so the goal prompt decides its next move and dispatches inner work through `lfq worker run`.
- **Demo** — `scripts/demo_waveagent.sh` renders the goals wave prompt and shows MEMORY.md reaching context.
- **`lf wave <name>` — progress arm shipped.** Foreground, non-terminating command (`rust/loopflow/src/lf/commands/loop.rs`, `loop` is an alias) that runs a wave's outer loop deterministically in Rust: each pass is one bounded `lf -b goal <wave> --once`, and it fires the next pass the instant the last exits — no timer, gated only on the inner pass finishing. Stops on Ctrl-C or `wave/<wave>/STOP`. Failed passes get a 3s cooldown so a broken inner run can't hot-spin. Inherits the terminal (`Command::status()`) — the earlier stdout/stderr tee to `wave/*/streams/` was cut as dead capture (nothing read it; inner `lf -b goal` already writes durable logs under the agent log dir). Re-add stream capture *with* the monitor consumer that needs it. `lf goal -b` now launches through the shared headless `launch_agent` runner instead of an interactive session.

## lf wave runtime — the design ahead (loopflow owns the outer loop)

The shipped progress arm is the crudest slice of a larger runtime. The vision: `lf wave` is the **one place loopflow owns a custom harness** — the deterministic outer loop — while every agent pass underneath is a bounded vendor-harness run. It fixes the "model owns the loop → gets stuck (declares victory early, spins, loses the thread)" failure by reclaiming the *outer* loop. **lfd is an absorb-target, not a dependency**: `lf wave` hosts the whole runtime in-process; today's `lfd/triggers/{loop_ticker,cron}.rs` + the dormant mailbox migrate *into* it, they aren't called across a process boundary. "Detached" = the same process with no terminal (what lfd used to be). `lf goal` / `/goal` stays untouched — `lf wave` is additive and dispatches `/goal --once` as its inner unit.

**Four arms, three shapes, all coordinating only through `MEMORY.md`** (no arm calls another):
- **Pass launcher** — progress + crons are the *same mechanism*, differ only in trigger policy: repeat-on-finish (progress, shipped) vs scheduled cron expr (maintenance: orient-daily · scan-changes · rebase). Each worker gets a tmux handle (human attach/steer) **and** a tee'd out/err stream (monitor input) — the two are independent.
- **Monitor** — reads workers' clean batch-mode stream logs (never parses dirty tmux scrollback), runs a summarizer/judge that distills what's *relevant*, forwards it + a standing summary into chat. Distinct from the killed `evaluate`: that judged loop control (cut — the loop just repeats); this judges output relevance for a human. **Open crux: its cadence** (tick every N sec vs on stream-append) and cost — it's an LLM pass per tick, needs a cheap trigger not a hot spin.
- **Chat API** — in-process HTTP/WS + mailbox, the one arm that returns a reply. Answers from the monitor's standing summary + MEMORY (skips heavy orientation), dispatches a solution thread if the ask needs work, drops steering into the mailbox for the next progress pass. Dispatch-and-return, never holds a long session. The dormant mailbox is its data layer: `ChatMessage`/`ChatMemoryBlock` DTOs, migrations 007/008, WS-inbound scaffolding all exist with zero routes — revival, not greenfield.

**Two-tier memory:** rolling window (volatile recent chat + run summaries — felt continuity; **v1 = full window, no eviction**) + `MEMORY.md` (durable, distilled — the source of truth). **Invariant: correctness never depends on the window** — it's a hot cache so the agent doesn't re-read MEMORY cold each pass; eviction (token/time) is a pure performance add, safe to defer and dumb. Roll is mechanical; distilling into MEMORY is part of what the single `lf` pass is asked to do (cheapest place — it already has the window).

**Inner-loop prompt doctrine** — conductor, not player: orient (mostly cached in MEMORY, refresh only what's stale) → act. Spine is a value chain: **clarify → real user wins → what blocks them → ruthlessly prioritize** the single highest-leverage blocker, dispatch through the `lf` API, scale breadth to budget. The *only* inversion vs today's `LOOPFLOW_OPERATING_PROMPT`: "do one orient-to-action pass and stop" (loopflow owns loop) instead of "keep dispatching until done" (model owns loop). Clarify gate: attached → ask and block; headless → assume + log to `scratch/questions.md`. Never declare done to escape a hard step — report `blocked`. Old `LOOPFLOW_OPERATING_PROMPT` (flow.rs) + the removed LOOPFLOW.md converge into this one doctrine. Each pass closes with a light `<lf:pass-result>` summary (integrated/dispatched/blocker/next/metric) inside its own stream — not a beacon; the monitor reads it like any other stream text.

**v1 write discipline (deferred to its own task):** single writer — only the progress pass writes MEMORY; chat and crons append to a mailbox the progress pass drains. No locks. Revisit only if it bites.

## Open regression

- **Wave ancestry dropped.** The reduction removed the parent-wave field from the durable `Wave` type, so `WaveAgentTree.child_waves` is always empty and the chord structure is invisible. The store still has `parent_wave_id` columns. Reintroduce ancestry before chords/child-waves can appear in the tree — this blocks the goals-as-chord model. See item `2-wave-ancestry`.

## Next (not yet built)

- **lf wave — remaining three arms.** Progress shipped; monitor, cron, chat still open (revival + rewiring, not greenfield — see the runtime section above). Monitor's summarizer cadence/cost is the real crux. Re-add worker stream capture together with the monitor that consumes it. Supersedes/absorbs "canonical always-on wave-agent session" below.
- **Wave one level out** — split singular wave *identity* (GOAL/MEMORY/agent) from per-repo *execution* (`repos: [RepoWork]`); repo becomes a filter, not a container (item `3-wave-repo-split`). Forks with the chord-spanning cross-repo model in `2-wave-ancestry`.
- **`lf goal` thin-call cleanup** — the local `lf goal` command still renders (`render_goal`) and launches the session locally; reduce it to a thin call into the lfd-backed wave-agent session API (`lfq wave run`) so the runtime owns rendering/launch. Minor.
- **`launch_agent` dedup** — `goal.rs::launch_goal_batch` duplicates the headless-launch sequence in `run.rs` (check_cli → write prompt/context logs → `StreamFormat::Human` → `launch_agent` → exit-code hint). Collapse into a shared `engine::agent` helper once a third caller appears.
- Close-the-loop: feed in-flight worker runs + PR state into re-measure.
- Attention as the loop's human-escalation channel for parked interactive steps.
- Fan-out branch/PR isolation before lifting `workers > 1`.

## Roadmap reconciliation owed (Asana)

The roadmap lives in Asana (project `1216257471889000`, adopted from main during rebase; branch's duplicate `1216272792262792` was abandoned). This branch could **not** reconcile it — the stored Asana token was expired in a headless run (`lf op auth asana` needed). When a human can re-auth, via `lf op pm update`:
- **File the lf wave follow-ons** — the three remaining arms (monitor, cron, chat). The branch-added local `wave/goals/*lf-loop*.md` roadmap files were removed at gate (Asana is source of truth); those items were never registered in the canonical project, so they must be filed fresh.
- **Close** whatever the shipped `lf wave` progress arm satisfies.
