# Publishbugs

Fixes a database initialization race condition where `_ensure_loop_columns` could fail on fresh databases.

## Review

**Verdict:** Ready to ship

The fix is correct and minimal. When `_get_db()` is called on a fresh database, `_run_migrations()` creates the schema but `_ensure_loop_columns()` was being called before checking if the `loops` table exists. The new guard clause at `db.py:72-77` properly handles this edge case by returning early if the table doesn't exist yet—migrations will create it with the correct schema.

No style violations. No missing tests needed—this is a defensive check for an ordering edge case that would be difficult to reproduce reliably in tests without mocking internals.
