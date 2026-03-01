# 01: Daemon Data Integrity

**Finish line:** SQLite migrations are transactional, resource leaks are bounded, and the webhook endpoint is verified safe.

## Problem

The lfd daemon has data integrity issues that compound over time. A crash mid-migration can leave the schema in a half-applied state. File handles and lock entries accumulate indefinitely. The wave item also flagged webhook security, but research shows that's already handled correctly.

Who benefits: anyone running lfd for more than a few hours. These issues are invisible at first and painful to debug when they surface.

## Research findings

**Migrations:** 24 migration files exist (not 21 as the wave item states). No PRAGMA statements in any migration file — all DDL is safe inside SQLite transactions. The Postgres path already wraps all migrations in a transaction; SQLite does not. Crash between `execute_batch` and the `INSERT INTO schema_migrations` leaves the migration applied but unrecorded, causing re-run on next startup which fails on `CREATE TABLE` / `ALTER TABLE`.

Two files share the `016_` prefix: `016_provider_tokens.sql` and `016_rename_sidecar_kind_to_ci_fix_kind.sql`. They operate on unrelated tables and both apply correctly because versioning uses full string keys, not numeric prefixes. The naming is misleading but not broken.

**Webhook security is already safe.** When `webhook_secret` is empty, `github_webhook_handler` returns HTTP 503 before processing any payload. `verify_webhook_signature` also rejects empty secrets as a second layer. HMAC uses constant-time comparison. No work needed here.

**Resource accumulation confirmed:**
- `OutputHub::Writers` (`output.rs`): `HashMap<String, File>` grows one entry per run, handles never closed
- `~/.lf/output/*.log`: one file per run, never deleted, not touched on wave delete
- `QUEUE_RECONCILE_LOCKS` (`queue.rs:96`): static `HashMap<String, Arc<Mutex<()>>>`, entries never removed on wave deletion

## Approach

### 1. Transactional SQLite migrations

Wrap each migration individually in `BEGIN EXCLUSIVE` / `COMMIT` with `ROLLBACK` on failure. Use `EXCLUSIVE` because migrations do DDL and we want to prevent concurrent readers from seeing partial schema changes.

```rust
// In apply_sqlite, for each unapplied migration:
conn.execute_batch("BEGIN EXCLUSIVE")?;
match conn.execute_batch(migration.sql)
    .and_then(|_| conn.execute(
        "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
        params![migration.version, now_unix()],
    ))
{
    Ok(_) => conn.execute_batch("COMMIT")?,
    Err(e) => {
        let _ = conn.execute_batch("ROLLBACK");
        return Err(e.into());
    }
}
```

Both the SQL and the version recording happen in the same transaction. Crash between them is no longer possible — either both commit or neither does.

Per-migration transactions (not one transaction for the whole loop) so that if migration N fails, migrations 1..N-1 remain safely applied and recorded.

**Duplicate 016_ prefix:** Add a comment in `ALL_MIGRATIONS` explaining the duplicate is historical and both apply correctly because version keys are full strings. No rename — that would break databases that already have the old version string recorded.

**Test:** Add a test that creates an in-memory SQLite database, applies all migrations, verifies `schema_migrations` contains all 24 versions, and spot-checks key tables exist in `sqlite_master`.

### 2. OutputHub file handle eviction

Add a `close_writer(&self, wave_run_id: &str)` method to `OutputHub` that removes the entry from `Writers.files` (dropping the `File` handle). Call it from the run completion path — wherever a `WaveRun` transitions to a terminal state (completed, failed, cancelled).

This is the natural cleanup point. The handle is only needed while the run is active and writing output. After completion, reads go through `read_log` which opens the file independently.

### 3. Output log pruning

Add a `prune_output_logs` function that deletes `.log` files older than 7 days (by mtime). Call it:
- Once on daemon startup (in `lfd::run` before entering the main loop)
- Periodically via a background task (every 6 hours)

Use `std::fs::metadata` to check mtime. Walk `output_dir` with `std::fs::read_dir`. Simple and bounded.

### 4. Queue reconcile lock cleanup

Add a `remove_reconcile_lock(wave_id: &str)` function next to `acquire_reconcile_lock`. Call it from `delete_wave_handler` after the wave is removed from the store.

The lock entry is tiny (String + Arc<Mutex<()>>), so this is more about correctness than memory pressure. A deleted wave should leave no trace in process state.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| One transaction for all migrations (matches Postgres) | All-or-nothing: if migration 20 fails, 1-19 roll back too | Per-migration is more resilient for operational recovery. One bad migration shouldn't undo 23 good ones |
| Idle timeout for OutputHub handles | Simpler, no coupling to run lifecycle | Adds a timer and sweep thread. Run completion is the natural cleanup signal and already exists |
| Weak refs for queue locks | Auto-cleanup when no one holds the lock | `Arc<Mutex<()>>` is always held by the static HashMap, so weak refs would immediately expire — wrong pattern here |
| Log pruning via external cron | Zero daemon code | Users forget to set it up. The daemon creates the files; the daemon should manage them |
| Fix webhook security | Wave item suggested it | Already safe — 503 on empty secret, constant-time HMAC. Adding more would be over-engineering |

## Key decisions

**Per-migration transactions, not whole-loop.** The Postgres path uses one transaction for everything, but that's because Postgres has robust DDL transaction support. SQLite's DDL-in-transaction support is solid but per-migration isolation is more defensive — and matches how migrations conceptually work (each is independent).

**`BEGIN EXCLUSIVE` not `BEGIN`.** Migrations modify schema. We don't want WAL readers seeing a half-applied migration. The window is small but the cost of EXCLUSIVE is negligible at startup.

**Drop webhook security from scope.** The code already does the right thing. Spending time here would be busy work. The wave item's concern was valid but the codebase addressed it before this sprint.

**7-day TTL for logs, not configurable.** Start with a hardcoded constant. If someone needs to change it, that's a future PR. Configurable TTL for log pruning is premature.

## Scope

**In scope:**
- Transaction wrapping for each SQLite migration in `apply_sqlite`
- Comment documenting the duplicate `016_` prefix
- Migration test (in-memory SQLite, all migrations, schema verification)
- `OutputHub::close_writer` method + call from run completion
- `prune_output_logs` function + startup and periodic calls
- `remove_reconcile_lock` function + call from `delete_wave_handler`

**Out of scope:**
- Webhook security changes (already safe)
- Postgres migration changes (already transactional)
- Configurable log TTL
- OutputHub read-path changes (read_log opens files independently, not affected)

## Done when

```bash
# All migrations apply cleanly to a fresh DB inside transactions
cargo test -p loopflow test_all_migrations_apply

# Clippy and existing tests still pass
cargo clippy -- -D warnings && cargo test --all

# Output log pruning runs on startup (visible in daemon logs)
# OutputHub handles are closed on run completion (no unbounded growth)
# Queue locks are cleaned up on wave deletion
```

Wave goals advanced:
- "No silent data corruption path in migrations" — each migration is atomic
- "Long-running lfd daemons don't accumulate unbounded resources" — all three leaks bounded
- "Security defaults are safe (no open webhook endpoint)" — verified already true
