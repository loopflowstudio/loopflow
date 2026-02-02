# Simplification Opportunities

## Product intent

Loopflow is a step/flow runner for coding agents. Users write markdown steps, compose them into flows, and execute them with `lf` (one-shot) or as daemon-managed waves (continuous). The product wants to be **simple local-first execution with optional persistence for automation**.

## Opportunity 1: Collapse the two RunStore traits

**Misalignment**: The engine's `RunStore` (6 methods, ephemeral runs) and lfd's `RunStore` (25+ methods, persistent waves) share a name but are incompatible interfaces for different concepts.

**Symptom**: `LfCoreStoreAdapter` in `loop_ticker.rs` (180 lines) exists solely to bridge these two interfaces. Every method manually converts between:
- `RunId` ↔ `LfdId`
- `WaveRun` ↔ `Wave`
- `WaveRunStatus` (4 states) ↔ `WaveStatus` (5 states)
- `ForkRunStatus` (engine) ↔ `ForkRunStatus` (lfd)
- `Agent` (engine) ↔ `Agent` (proto)

Status conversion is lossy—`WaveIdle` maps to `Completed`, `WaveError` maps to `Failed`. This semantic gap causes subtle bugs.

**Realignment**: The engine should define **one canonical store interface** that lfd implements. The daemon-specific state (stimuli, pending activations, CI status) lives in a **separate DaemonStore** that composes with the core interface:

```rust
// loopflow-engine: core execution state
pub trait RunStore {
    fn get_run(&self, id: &str) -> Result<Run>;
    fn update_run(&self, run: &Run) -> Result<()>;
    fn create_agent(&self, agent: &Agent) -> Result<()>;
    fn list_fork_runs(&self, run_id: &str, step_index: usize) -> Result<Vec<ForkRun>>;
    // ...
}

// lfd extends for daemon-specific orchestration state
pub trait DaemonStore: RunStore {
    fn list_waves(&self, repo: Option<&str>) -> Result<Vec<Wave>>;
    fn list_stimuli(&self, wave_id: Option<&str>) -> Result<Vec<Stimulus>>;
    fn create_pending_activation(&self, activation: &PendingActivation) -> Result<()>;
    // ...
}
```

**Cascade**:
- Delete `LfCoreStoreAdapter` entirely
- Delete all `flow_status_from_wave`/`wave_status_from_flow` functions
- Delete duplicate `ForkRunStatus` enum from `lfd/src/store/mod.rs`
- Engine's `tick_flow()` works directly with lfd's store without adapter

## Opportunity 2: Merge RunId and LfdId

**Misalignment**: Two identical newtype wrappers for UUID strings exist independently—`RunId` in engine, `LfdId` in lfd.

**Symptom**: Every ID crossing the crate boundary requires explicit parsing:
```rust
let wave_id = LfdId::parse(run_id.as_str())?;  // 15+ occurrences
```

Both types validate UUIDs, implement Display, wrap `String`. Zero code sharing.

**Realignment**: Define `RunId` once in the engine with a proper API:

```rust
// loopflow-engine
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RunId(Uuid);

impl RunId {
    pub fn new() -> Self { Self(Uuid::new_v4()) }
    pub fn parse(s: &str) -> Result<Self, ParseError> { ... }
}
```

lfd uses `RunId` directly—no `LfdId` type.

**Cascade**:
- Delete `lfd/src/id.rs` entirely (~80 lines)
- Delete all `LfdId::parse()` calls throughout lfd
- Proto messages use `string id` but Rust code converts once at the gRPC boundary

## Opportunity 3: Unify status enums

**Misalignment**: Four parallel status hierarchies exist:
- Engine `WaveRunStatus`: Running, Waiting, Completed, Failed
- Proto `WaveStatus`: WaveIdle, WaveRunning, WaveWaiting, WaveError
- Engine `AgentStatus`: Running, Waiting, Completed, Failed
- Proto `AgentStatus`: AgentRunning, AgentWaiting, AgentCompleted, AgentFailed

**Symptom**: `flow_status_from_wave()`, `wave_status_from_flow()`, `map_fork_status()`, `map_fork_status_back()`—four conversion functions for concepts that should be the same type.

Proto adds `WaveIdle` and `WaveError` as daemon-specific states. But `Completed`→`WaveIdle` and `Failed`→`WaveError` mappings leak daemon semantics into engine code.

**Realignment**: Engine defines canonical status. Daemon extends it:

```rust
// loopflow-engine: execution states
pub enum RunStatus {
    Running,
    Waiting,
    Completed,
    Failed,
}

// lfd extends for daemon lifecycle
pub enum WaveStatus {
    Idle,           // daemon-only: between iterations
    Active(RunStatus),  // wraps engine status during execution
}
```

Proto generates into Rust enums, but gRPC handlers convert at the boundary.

**Cascade**:
- Delete all status mapping functions
- Proto becomes a serialization format, not a source of domain types
- Single `match` at gRPC boundary handles all conversion

## Opportunity 4: Remove CLI's InMemoryStore duplication

**Misalignment**: `lf flow` creates an `InMemoryStore` that reimplements `RunStore` for ephemeral local execution. Meanwhile, lfd has full `SqliteStore`/`PostgresStore` implementations.

**Symptom**: `lf/src/commands/flow.rs` contains 90 lines of `InMemoryStore` that duplicates what lfd's stores already do. The engine already defines the trait; we just keep reimplementing it.

**Realignment**: Engine provides an `InMemoryStore` reference implementation. CLI and tests use it directly:

```rust
// loopflow-engine (not lf)
pub mod store {
    pub trait RunStore { ... }
    pub struct InMemoryStore { ... }  // reference implementation
}
```

**Cascade**:
- Delete `InMemoryStore` from `lf/src/commands/flow.rs`
- Engine tests use the same in-memory store
- lfd's stores implement the trait; no new code

## Opportunity 5: Proto as wire format, not domain model

**Misalignment**: lfd uses generated proto types (`Wave`, `Agent`, `Stimulus`) as internal domain objects. These carry proto baggage (optional fields as `Option<Timestamp>`, `i32` for enums) throughout business logic.

**Symptom**: Throughout lfd:
```rust
wave.status != WaveStatus::WaveRunning as i32  // compare i32, not enum
stimulus.enabled  // proto bool, no type safety
agent.started_at.map(|t| t.seconds)  // unwrap Timestamp wrapper
```

Proto types leak into store trait signatures, loop logic, scheduler logic.

**Realignment**: Define internal domain types in Rust. Convert at gRPC/HTTP boundaries only:

```rust
// lfd internal types
pub struct Wave {
    pub id: RunId,
    pub status: WaveStatus,  // real enum
    pub consecutive_failures: u32,  // not i32
    pub created_at: time::OffsetDateTime,  // not Timestamp
}

// conversion only at proto boundary
impl From<proto::Wave> for Wave { ... }
impl From<Wave> for proto::Wave { ... }
```

**Cascade**:
- Clean Rust types throughout lfd business logic
- Proto changes don't ripple into domain code
- Type safety for enums, timestamps, IDs
- Store trait uses domain types, not proto types

## Aligned areas

**Engine's flow execution model**: `tick_flow()` with `StepRunner` trait injection is well-aligned. The state machine is clear, the runner abstraction enables testing.

**Agent launching**: `launch_agent()` cleanly abstracts over Claude/Codex/Gemini CLIs. The `LaunchConfig` structure matches product needs.

**Context gathering**: `gather_context()` with `GatherContextOpts` matches the CLI's needs directly. No translation layer.

**Scheduler slot management**: lfd's `Scheduler` with semaphore-based slots is simple and correct for the problem.

**gRPC/HTTP separation**: lfd separates gRPC control plane from HTTP health/metrics cleanly.

## Priority order

1. **Merge ID types** (Opportunity 2) — smallest change, immediate cleanup
2. **Unify status enums** (Opportunity 3) — removes conversion functions
3. **Collapse RunStore traits** (Opportunity 1) — biggest impact, removes adapter layer
4. **Remove InMemoryStore duplication** (Opportunity 4) — cleanup after #3
5. **Proto as wire format** (Opportunity 5) — larger refactor, do after others stabilize
