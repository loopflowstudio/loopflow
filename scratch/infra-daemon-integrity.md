# Daemon Data Integrity

## Problem

The lfd daemon has three categories of data integrity issues that compound over time: non-atomic migrations, unbounded resource accumulation, and a silent webhook misconfiguration state. Each is low-severity individually but together they erode trust in long-running deployments.

## Approach

Three focused changes, each independently shippable.

### 1. Transactional SQLite Migrations

**Current state:** Both `apply_sqlite()` and `apply_postgres()` in `lfd/store/migrations.rs` lack per-migration atomicity. SQLite runs each migration via `execute_batch()` with auto-commit — a crash between the migration SQL and its `schema_migrations` insert leaves the schema in an unknown state. Postgres wraps the entire batch in one caller-provided transaction, so a failure in migration 16 rolls back 15 successful migrations.

**Change:** Wrap each migration + its bookkeeping insert in its own transaction on both backends. One transaction per migration, not one for the whole batch — partial progress is preserved if a later migration fails.

SQLite — `BEGIN EXCLUSIVE` / `COMMIT` per migration:

```rust
for migration in migrations() {
    if applied.contains(migration.version) {
        continue;
    }
    conn.execute_batch("BEGIN EXCLUSIVE")?;
    match conn.execute_batch(migration.sql) {
        Ok(()) => {
            conn.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![migration.version, now],
            )?;
            conn.execute_batch("COMMIT")?;
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(e.into());
        }
    }
}
```

Postgres — change `apply_postgres()` to accept `&tokio_postgres::Client` instead of `&tokio_postgres::Transaction`, create a transaction per migration:

```rust
// migrate_async() passes &client directly instead of creating one transaction
// apply_postgres() signature changes: &Transaction<'_> → &Client
for migration in migrations() {
    if applied.contains(migration.version) {
        continue;
    }
    let transaction = client.transaction().await?;
    transaction.batch_execute(migration.sql).await?;
    transaction.execute(
        "INSERT INTO schema_migrations (version, applied_at) VALUES ($1, $2)",
        &[&migration.version, &now_unix()],
    ).await?;
    transaction.commit().await?;
}
```

Also update `applied_versions_postgres()` to accept `&Client` instead of `&Transaction`.

**Research confirmed:** All 24 migration files use standard DDL/DML (CREATE TABLE, ALTER TABLE, INSERT, DROP). Zero PRAGMA statements. Everything is safe to run inside a transaction on both backends.

**Duplicate 016_ prefix:** Two migrations share the `016_` numeric prefix — `016_provider_tokens` and `016_rename_sidecar_kind_to_ci_fix_kind`. The version strings stored in `schema_migrations` are the full names, so they're distinct rows. This is cosmetic, not a correctness bug. Add a comment in the migration list explaining it. No renumbering (that would require a migration to update existing schema_migrations entries).

**Test:** Add an integration test that applies all migrations to a fresh in-memory SQLite database and asserts:
- All 24 versions appear in `schema_migrations`
- Key tables exist (waves, sessions, repos, stimuli, etc.)
- No errors on re-application (idempotent check)

### 2. Resource Accumulation Bounds

Three unbounded collections, three targeted fixes.

#### Output log pruning (`~/.lf/output/*.log`)

Add `prune_output_logs(dir: &Path, max_age: Duration)` that deletes files older than `max_age` based on filesystem mtime. Call it:
- On startup (before accepting requests)
- Every hour from the background reconciliation loop

Default TTL: 7 days. Configurable via `output_log_retention_days` in lfd config.

#### Output file handle eviction

The `Writers` struct in `output.rs` holds a `HashMap<String, File>` — one open file handle per wave run's log file. It only grows. Add a `close(wave_run_id: &str)` method that removes and drops the file handle. Call it when a wave run reaches a terminal state (completed/failed) in `WaveExecutor::execute()` (line ~756) and `WaveExecutor::fail_run()` (line ~898) in `lfd/executor/wave/mod.rs`, after `store.update_wave_run()`.

The HashMap naturally shrinks as completed runs are closed. Active runs keep their handles — no LRU needed. Handles reopen on demand if a completed run somehow receives more output.

#### QUEUE_RECONCILE_LOCKS cleanup

Add `remove_reconcile_lock(wave_id: &LfdId)`:

```rust
pub async fn remove_reconcile_lock(wave_id: &LfdId) {
    let mut locks = QUEUE_RECONCILE_LOCKS.lock().await;
    locks.remove(&wave_id.to_string());
}
```

Call it from `delete_wave_handler` after the wave is deleted from the store. Safe because a deleted wave won't receive new reconciliation requests.

### 3. Webhook Startup Warning

**Current state:** The webhook endpoint already returns 503 when `webhook_secret` is empty. This is correct behavior — unauthenticated webhooks are rejected. The wave item's concern about "silently accept unauthenticated webhooks" is already addressed in code.

**Missing piece:** There's no startup log message. An operator deploying lfd for the first time won't know why their webhooks fail until they check HTTP responses. Add a `warn!` at startup when the secret is empty:

```rust
if config.github.webhook_secret.trim().is_empty() {
    warn!("GitHub webhook secret is not configured — webhook endpoint will reject all requests. Set LFD_GITHUB_WEBHOOK_SECRET or github.webhook_secret in config.");
}
```

This is the entire change for webhook security. The endpoint is already safe by default.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Single transaction for all migrations | All-or-nothing: one failure rolls back everything | Too aggressive — partial progress is valuable for debugging. Both backends now use per-migration transactions for consistency. |
| LRU cache for file handles | Bounds memory regardless of run lifecycle | Over-engineering — closing on completion is simpler and more predictable |
| Weak refs for QUEUE_RECONCILE_LOCKS | Auto-cleanup when no references remain | Arc<Mutex<()>> is cloned into the lock guard, keeping it alive during use — weak refs add complexity without benefit here |
| Auto-generate webhook_secret on first run | Zero-config security | Bad UX — operator needs to configure the same secret in GitHub. Generated secrets they can't see are useless. |

## Key decisions

**BEGIN EXCLUSIVE over BEGIN IMMEDIATE.** EXCLUSIVE prevents other connections from reading during migration, which is correct — no one should read a half-migrated schema. Migrations run once at startup, so the brief lock is fine.

**7-day default TTL for logs.** Long enough to debug recent issues, short enough to prevent disk bloat. Configurable for operators who need longer retention.

**No renumbering of 016_ duplicates.** Renumbering would require a corrective migration to update `schema_migrations`. The cosmetic inconsistency isn't worth the operational risk.

**Startup warning, not startup rejection.** An operator may intentionally run lfd without GitHub integration. Rejecting startup would block valid use cases. A loud warning is the right balance.

## Scope

- In scope: transaction wrapping (both SQLite and Postgres), migration test, log pruning, handle eviction, lock cleanup, startup warning
- Out of scope: log rotation/compression, webhook rate limiting, migration versioning overhaul

## Done when

```bash
# All migrations apply cleanly to a fresh DB
cargo test -p loopflow test_migrations

# Clippy and fmt pass
cargo clippy -- -D warnings && cargo fmt --check

# Existing tests still pass
cargo test --all
```

Observable: after running lfd with empty webhook_secret, logs contain the warning message. After deleting a wave, its lock entry and completed run file handles are cleaned up. Output logs older than 7 days are removed on startup.
