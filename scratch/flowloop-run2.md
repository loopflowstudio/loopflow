# flowloop run 2 — one runtime, three tiers, mind retired

Run 1 built an instance (`flowloop/task.rs`, `lf task`) and called it "the
first flowloop." The vision (charter `scratch/flowloop.md`; design session
2026-07-07) is a **runtime**: everything agentic in loopflow is the same
looping flow — `clarify → pursue_goal → mutate` — differing only on what it
owns and what oracle halts it. Run 1's slicing was too conservative; this run
builds the vision, deferring only polish (explicitly listed in §7).

Three commitments, none optional:

1. **One runtime.** `flowloop/` owns the pass runner, the oracles, and all
   three tier drivers — wave included.
2. **The wave runs as a flowloop.** Its persistent-vendor-thread mind is
   replaced; the residency's scheduler and chat surface remain as the shell.
3. **`mind` → `flowloop` everywhere in the code.** Names, comments, docs.
   "Mind" survives only in git history.

The shipped `lf task` behavior survives (its tests keep passing), refit onto
the shared runtime.

## 1. The runtime

```rust
// flowloop/mod.rs
pub enum Tier { Wave, Project, Task }
impl Tier {
    pub fn pass_flow(&self) -> &'static str  // "wave-pass" | "project-pass" | "task-pass"
    pub fn oracle(&self) -> Oracle           // Never | KrSetDone | PrMerged
}

// flowloop/pass.rs — one bounded, headless run of the tier's 3-phase flow
// in a worktree. Extracted from run 1's run_task_pass + run_with_timeout.
pub struct PassOptions { pub timeout: Duration, pub max_turns: Option<u32> }
pub fn run_pass(worktree: &Path, tier: Tier, seed: &str, opts: &PassOptions) -> OpsResult<PassOutcome>

// flowloop/oracle.rs — deterministic halt predicates (charter §2).
// The flowloop chooses moves; it never decides it is done.
pub enum Oracle {
    PrMerged,   // task — gh pr view (extracted from run 1's poll_pr_oracle)
    KrSetDone,  // project — all kr-labeled Linear items completed
    Never,      // wave — the loop is the point
}
```

Drivers, one per tier, all in `flowloop/`:

- `flowloop/task.rs` — **refit** onto `run_pass` + `Oracle::PrMerged`.
  Sequential loop: pass → poll → sleep-or-pass → caps. Behavior identical.
- `flowloop/project.rs` — **new** (§4). Same sequential shape.
- `flowloop/wave.rs` — **the converted mind** (§2). Same runtime, richer wake
  source: instead of "loop until oracle," passes fire from the residency's
  select (inbox / heartbeat / cron). One runtime; the wave's driver plugs it
  into an event-driven scheduler instead of a polling loop. `Oracle::Never`
  means no terminate arm — mutate's living arms (update GOAL.md, launch
  sub-waves, reset, split) live in `wave_mutate`'s skill text.

## 2. The wave is a flowloop

Today `wave/mind.rs` keeps one persistent vendor thread scheduled by a biased
select (inbox / harness events / heartbeat / cron / interrupt deadlines),
surfacing `ResidentDelta`s through the listener. The conversion: **keep the
scheduler and the shell, replace the turn.** A wake runs one `wave-pass` flow
(`wave_clarify → wave_pursue → wave_mutate`) as a bounded headless child in
the wave home — exactly how a task pass runs. Continuity is GOAL.md + MEMORY +
chat journal riding context assembly every pass (`<lf:wave-memory>`,
`<lf:wave-chat-recent>`): log-as-truth, the already-decided model.

Why this costs less than it looks:

- **Vendor-thread resume is already broken on codex** — "every boot is a cold
  start" (`mind.rs:59`). The visible thread is already the listener's fold.
- **Steer-mid-turn is codex-only** (`harness/codex.rs:552`; claude/opencode:
  `supports_steer: false`). Two of three harnesses already queue-and-fold at
  the boundary; the conversion makes that path universal, which is already
  the doctrine (charter §3b: chat folds at phase boundaries).
- **Interrupt simplifies to killing the pass child.** The cooperative-cancel
  + `INTERRUPT_DEADLINE` + force-close machinery goes away.

**Wire seam** (the real engineering): deltas today come from the in-process
`EventAdapter`. With the turn in a child process, the wave driver feeds the
wire coarsely — `TurnOpened` on wake, child output streamed as content
deltas, `TurnFinished` with the pass's reply text posted to the thread.
Per-tool-call streaming fidelity is **deferred polish** (§7), not a blocker:
chat and Concerto still see turns open, progress, and close.

Heartbeat: a pass is three harness invocations, so the 300s-idle heartbeat
triples in cost. Coarsen `HEARTBEAT_IDLE` for pass-based waves (hours, not
minutes); messages and cron carry the real cadence. Charter says the wave
mutates at ~24h scale — the 5-minute nudge was a persistent-session artifact.

**No dual mode.** Live waves flip with this PR. One implementation; git is
the history. Listener, supervisor, journal, wire protocol: untouched apart
from the rename sweep.

## 3. mind → flowloop everywhere

The rename is a commitment of the design ("fix the mind/loop wording in all
the code"), not cosmetics. The noun is flowloop; a flowloop is a flow that is
looped.

- `wave/mind.rs` → `flowloop/wave.rs` (the driver; charter §4.1's
  `wave/flowloop.rs` target predates the `flowloop/` module — updated here)
- `run_mind` → `run_flowloop`; `MindEnd` → `FlowloopEnd`;
  `MindConfig` → `FlowloopConfig`
- `--no-mind` / `--mind-only` flags → `--no-flowloop` / `--flowloop-only`
  (no back-compat aliases; internal surface)
- Comment/doc sweep: every "mind" in `wave/`, `lfd/`, READMEs, `docs/lf.md`
- `MindState` (wire DTO, three-language mirror + fixtures) →
  `FlowloopState`: in scope as the **last slice**; it rides only if the
  Swift/fixture churn fits the run — the one rename allowed to slip to a
  follow-up PR (§7).

## 4. The project tier

Built and tested in this run; not yet wired into the product surface (no CLI
verb) — the wiring arrives when the wave starts spawning projects.

- **KR set** = items in the wave's Linear project labeled `kr` (minimal
  answer to charter open Q1; revisit when projects get their own Linear
  container).
- **`Oracle::KrSetDone`** = every kr-labeled item `completed` (via
  `pm_show`). **An empty KR set refuses to start** — no KRs is a clarify
  problem, not vacuous completion.
- Driver: pass → poll → caps → `lf chat --parent` on exhaustion, same shape
  as task. The renew arm (self-renewing KRs) is `project_mutate` skill text,
  not runner code.
- `project_pursue` decomposes KRs into Linear tasks and may dispatch
  `lf task <id>` — the command exists, so the tier is semi-functional through
  skill text alone. The runner does not manage children in this run.

## 5. The full 3×3 skill matrix

Six new builtin steps + two flows complete the matrix beside run 1's `task_*`
trio. Phase names are the charter's: **clarify, pursue_goal, mutate.**

- `flow/wave-pass.yaml`: `wave_clarify → wave_pursue → wave_mutate`
- `flow/project-pass.yaml`: `project_clarify → project_pursue → project_mutate`
- `wave_clarify` — GOAL.md is the artifact: make the objective computable;
  interactive branch = ask the thread.
- `wave_pursue` — delegate: launch project/task flowloops for planned work;
  run execs directly only for hot/now problems.
- `wave_mutate` — update GOAL.md from learnings, launch/retire sub-waves,
  curate memory; never terminates.
- `project_clarify` — the KR set: make each KR measurable.
- `project_pursue` — decompose KRs → file/dispatch tasks; file discovered
  debt as new tasks.
- `project_mutate` — retire milestone KRs, renew self-renewing ones; the
  oracle (all KRs done) terminates.

Each skill states its tier's artifact, move menu, and oracle concretely —
tier behavior lives in skill text, not runtime branching.

## 6. Build order, demo, done-when

1. **Extract** `flowloop/pass.rs` + `flowloop/oracle.rs` from `task.rs`;
   refit the task driver. No behavior change; existing tests green.
2. **Project tier**: driver + `KrSetDone` + skills + tests.
3. **Wave conversion**: `flowloop/wave.rs`, turn = pass, coarse wire,
   kill-on-interrupt, heartbeat coarsening.
4. **Rename sweep**: code, flags, comments, docs. `MindState` wire rename
   last, may split off.
5. Sync charter + READMEs; retire superseded scratch docs.

**Demo:** a live wave running on passes — send it a chat message, watch a
wave-pass fire (three phases in the run log), see the reply land in the
thread. Plus run 1's `lf task` demo, unchanged.

**Done when:** the demo above; `grep -ri '\bmind\b' rust/loopflow/src` returns
nothing meaningful; `cargo test flowloop` covers tier→flow binding, project
oracle (all-done / some-open / empty-set-refuses), pass timeout kill; `lf
task` tests untouched and green.

## 7. Deferred polish (explicit, not silent)

- Per-event delta streaming from the pass child (wire fidelity); coarse
  open/stream/close ships now.
- `MindState` → `FlowloopState` wire rename may split into a follow-up PR if
  the three-mirror churn crowds this one.
- Codex live steer (fold-at-boundary becomes universal; revisit only if
  boundary-folding proves too slow in practice).
- `lf project` CLI verb / wave-spawns-projects wiring.
- Heartbeat cadence tuning beyond the initial coarsening.
- KR representation richer than the `kr` label (charter open Q1).

## 8. Session provenance

Design conversation: Claude session `be857d6e` (project dir
`-Users-jack-src-loopflow-goal-md-research-worktreeworkers`), 2026-07-06/07.
Load-bearing moments: tier table (17:13Z), KRs-per-project + self-renewal
(17:20Z), wave = objective only (17:21Z), execs-for-hot-problems (17:27Z),
`terminate()` inside `mutate()` (17:29–30Z), phases as `-b` skills + chat as
the only interface (17:33Z), "clarify / pursue_goal / mutate is a flowloop"
(17:36Z), tier known in advance → targeted skills (17:37Z), renames are the
point (17:11Z), "your slicing was likely too conservative" (02:01Z).
