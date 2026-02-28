# 01: Daemon Data Integrity

**Finish line:** SQLite migrations are transactional, resource leaks are bounded, and the webhook endpoint is safe by default.

## Scope

The lfd daemon has three data integrity issues that compound over time:

**Transactional migrations.** Wrap each of the 21 SQLite migrations in `BEGIN/COMMIT` in `store/migrations.rs`. Rename or document the duplicate `016_` prefix. Add a test that applies all migrations to a fresh SQLite DB and verifies the resulting schema.

**Resource accumulation.** Three unbounded collections need bounds:
- `~/.lf/output/*.log` — add TTL-based pruning (e.g., delete logs older than 7 days on startup and periodically)
- `OutputHub` file handle cache — evict handles when a run completes or after idle timeout
- `QUEUE_RECONCILE_LOCKS` in `queue.rs` — remove entries when the wave is deleted, or use weak refs

**Webhook security.** When `webhook_secret` is empty (the default), either reject webhook requests entirely or log a loud startup warning. An unconfigured deployment should not silently accept unauthenticated GitHub webhooks.

## What to learn

Whether any existing migration SQL relies on implicit auto-commit (statements that can't run inside a transaction, like `PRAGMA` changes). Read all 21 migration files before wrapping.
