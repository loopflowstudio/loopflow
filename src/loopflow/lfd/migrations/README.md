# Database Migrations

All schema changes go through migrations. Each migration is a Python file in this directory.

## Adding a Migration

1. Create `m_YYYY_MM_DD_description.py`
2. Define `VERSION` (ISO timestamp), `DESCRIPTION`, and `apply(conn)`
3. Make it idempotent—check before modifying

```python
VERSION = "2025-01-23T00:00:00"
DESCRIPTION = "add foo column to bars"

def apply(conn):
    cols = {r["name"] for r in conn.execute("PRAGMA table_info(bars)")}
    if "foo" not in cols:
        conn.execute("ALTER TABLE bars ADD COLUMN foo TEXT")
```

## Rules

- **All schema changes go through migrations.** Never ALTER tables in runtime code.
- **Make migrations idempotent.** Check if the change is needed before applying.
- **One migration per change.** Don't modify existing migration files.
- **Migrations are one-way.** No rollback support—branch switching may require `rm ~/.lf/lfd.db`.
