# Chords

## Vision

Named groups of waves with inter-wave listening. Waves are flat. Chords group them. Listening connects them. Not nested orchestration, not multi-user coordination, not approval routing.

## Strategy

The original plan called for a recursive `Wave` enum with nested execution and inherited triggers. Building Phase 01 revealed this was over-engineered — users want named groups and inter-wave reactivity, not a tree-structured scheduler.

The pivot: flatten `Wave` to a single struct, use join tables for grouping, and add `Signal::Listen` with `source_wave_id` for coordination.

**Invariants:**

- Waves are flat — a `Wave` is a single struct, no enum/tree/parent. A wave belongs to exactly one chord (including the default).
- Chords are groups, not executors — no trigger ownership or child lifecycle management.
- Signals are reactive — external events (Watch, Listen, CiFailure) that trigger flow overrides. Loop/Cron/Manual are execution modes on the wave, not signals.
- Every wave gets default stimuli: Watch (flow: integrate) and CiFailure (flow: ci-fix).
- Stimulus-based composition over tree structure — a wave can listen to any other wave regardless of chord membership.

### Signal model

Phases 01–03.5 collapsed `WaveRunKind`/`StimulusKind` into a single `Signal` enum. Signal cleanup split further: execution modes (Loop, Cron, Manual) moved to `wave.mode`, leaving Signal as purely reactive triggers. Starting a wave dispatches directly — no "manual stimulus" concept.

```
Signal (reactive, external events):    Watch | Listen | CiFailure
Wave (execution behavior):             mode (Loop|Cron|Manual), primary_flow
```

CI fix is a normal stimulus activation (`Signal::CiFailure` + `flow: ci-fix`). Watch triggers the `integrate` flow (rebase + integrate-upstream). `stimulus.flow` override lets any stimulus select a flow at activation time.

Starting a wave runs `wave.primary_flow` (default: `ship-roadmap`). Callers can override flow at start time via the API — the override is ephemeral, not saved on the wave.

External API still uses `stimulus.kind` — coordinated rename to `stimulus.signal` deferred (requires Python client + Concerto + wave config schema update in lockstep).

### Data model

```rust
struct Wave {
    id: LfdId,
    name: String,
    repo: String,
    mode: WaveMode,        // Loop | Cron | Manual
    primary_flow: String,  // the wave's flow ("ship-roadmap")
    cron: Option<String>,  // cron expression, required when mode=Cron
    // flat fields, no type/parent/position
}

struct Stimulus {
    signal: Signal,  // Watch | Listen | CiFailure
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
    primary_flow_run: FlowRun,        // primary flow execution
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

- WaveRun/FlowRun split: WaveRun is an iteration (branch, worktree, PR); FlowRun is one flow execution within it
- Reactive stimuli during an active iteration create triggered FlowRuns, not new WaveRuns
- Integrate flow: rebase + integrate-upstream step keeps waves current with main
- Concerto shows chord grouping as sidebar sections with progressive disclosure

## Risks

- **Listen fan-out.** Many waves listening to one source triggers N runs simultaneously. No concurrency limiting today. Acceptable at current scale; revisit if fan-out exceeds scheduler capacity.
- **CI recursion guard coupling.** Recursion prevention keys off `flow_run.flow == "ci-fix"`. Renaming the flow without updating the guard reintroduces recursion.
- **Concurrent CI stimulus creation.** CI failure trigger resolves-then-creates `Signal::CiFailure` stimuli. Serialized today by one event loop; needs uniqueness guard if parallelized.
- **Run worktree accumulation.** Parallel runs create per-run worktrees cleaned up on completion. Daemon crash mid-run leaves orphans until janitor sweep.
- **Integrate-upstream false positives.** Watch triggers on any main advance, even when upstream changes are irrelevant to the wave. The integrate-upstream step should no-op quickly in that case, but it still costs an agent invocation.
- **Iteration counter cumulative across cycles.** `wave.iteration` increments with every WaveRun and never resets between cron cycles. `cycle_start_iteration` tracks the start of each cycle for the `max_iterations` safety valve. Low urgency since `max_iterations` defaults to `None`.
- **Branch sub-flow items silently skipped.** The branch executor only handles Step items — forks or nested branches in a branch path's flow are silently ignored. Fork-in-branch is prevented at parse time, but a branch path pointing to a flow containing a fork would skip it.
- **Cron `last_triggered` tracking lost.** Signal cleanup removed `last_triggered_at` from the stimulus. Cron waves attempt activation on every 30-second tick, relying on the dedup layer to coalesce. Correct but noisy — add `last_cron_triggered_at` to the wave when FlowRun lands.
- **No cron active-run check.** Unlike the loop ticker (which checks for active runs before dispatching), the cron ticker always dispatches and relies on activation-layer dedup. Correct but less efficient.

## Metrics

- Listen stimulus latency: seconds from source wave completion to triggered run start (target: <5s)
- % of CI failures that auto-trigger ci-fix flow without manual intervention (target: 100%)
- Number of orphaned FlowRuns per week (triggered but never completed) (target: 0)
- Integrate flow no-op rate: % of Watch triggers where upstream changes are irrelevant to the wave (track to calibrate filtering)
