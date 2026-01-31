# Rust Data Backend: Postgres + SQLite

## Problem

Loopflow is moving toward managed, multi-tenant clusters. The current SQLite-only persistence model cannot safely support multi-node concurrency, durable auditing, or enterprise backups. We need a Rust data layer that uses Postgres as the system of record for hosted `lfd`, while keeping local dev fast and unchanged.

## Approach

Add a `PostgresStore` that implements the existing `RunStore` trait. The trait is already well-designed—42 methods covering waves, stimuli, pending activations, fork runs, and step runs. Ship Postgres support without changing the trait, then add events and multi-tenancy in a follow-up.

**Phase 1: Postgres parity with SQLite**
- Implement `PostgresStore` matching the current `RunStore` trait
- Same schema shape as SQLite, same operations, same semantics
- Config switch: `storage = "sqlite"` (default) or `storage = "postgres"`
- Connection string via `LFD_DATABASE_URL` environment variable

**Phase 2: Multi-tenancy and events**
- Add `tenant_id` column to all tables (Postgres only)
- Add `events` table for append-only audit log
- Atomic event append + state update in same transaction

**Phase 3: Retention and ops**
- Configurable event retention (TTL-based cleanup)
- Backup guidance and runbook for managed deployments

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Postgres everywhere | Simpler code | Breaks local UX, heavyweight dependency for single-user dev |
| SQLite only with WAL + file locks | Minimal change | Doesn't meet multi-tenant concurrency or managed ops requirements |
| SQLx instead of raw postgres crate | Compile-time query checking | Adds complexity; runtime validation is sufficient for this scale |
| Event-sourcing only (no state tables) | Strong audit trail | Query performance suffers; read paths become complex |
| New abstraction layer | Clean slate | RunStore trait already works; don't fix what isn't broken |

## Key decisions

- **"Keep the trait, add a backend."** The existing `RunStore` trait (113 lines, 42 methods) is well-designed. `PostgresStore` implements it directly. No trait changes for Phase 1.

- **"Postgres in managed mode: Postgres is the system of record for hosted `lfd`."** SQLite stays the default for local dev. Managed clusters require Postgres. This is config, not discovery.

- **"tenant_id comes in Phase 2, not Phase 1."** Phase 1 proves Postgres works. Phase 2 adds multi-tenancy. Shipping smaller increments reduces risk.

- **"Flat tenancy: tenant_id + project_id."** A tenant contains projects. Projects contain waves. No deeper nesting. This answers the open question about tenancy model—choose the simpler option.

- **"Migrations live in `lfd`."** The Rust daemon owns its schema. Migration tooling runs as `lfd migrate` subcommand. DBAs can run it directly or via CI.

- **"Event retention is operator-defined."** No hardcoded defaults. Managed deployments configure retention via `LFD_EVENT_RETENTION_DAYS`. Unset = keep forever.

## Scope

**In scope:**
- `PostgresStore` implementing `RunStore` trait
- Postgres schema matching current SQLite tables (waves, stimuli, pending_activations, fork_runs, step_runs)
- Migration system for Postgres (`lfd migrate` command)
- Config-driven backend selection
- Dual-backend integration tests

**Out of scope (Phase 2+):**
- Multi-tenant schema (tenant_id, project_id columns)
- Events table and append-only audit log
- Retention policies and cleanup jobs
- Analytics or BI pipelines
- Python migration from SQLite to Postgres

## Schema

### Phase 1: Parity with SQLite

```sql
-- waves: long-running autonomous workflows
CREATE TABLE waves (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL UNIQUE,
    repo TEXT NOT NULL,
    flow TEXT NOT NULL,
    direction JSONB DEFAULT '[]'::jsonb,
    area JSONB DEFAULT '[]'::jsonb,
    paused BOOLEAN NOT NULL DEFAULT false,
    status TEXT NOT NULL DEFAULT 'idle',
    iteration INTEGER NOT NULL DEFAULT 0,
    worktree TEXT,
    branch TEXT,
    pr_limit INTEGER NOT NULL DEFAULT 5,
    merge_mode TEXT NOT NULL DEFAULT 'pr',
    pid INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    pending_activations INTEGER NOT NULL DEFAULT 0,
    step_index INTEGER NOT NULL DEFAULT 0,
    base_branch TEXT,
    base_commit TEXT
);
CREATE INDEX idx_waves_repo ON waves(repo);
CREATE INDEX idx_waves_status ON waves(status);

-- stimuli: trigger mechanisms (many:1 with waves)
CREATE TABLE stimuli (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wave_id UUID NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    cron TEXT,
    last_main_sha TEXT,
    last_triggered_at BIGINT,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_stimuli_wave_id ON stimuli(wave_id);
CREATE INDEX idx_stimuli_kind ON stimuli(kind);

-- pending_activations: queued trigger events
CREATE TABLE pending_activations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wave_id UUID NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    stimulus_id UUID NOT NULL REFERENCES stimuli(id) ON DELETE CASCADE,
    from_sha TEXT NOT NULL,
    to_sha TEXT NOT NULL,
    queued_at BIGINT NOT NULL
);
CREATE INDEX idx_pending_wave_id ON pending_activations(wave_id);

-- fork_runs: parallel branch execution state
CREATE TABLE fork_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wave_id UUID NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    step_index INTEGER NOT NULL,
    branch_index INTEGER NOT NULL,
    status INTEGER NOT NULL DEFAULT 0,
    worktree TEXT NOT NULL
);
CREATE INDEX idx_fork_runs_wave_step ON fork_runs(wave_id, step_index);

-- step_runs: individual step executions
CREATE TABLE step_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    step TEXT NOT NULL,
    repo TEXT NOT NULL,
    worktree TEXT NOT NULL,
    flow_run_id UUID,
    wave_id UUID REFERENCES waves(id) ON DELETE SET NULL,
    status INTEGER NOT NULL DEFAULT 1,
    started_at BIGINT NOT NULL,
    ended_at BIGINT,
    pid INTEGER,
    model TEXT NOT NULL DEFAULT 'claude-code',
    run_mode TEXT NOT NULL DEFAULT 'auto'
);
CREATE INDEX idx_step_runs_status ON step_runs(status);
CREATE INDEX idx_step_runs_wave ON step_runs(wave_id);

-- meta: schema versioning
CREATE TABLE meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
INSERT INTO meta (key, value) VALUES ('schema_version', '1');
```

### Phase 2: Multi-tenancy (future)

```sql
-- Add tenant_id to all tables
ALTER TABLE waves ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE stimuli ADD COLUMN tenant_id UUID NOT NULL;
-- ... etc

-- Tenants table
CREATE TABLE tenants (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Projects table
CREATE TABLE projects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(tenant_id, name)
);

-- Events table (append-only)
CREATE TABLE events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    project_id UUID NOT NULL,
    entity_type TEXT NOT NULL,  -- 'wave', 'step_run', etc
    entity_id UUID NOT NULL,
    event_type TEXT NOT NULL,   -- 'created', 'status_changed', etc
    payload JSONB NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_events_tenant_entity ON events(tenant_id, entity_type, entity_id);
CREATE INDEX idx_events_occurred ON events(occurred_at);
```

## Implementation

### File structure

```
rust/lfd/src/store/
├── mod.rs           # RunStore trait (unchanged)
├── sqlite.rs        # SqliteStore (existing)
├── postgres.rs      # PostgresStore (new)
└── migrations/
    ├── postgres/
    │   └── 001_initial.sql
    └── sqlite/
        └── (existing inline migrations)
```

### Dependencies

```toml
# Cargo.toml additions
[dependencies]
tokio-postgres = { version = "0.7", features = ["with-uuid-1", "with-serde_json-1", "with-time-0_3"] }
deadpool-postgres = "0.12"
```

### Backend selection

```rust
// main.rs
let store: SharedStore = match config.storage.as_str() {
    "postgres" => {
        let url = std::env::var("LFD_DATABASE_URL")
            .expect("LFD_DATABASE_URL required for postgres storage");
        Arc::new(PostgresStore::connect(&url).await?)
    }
    _ => Arc::new(SqliteStore::new(&db_path)?),
};
```

### Migration command

```bash
# Run migrations
lfd migrate

# Check migration status
lfd migrate --status

# Rollback (manual SQL, documented in runbook)
```

## Testing

### Dual-backend test suite

```rust
#[test_case(sqlite_store(); "sqlite")]
#[test_case(postgres_store(); "postgres")]
fn test_wave_lifecycle(store: impl RunStore) {
    let wave = create_test_wave();
    store.create_wave(&wave).unwrap();

    let loaded = store.get_wave(&wave.id).unwrap();
    assert_eq!(loaded.unwrap().name, wave.name);

    store.delete_wave(&wave.id).unwrap();
    assert!(store.get_wave(&wave.id).unwrap().is_none());
}
```

### CI setup

```yaml
# .github/workflows/ci.yml additions
services:
  postgres:
    image: postgres:16
    env:
      POSTGRES_PASSWORD: test
    ports:
      - 5432:5432

env:
  LFD_DATABASE_URL: postgres://postgres:test@localhost:5432/lfd_test
```

## Done when

- [ ] `lfd` starts with `storage=postgres` and connects successfully
- [ ] All 42 `RunStore` trait methods pass dual-backend tests
- [ ] `lfd migrate` creates schema from scratch on empty Postgres
- [ ] SQLite behavior unchanged (default, no config required)
- [ ] CI runs both SQLite and Postgres test suites
- [ ] Performance: Postgres queries under 10ms p99 for common operations

## Open questions resolved

| Question | Decision |
|----------|----------|
| Tenancy model | `tenant_id` + `project_id` (flat hierarchy) |
| Event retention defaults | Operator-defined via `LFD_EVENT_RETENTION_DAYS`, no hardcoded default |
| Migration tooling location | `lfd migrate` subcommand in Rust daemon |
