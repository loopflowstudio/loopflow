# Schema Versioning for lfd Database

Add explicit version tracking to `lfd.db` migrations using timestamp-based versions.

## What to build

Replace ad-hoc column-existence checks with timestamp-versioned migrations tracked in a `schema_migrations` table.

## Data structures

```python
@dataclass
class Migration:
    version: str  # ISO timestamp: "2025-01-20T12:00:00"
    description: str
    apply: Callable[[sqlite3.Connection], None]
```

```sql
CREATE TABLE schema_migrations (
    version TEXT PRIMARY KEY,
    applied_at TEXT NOT NULL
);
```

## Key functions

```python
def _get_applied_migrations(conn: sqlite3.Connection) -> set[str]:
    """Get set of applied migration versions."""
    try:
        cursor = conn.execute("SELECT version FROM schema_migrations")
        return {row[0] for row in cursor}
    except sqlite3.OperationalError:
        return set()  # Table doesn't exist yet

def _record_migration(conn: sqlite3.Connection, version: str) -> None:
    """Record that a migration was applied."""
    conn.execute(
        "INSERT INTO schema_migrations (version, applied_at) VALUES (?, ?)",
        (version, datetime.now().isoformat()),
    )

def _migrate_db(conn: sqlite3.Connection) -> None:
    """Apply pending migrations in order."""
    applied = _get_applied_migrations(conn)

    for migration in MIGRATIONS:
        if migration.version not in applied:
            migration.apply(conn)
            _record_migration(conn, migration.version)
            conn.commit()
```

## Migration registry

Migrations live in `src/loopflow/lfd/migrations/` as individual files:

```
migrations/
    __init__.py          # Collects and sorts all migrations
    m_2025_01_20_initial.py
    m_2025_01_22_add_foo.py
```

Each file:

```python
# m_2025_01_20_initial.py
VERSION = "2025-01-20T00:00:00"
DESCRIPTION = "initial schema"

def apply(conn: sqlite3.Connection) -> None:
    conn.executescript("""...""")
```

The `__init__.py` auto-discovers and sorts:

```python
# migrations/__init__.py
from pathlib import Path
import importlib

def load_migrations() -> list[Migration]:
    migrations = []
    for file in sorted(Path(__file__).parent.glob("m_*.py")):
        mod = importlib.import_module(f".{file.stem}", __package__)
        migrations.append(Migration(mod.VERSION, mod.DESCRIPTION, mod.apply))
    return migrations

MIGRATIONS = load_migrations()
```

## Creating new migrations

Helper function generates the file:

```python
def create_migration(description: str) -> Path:
    """Create a new migration file with auto-generated timestamp."""
    now = datetime.now()
    slug = description.lower().replace(" ", "_")[:30]
    filename = f"m_{now.strftime('%Y_%m_%d_%H%M%S')}_{slug}.py"

    template = f'''"""
{description}
"""
VERSION = "{now.isoformat()}"
DESCRIPTION = "{description}"

def apply(conn):
    conn.executescript("""
        -- TODO: migration SQL here
    """)
'''
    path = Path(__file__).parent / "migrations" / filename
    path.write_text(template)
    return path
```

Usage (could be exposed via CLI later):

```python
from loopflow.lfd.db import create_migration
create_migration("add notifications table")
# Creates: migrations/m_2025_01_22_143052_add_notifications_table.py
```

## Initial migration

The initial migration creates everything fresh—current schema, no history:

```python
def _migrate_initial(conn: sqlite3.Connection) -> None:
    """Create initial schema (current state)."""
    conn.executescript("""
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version TEXT PRIMARY KEY,
            applied_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS loops (...current full schema...);
        CREATE TABLE IF NOT EXISTS loop_runs (...);
        CREATE TABLE IF NOT EXISTS sessions (...);

        -- indexes
        CREATE INDEX IF NOT EXISTS ...;
    """)
```

## Handling existing databases

Existing databases won't have `schema_migrations` table. Detection:

```python
def _is_legacy_db(conn: sqlite3.Connection) -> bool:
    """Check if this is a pre-migration database."""
    cursor = conn.execute(
        "SELECT name FROM sqlite_master WHERE type='table' AND name='schema_migrations'"
    )
    return cursor.fetchone() is None

def _migrate_db(conn: sqlite3.Connection) -> None:
    if _is_legacy_db(conn):
        # Legacy DB: create migrations table, mark initial as applied
        # (schema already exists from old _init_db)
        conn.execute("""
            CREATE TABLE schema_migrations (
                version TEXT PRIMARY KEY,
                applied_at TEXT NOT NULL
            )
        """)
        _record_migration(conn, "2025-01-20T00:00:00")
        conn.commit()
        return

    # Normal migration path
    applied = _get_applied_migrations(conn)
    for migration in MIGRATIONS:
        if migration.version not in applied:
            migration.apply(conn)
            _record_migration(conn, migration.version)
            conn.commit()
```

## Changes from current code

| Before | After |
|--------|-------|
| `_migrated_dbs: set[Path]` | Remove entirely |
| `_init_db()` creates schema | `_migrate_initial()` creates schema |
| Column existence checks | Migration table checks |
| `_migrate_db()` with column checks | `_migrate_db()` with version checks |
| `_migrate_goal_nullable()` | Absorbed into initial schema |

## Constraints

- **Forward-only**: No rollback support
- **Timestamps**: ISO 8601 format, sorted lexicographically
- **Idempotent**: Safe to call `_get_db()` multiple times

## Done when

```bash
# Fresh database gets migration recorded
rm -f ~/.lf/lfd.db
python -c "
from loopflow.lfd.db import _get_db
conn = _get_db()
print(conn.execute('SELECT * FROM schema_migrations').fetchall())
"
# Output: [('2025-01-20T00:00:00', '2025-01-20T...')]

# Existing database upgrades cleanly
# (test with a pre-existing ~/.lf/lfd.db)

# Tests pass
uv run pytest tests/ -k db
```
