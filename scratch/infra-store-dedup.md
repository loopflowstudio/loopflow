# Store layer deduplication

## Problem

The store has two backends (SQLite and Postgres) with identical business logic and different async syntax. Composite operations — `join_waves` (70 lines, 5 match arms), `leave_wave`, `create_wave` (recursive tree walk) — are copy-pasted verbatim across `sqlite.rs` and `postgres.rs`. As new chord/wave operations land, drift risk grows linearly.

Who benefits: anyone adding store operations. Today that means writing the logic twice and hoping both copies stay in sync. Tomorrow it means debugging a Postgres-only bug because a fix was applied to SQLite and not the other.

## Approach

**Pure logic functions + plan-then-execute.**

Extract composite business logic into pure functions in `store/ops.rs` that compute a mutation plan — no I/O, no async. Both backends execute the plan using their own primitives. This sidesteps the fundamental sync/async split between SQLite (`spawn_blocking`, synchronous `rusqlite`) and Postgres (native async `deadpool_postgres`).

### The pattern

```rust
// store/ops.rs — pure business logic, no I/O

/// Mutations that a composite operation needs the backend to execute.
pub(crate) enum StoreMutation {
    UpsertWave(Wave),
    DeleteWave(LfdId),
    DeleteStimuliForWave(LfdId),
}

pub(crate) struct JoinPlan {
    pub mutations: Vec<StoreMutation>,
    pub result_id: LfdId,
}

/// Compute the join plan from two already-fetched waves.
pub(crate) fn plan_join_waves(
    wave_a: &Wave,
    wave_b: &Wave,
    chord_name: Option<String>,
    nest: bool,
) -> StoreResult<JoinPlan> {
    match (wave_a.is_chord(), wave_b.is_chord()) {
        (false, false) => { /* Voice + Voice → new chord, reparent both */ }
        (true, false)  => { /* Chord + Voice → absorb into A */ }
        (false, true)  => { /* Voice + Chord → new chord, reparent both */ }
        (true, true) if nest => { /* Nest B under A */ }
        (true, true)   => { /* Merge B's children into A, delete B */ }
    }
}

pub(crate) struct LeavePlan {
    pub mutations: Vec<StoreMutation>,
}

pub(crate) fn plan_leave_wave(wave: &Wave) -> StoreResult<LeavePlan> {
    // Guard: wave must have a parent
    // Return: reparent to root + delete stimuli
}

/// Flatten a wave tree into ordered upserts (parent before children).
pub(crate) fn plan_create_wave(wave: &Wave) -> Vec<StoreMutation> {
    let mut mutations = vec![StoreMutation::UpsertWave(wave.clone())];
    for child in wave.children() {
        mutations.extend(plan_create_wave(child));
    }
    mutations
}
```

Then each backend has a thin executor:

```rust
// sqlite.rs
fn execute_mutations(&self, mutations: &[StoreMutation]) -> StoreResult<()> {
    for m in mutations {
        match m {
            StoreMutation::UpsertWave(w) => self.upsert_wave(w)?,
            StoreMutation::DeleteWave(id) => self.delete_wave(id)?,
            StoreMutation::DeleteStimuliForWave(id) => { self.delete_stimuli_for_wave(id)?; }
        }
    }
    Ok(())
}

pub fn join_waves(&self, wave_a: &Wave, wave_b: &Wave, chord_name: Option<String>, nest: bool) -> StoreResult<LfdId> {
    let plan = ops::plan_join_waves(wave_a, wave_b, chord_name, nest)?;
    self.execute_mutations(&plan.mutations)?;
    Ok(plan.result_id)
}
```

```rust
// postgres.rs
async fn execute_mutations(&self, mutations: &[StoreMutation]) -> StoreResult<()> {
    for m in mutations {
        match m {
            StoreMutation::UpsertWave(w) => self.upsert_wave(w).await?,
            StoreMutation::DeleteWave(id) => self.delete_wave(id).await?,
            StoreMutation::DeleteStimuliForWave(id) => { self.delete_stimuli_for_wave(id).await?; }
        }
    }
    Ok(())
}

pub async fn join_waves(&self, wave_a: &Wave, wave_b: &Wave, chord_name: Option<String>, nest: bool) -> StoreResult<LfdId> {
    let plan = ops::plan_join_waves(wave_a, wave_b, chord_name, nest)?;
    self.execute_mutations(&plan.mutations).await?;
    Ok(plan.result_id)
}
```

### Secondary cleanup

1. **Move `QueueBlock` row mapping to `rows.rs`.** Both backends inline the same `parse::<QueueBlockReason>()` + JSON deserialization for `conflict_files`. This belongs in `rows.rs` alongside the other `map_*_row` functions.

2. **Document SQL dialect divergences.** Two queries use `IN (?, ?, ?)` (SQLite) vs `= ANY($1)` (Postgres): `GetActiveWaveRun` and `FailOrphanedRuns`. These are already cleanly separated in `catalog.rs` via `sqlite_override`/`postgres_override`. No code change needed — just a comment in the catalog documenting why.

3. **Leave simple CRUD alone.** The ~35 single-query methods (`get_wave`, `upsert_wave`, `create_stimulus`, etc.) are mechanical: get SQL from catalog, bind params, map rows. The SQL is already shared (`catalog.rs`), row mapping is already shared (`rows.rs`). The only per-backend code is param binding, which is inherently different between `rusqlite::params![]` and `&[&dyn ToSql + Sync]`. This duplication is low-risk and not worth abstracting.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Internal async trait with default methods | Composites as default impls on an async trait that both backends implement | SQLite methods are synchronous (inside `spawn_blocking`). Can't implement an async trait with sync methods without wrapping every call — adds complexity for no clarity gain. |
| Macro-generated impls | Declarative macro emits both sync and async versions from one definition | Destroys IDE support, hard to debug, and the sync/async difference isn't just syntax — SQLite needs `i64` casts, Postgres needs `i32`; SQLite needs `Box::pin` for recursive async. The macro would accumulate cfg-like branches. |
| Generic executor trait | Abstract DB execution behind `trait Executor { async fn query(...) }` | `rusqlite` and `tokio_postgres` have fundamentally different type systems for params (`rusqlite::ToSql` vs `tokio_postgres::types::ToSql + Sync`). The `StoreRow` trait already abstracts the output side; abstracting the input side would require our own `ToParam` trait with conversions for every Rust type. High cost, marginal benefit over current `catalog.rs` + `rows.rs`. |
| Do nothing | Accept duplication | Viable short-term but `join_waves` is already 70 lines duplicated. Each new chord operation (split, reorder, promote) will add another 30-70 lines. The drift bug is a matter of when, not if. |

## Key decisions

1. **Pure functions over trait abstraction.** The sync/async split between backends is fundamental — SQLite runs inside `spawn_blocking`, Postgres is native async. Rather than fighting this with trait gymnastics, separate I/O from logic. The plan-then-execute pattern makes business logic testable without a database.

2. **`StoreMutation` enum, not function callbacks.** A data structure describing mutations is inspectable, testable, and serializable. Callbacks would need to be generic over sync/async. The enum also makes it trivial to add transaction wrapping later — execute all mutations inside a single transaction.

3. **Leave CRUD methods alone.** The existing `catalog.rs` + `rows.rs` layering already deduplicates the hard parts (SQL and row mapping). The remaining per-backend code for simple methods is ~5 lines of param binding — mechanical, readable, and unlikely to drift.

4. **Move `QueueBlock` mapping while we're here.** It's the one inline row mapper that escaped to `rows.rs`. Fixing it is five minutes and prevents the pattern from spreading.

## Scope

- In scope:
  - New `store/ops.rs` with pure logic for `join_waves`, `leave_wave`, `create_wave`
  - `StoreMutation` enum and `execute_mutations` in both backends
  - `QueueBlock` row mapping moved to `rows.rs`
  - Comments on dialect-divergent queries in `catalog.rs`

- Out of scope:
  - Abstracting simple CRUD methods (the existing dedup via catalog + rows is sufficient)
  - Session methods (on `Store` directly, not on traits — separate concern)
  - Transaction wrapping (future work that this design enables but doesn't require)
  - Changing the public `WaveStateStore` / `ExecutionStore` trait signatures

## Done when

```bash
cargo test --all                      # All existing tests pass
cargo clippy -- -D warnings           # No new warnings
```

And: `join_waves`, `leave_wave`, `create_wave` business logic appears exactly once (in `store/ops.rs`). Both `sqlite.rs` and `postgres.rs` call `plan_*` + `execute_mutations` with no duplicated match arms.
