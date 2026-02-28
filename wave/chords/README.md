# Chords

## Vision

Named groups of waves with inter-wave listening. Waves are flat. Chords group them. Listening connects them. Not nested orchestration, not multi-user coordination, not approval routing.

## Strategy

The original plan called for a recursive `Wave` enum with nested execution and inherited triggers. Building Phase 01 revealed this was over-engineered — users want named groups and inter-wave reactivity, not a tree-structured scheduler.

The pivot: flatten `Wave` to a single struct, use join tables for grouping, and add `Signal::Listen` with `source_wave_id` for coordination.

**Invariants:**

- Waves are flat — a `Wave` is a single struct, no enum/tree/parent. A wave belongs to exactly one chord (including the default).
- Chords are groups, not executors — no trigger ownership or child lifecycle management.
- Signals are reactive — external events (Watch, Listen, Cron, CiFailure) that trigger flow overrides. Loop/Once are execution modes on the wave, not signals.
- Every wave gets default stimuli: Watch (flow: integrate) and CiFailure (flow: ci-fix).
- Stimulus-based composition over tree structure — a wave can listen to any other wave regardless of chord membership.

### Signal model

Phases 01–03.5 collapsed `WaveRunKind`/`StimulusKind` into a single `Signal` enum. Phase 04 splits further: execution modes (Loop, Once) move to `wave.mode`, leaving Signal as purely reactive triggers. Starting a wave dispatches directly — no "manual stimulus" concept.

```
Signal (reactive, external events):    Watch | Listen | Cron | CiFailure
Wave (execution behavior):             mode (Loop|Once), flow, loop_flow
```

CI fix is a normal stimulus activation (`Signal::CiFailure` + `flow: ci-fix`). Watch triggers the `integrate` flow (rebase + integrate-upstream). `stimulus.flow` override lets any stimulus select a flow at activation time.

Starting a wave = running `wave.flow` (default: `ship`). The loop ticker runs `wave.loop_flow` (default: `ship-roadmap`). Callers can override flow at start time via the API.

External API still uses `stimulus.kind` — coordinated rename to `stimulus.signal` deferred (requires Python client + Concerto + wave config schema update in lockstep).

### Data model

```rust
struct Wave {
    id: LfdId,
    name: String,
    repo: String,
    mode: WaveMode,        // Loop | Once
    flow: String,          // default flow for manual starts ("ship")
    loop_flow: String,     // flow for loop ticker ("ship-roadmap")
    // flat fields, no type/parent/position
}

struct Stimulus {
    signal: Signal,  // Watch | Listen | Cron | CiFailure
    flow: Option<String>,  // override wave.flow for this activation
    source_wave_id: Option<LfdId>,  // for Listen stimuli
}

// WaveRun is an iteration container — owns branch, worktree, PR.
struct WaveRun {
    id: LfdId,
    wave_id: LfdId,
    iteration: u32,
    repo: String,
    branch: String,
    worktree: String,
    pr: Option<PullRequest>,
    loop_flow_run: FlowRun,           // primary flow execution
    triggered_flows: Vec<FlowRun>,    // reactive: integrate, ci-fix, etc.
}

// FlowRun is one execution of a specific flow.
struct FlowRun {
    id: LfdId,
    wave_run_id: LfdId,
    flow: String,
    step_index: u32,
    direction: Vec<String>,
    area: Vec<String>,
    activation_log_id: Option<LfdId>,
    status: FlowRunStatus,
}
```

```sql
CREATE TABLE chords (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    is_default INTEGER NOT NULL DEFAULT 0,
    created_at BIGINT NOT NULL
);

CREATE TABLE chord_members (
    chord_id TEXT NOT NULL REFERENCES chords(id) ON DELETE CASCADE,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    PRIMARY KEY (chord_id, wave_id)
);
```

## Goals

- Signal cleanup: execution modes (Loop/Once) separated from reactive triggers (Watch/Listen/Cron/CiFailure)
- WaveRun/FlowRun split: WaveRun is an iteration (branch, worktree, PR); FlowRun is one flow execution within it
- Starting a wave = running a flow. No manual stimulus. `lfq run` accepts `--flow`.
- Default stimuli on wave creation: Watch (flow: integrate) and CiFailure (flow: ci-fix)
- Integrate flow: rebase + integrate-upstream step keeps waves current with main
- Chord CRUD works end-to-end: create chord, add/remove waves, list members, delete chord
- Listen stimulus fires reliably when source wave completes
- Concerto shows chord grouping as sidebar sections with progressive disclosure
- HTTP API and Python client support chord operations

## Risks

- **Listen fan-out.** Many waves listening to one source triggers N runs simultaneously. No concurrency limiting today. Acceptable at current scale; revisit if fan-out exceeds scheduler capacity.
- **CI recursion guard coupling.** Recursion prevention keys off `flow_run.flow == "ci-fix"`. Renaming the flow without updating the guard reintroduces recursion.
- **Concurrent CI stimulus creation.** CI failure trigger resolves-then-creates `Signal::CiFailure` stimuli. Serialized today by one event loop; needs uniqueness guard if parallelized.
- **Run worktree accumulation.** Parallel runs create per-run worktrees cleaned up on completion. Daemon crash mid-run leaves orphans until janitor sweep.
- **Integrate-upstream false positives.** Watch triggers on any main advance, even when upstream changes are irrelevant to the wave. The integrate-upstream step should no-op quickly in that case, but it still costs an agent invocation.

## Metrics

- Chord CRUD works end-to-end from Python client and HTTP API
- Listen stimulus fires reliably when source wave completes (including edge cases: source fails, source is stopped)
- Concerto UI shows chord grouping with progressive disclosure (flat when one chord, sections when multiple)
- Default stimuli present on every new wave (watch + ci-fix)
- Integrate flow runs automatically when main advances
- `lfq run <wave> --flow <flow>` starts a wave with the specified flow
- Reactive stimuli during an active iteration create triggered FlowRuns, not new WaveRuns
- WaveRun history shows loop_flow_run and triggered_flows per iteration
