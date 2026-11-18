"""Print the SQL that rebuilds each table whose foreign keys name `processes`
without those keys, keeping every column, index and trigger.

    uv run python scratch/rebuild_without_process_keys.py

Replays the migrations up to and including `process_rows_on_owners.sql` as it
stands without its generated block, so run it before pasting the block back.
"""
import re
import sys
from pathlib import Path

sys.path.insert(0, "scripts")
import check_migrations as cm  # noqa: E402

TABLES = ["agent_sessions", "ci_incidents", "desktop_shells", "session_events",
          "steer_adapters", "task_graphs", "tasks", "waves"]
KEY = re.compile(
    r'\s+REFERENCES\s+"?processes"?\s*\(\s*"?id"?\s*\)(\s+ON\s+(DELETE|UPDATE)\s+(SET\s+NULL|SET\s+DEFAULT|CASCADE|RESTRICT|NO\s+ACTION))*',
    re.IGNORECASE)
MARK = "-- generated: rebuild_without_process_keys"

local = cm._local_migrations()
name = next(n for n in local if Path(n).stem == "process_rows_on_owners")
local[name] = local[name].decode().split(MARK)[0].encode()
db = cm.Catalog.of_origin_main(local).replay()

out = [MARK, "PRAGMA legacy_alter_table = ON;"]
indexes, triggers = [], []
for table in TABLES:
    (sql,) = db.execute("SELECT sql FROM sqlite_master WHERE type='table' AND name=?", (table,)).fetchone()
    stripped, count = KEY.subn("", sql)
    assert count, table
    head = re.match(r'CREATE TABLE\s+"?' + table + r'"?\s*\(', stripped)
    body = stripped[head.end():]
    columns = [row[1] for row in db.execute(f"PRAGMA table_info('{table}')")]
    names = ",".join(columns)
    out += [f"CREATE TABLE {table}_owned ({body};",
            f"INSERT INTO {table}_owned ({names}) SELECT {names} FROM {table};",
            f"DROP TABLE {table};",
            f"ALTER TABLE {table}_owned RENAME TO {table};"]
    indexes += [s for (s,) in db.execute(
        "SELECT sql FROM sqlite_master WHERE type='index' AND tbl_name=? AND sql IS NOT NULL ORDER BY name", (table,))]
    triggers += [s for (s,) in db.execute(
        "SELECT sql FROM sqlite_master WHERE type='trigger' AND tbl_name=? ORDER BY name", (table,))]
out += [s + ";" for s in indexes + triggers]
out.append("PRAGMA legacy_alter_table = OFF;")
print("\n".join(out))
