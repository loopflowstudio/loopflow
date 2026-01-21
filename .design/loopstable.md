# Schema Versioning for lfd Database

Add explicit version tracking to `lfd.db` migrations using timestamp-based versions.

## Current behavior

- Migrations are tracked in a `schema_migrations` table with ISO-8601 timestamps.
- Migration files live in `src/loopflow/lfd/migrations/m_*.py`.
- The registry lives in `src/loopflow/lfd/migrations/registry.py` and loads migrations in filename order.
- `_get_db()` always calls `_migrate_db()` after opening the connection.
- If `schema_migrations` does not exist, the initial migration runs and creates it.

## Migration format

```python
VERSION = "2025-01-20T00:00:00"
DESCRIPTION = "initial schema"

def apply(conn: sqlite3.Connection) -> None:
    conn.executescript("""
        CREATE TABLE IF NOT EXISTS schema_migrations (...);
        -- other schema setup
    """)
```

## Helper for new migrations

`create_migration(description: str) -> Path` in `src/loopflow/lfd/db.py` generates a timestamped migration file with a placeholder `apply()` function.

## Remaining work

None. Implementation matches this spec.
