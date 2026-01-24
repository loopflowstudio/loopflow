# Database Migrations

All schema changes go through migrations. The baseline contains the current schema; incremental migrations accumulate until the next consolidation.

## Adding a Migration

1. Create `m_YYYY_MM_DD_description.py`
2. Define `VERSION` (ISO timestamp after baseline), `DESCRIPTION`, and `apply(conn)`
3. Make it idempotent—check before modifying
4. **Register in `registry.py`**—migrations won't run unless registered

```python
# m_2026_01_23_add_foo.py
VERSION = "2026-01-23T12:00:00Z"
DESCRIPTION = "add foo column to bars"

def apply(conn):
    cols = {r[1] for r in conn.execute("PRAGMA table_info(bars)")}
    if "foo" not in cols:
        conn.execute("ALTER TABLE bars ADD COLUMN foo TEXT")
```

```python
# registry.py
from loopflow.lfd.migrations import baseline, m_2026_01_23_add_foo

MIGRATIONS = [
    Migration(baseline.SCHEMA_VERSION, baseline.DESCRIPTION, baseline.apply),
    Migration(m_2026_01_23_add_foo.VERSION, m_2026_01_23_add_foo.DESCRIPTION, m_2026_01_23_add_foo.apply),
]
```

**Important:** Migration `VERSION` must sort after `baseline.SCHEMA_VERSION` (use ISO timestamps with Z suffix).

## Consolidating Migrations

When incremental migrations accumulate, fold them into the baseline:

1. Manually update `baseline.py` with the combined schema
2. Run cleanup and commit:

```bash
./scripts/collapse_migrations.py --clean   # removes m_*.py, sets placeholder version
git add -A && git commit -m "consolidate migrations"
./scripts/collapse_migrations.py --finalize  # stamps with commit SHA
git commit --amend --no-edit
```

The version uses **git commit timestamp + SHA** (`2026-01-23T16:39:43Z_abc1234`):
- Ordered (ISO timestamps sort correctly)
- Reproducible (from git, not local time)
- Handles parallel development (no version collisions between branches)

## Rules

- **All schema changes go through migrations.** Never ALTER tables in runtime code.
- **Never modify baseline.py directly.** New columns = new migration. Baseline only changes during consolidation.
- **Make migrations idempotent.** Check if the change is needed before applying.
- **One migration per change.** Don't modify existing migration files.
- **Register all migrations.** Unregistered migrations won't run.
- **Migrations are one-way.** No rollback support—branch switching may require `rm ~/.lf/lfd.db`.
