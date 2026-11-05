#!/usr/bin/env python3
"""Verify the migration set before it merges.

A migration on origin/main is immutable: stores have already run it, so changing
it changes their history, not their schema. Repair a merged schema with a new
forward migration.

- Released batches (`<major>.<minor>[.<patch>].<NNN>_<name>.sql`) are frozen
  history: none is added, edited, renamed or deleted.
- Every later migration is `<slug>.sql`. `build.rs` orders slugs by the
  first-parent commit on origin/main that added them; a Task keeps one and
  edits it in place until it merges (`scripts/new_migration.py`).
- Runtime SQL names its columns, so an older lf can write a store a newer one
  extended.
- Destructive change is staged (Jack Heart, 2026-10-27): a new slug is additive
  against origin/main's schema, or it is a contract that drops only what an
  expand migration merged at least `STAGING_DAYS` ago left behind and exports
  the data it drops. A rebuild that keeps every column and a trigger or index
  change are additive. This is a CI measurement; nothing at runtime refuses.

    uv run python scripts/check_migrations.py

Stdlib only, so CI can run it without a Python environment.
"""

from __future__ import annotations

import datetime as dt
import os
import re
import sqlite3
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).parent.parent
MIGRATIONS_DIR = REPO_ROOT / "rust/loopflow/src/store/migrations"
RELEASED = re.compile(r"^\d+\.\d+\.(?:\d+\.)?\d{3}_[a-z0-9_]+\.sql$")
SLUG = re.compile(r"^([a-z][a-z0-9_]*)\.sql$")
DRAFT_MARKER = re.compile(r"^--[ \t]*draft:[ \t]*([a-z][a-z0-9_]*)[ \t]*$", re.MULTILINE)
# A contract names the expand migration it completes and where its preflight
# exported the data it drops before the SQL ran.
EXPAND_MARKER = re.compile(r"^--[ \t]*expand:[ \t]*([a-z][a-z0-9_]*)[ \t]*$", re.MULTILINE)
EXPORTS_MARKER = re.compile(r"^--[ \t]*exports:[ \t]*(\S.*?)[ \t]*$", re.MULTILINE)
# How long an expand migration has been on origin/main before its contract may
# drop what it left behind: the window in which lf processes on older builds
# exit. The one place this number lives.
STAGING_DAYS = 30


def _fail(message: str) -> None:
    print(f"migration check failed: {message}", file=sys.stderr)
    raise SystemExit(1)


def _migrations_at(ref: str) -> dict[str, bytes] | None:
    """Every migration file on `ref`, by file name; None when `ref` is unknown."""
    exists = subprocess.run(
        ["git", "rev-parse", "--verify", "--quiet", ref],
        cwd=REPO_ROOT,
        capture_output=True,
    )
    if exists.returncode != 0:
        return None
    listed = subprocess.run(
        ["git", "ls-tree", "--name-only", ref, "--", f"{MIGRATIONS_DIR.relative_to(REPO_ROOT)}/"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    migrations = {}
    for relative in listed.stdout.splitlines():
        if not relative.endswith(".sql"):
            continue
        migrations[Path(relative).name] = subprocess.run(
            ["git", "show", f"{ref}:{relative}"],
            cwd=REPO_ROOT,
            capture_output=True,
            check=True,
        ).stdout
    return migrations


def _local_migrations() -> dict[str, bytes]:
    if not MIGRATIONS_DIR.is_dir():
        _fail(f"{MIGRATIONS_DIR.relative_to(REPO_ROOT)} is missing")
    local = {}
    for path in sorted(MIGRATIONS_DIR.iterdir()):
        if path.is_dir():
            _fail(f"{path.name}/ — migrations live directly in {MIGRATIONS_DIR.name}/")
        if path.suffix != ".sql":
            continue
        if not RELEASED.match(path.name) and not SLUG.match(path.name):
            _fail(f"{path.name} is not a `<snake_case_slug>.sql` — run scripts/new_migration.py")
        local[path.name] = path.read_bytes()
    if not local:
        _fail("no migrations found")
    return local


def _released_names(local: dict[str, bytes]) -> set[str]:
    names = set()
    for name, content in local.items():
        if RELEASED.match(name):
            names.add(name.split("_", 1)[1].removesuffix(".sql"))
            names.update(DRAFT_MARKER.findall(content.decode()))
    return names


def _check_against_main(local: dict[str, bytes], main: dict[str, bytes] | None) -> list[str]:
    """Merged bytes are durable history; returns this branch's unmerged slugs."""
    if main is None:
        if os.environ.get("LOOPFLOW_REQUIRE_ORIGIN_MAIN") == "1":
            _fail("origin/main is unavailable in CI; the integration-history check cannot run")
        print("origin/main unavailable — skipping the integration-history check")
        return []
    for name, content in main.items():
        if name not in local:
            _fail(f"{name} is on origin/main but missing here; a merged migration is never deleted")
        if local[name] != content:
            _fail(f"{name} differs from origin/main — add a forward migration instead")
    for name in local:
        if RELEASED.match(name) and name not in main:
            _fail(f"{name} adds a released batch; new migrations are `<slug>.sql`")
    return sorted(name for name in local if name not in main)


def _check_runtime_sql() -> None:
    """An older lf writes a store a newer one extended additively, so runtime SQL
    names its columns: no positional INSERT, no `SELECT *`."""
    positional = re.compile(r"\bINSERT(?:\s+OR\s+\w+)?\s+INTO\s+\w+\s+VALUES\b", re.IGNORECASE)
    wildcard = re.compile(r"\bSELECT\s+(?:\w+\.)?\*", re.IGNORECASE)
    for path in sorted((REPO_ROOT / "rust/loopflow/src").rglob("*.rs")):
        if "test" in path.stem:
            continue
        # Unit-test modules trail production code; their fixtures seed historical schemas.
        source = path.read_text().split("#[cfg(test)]\nmod ", 1)[0]
        if positional.search(source) or wildcard.search(source):
            _fail(f"{path.relative_to(REPO_ROOT)} uses positional INSERT or SELECT *")


# -- Staging -------------------------------------------------------------------


def _released_key(name: str) -> tuple[int, ...]:
    numbers = name.split("_", 1)[0].split(".")
    # `0.10.001` sorts with the legacy three-part ids before `0.10.1.001`.
    if len(numbers) == 3:
        numbers.insert(2, "-1")
    return tuple(int(number) for number in numbers)


@dataclass(frozen=True)
class Column:
    declared_type: str
    required: bool
    default: str | None
    primary_key: bool

    @property
    def optional(self) -> bool:
        """An INSERT that does not name this column still succeeds."""
        defaulted = self.default is not None and self.default.upper() != "NULL"
        return not self.primary_key and (not self.required or defaulted)


@dataclass(frozen=True)
class UniqueIndex:
    table: str
    columns: tuple[str | None, ...]  # None indexes an expression
    predicate: str | None  # the partial index's WHERE

    def admits_older_rows(self, added: dict[str, Column]) -> bool:
        """Whether a row an older writer inserts can enter this index. Such a row
        carries every added column's default, so an index over only added columns
        whose WHERE is false for those defaults never sees one."""
        if self.predicate is None or any(column not in added for column in self.columns):
            return True
        names = ", ".join(_quote(name) for name in added)
        values = ", ".join(column.default or "NULL" for column in added.values())
        try:
            (entered,) = (
                sqlite3.connect(":memory:")
                .execute(
                    f"WITH defaults({names}) AS (SELECT {values})"
                    f" SELECT COUNT(*) FROM defaults WHERE {self.predicate}"
                )
                .fetchone()
            )
        except sqlite3.Error:  # the predicate names an old column
            return True
        return entered == 1


@dataclass(frozen=True)
class Schema:
    tables: dict[str, dict[str, Column]]
    views: dict[str, dict[str, Column]]
    unique_indexes: dict[str, UniqueIndex]

    def members(self) -> set[str]:
        """`table waves`, `column waves.id`, `view wave_health`: what a drop names."""
        members = set()
        for kind, relations in (("table", self.tables), ("view", self.views)):
            for name, columns in relations.items():
                members.add(f"{kind} {name}")
                members.update(f"column {name}.{column}" for column in columns)
        return members


@dataclass(frozen=True)
class Catalog:
    """origin/main's migrations: their bytes, and the time the first-parent commit
    that added each file landed, which is the order `build.rs` applies slugs in."""

    files: dict[str, bytes]
    added: dict[str, dt.datetime]

    @classmethod
    def of_origin_main(cls, files: dict[str, bytes]) -> Catalog:
        directory = MIGRATIONS_DIR.relative_to(REPO_ROOT)
        listed = subprocess.run(
            [
                "git", "log", "origin/main", "--first-parent", "--diff-merges=first-parent",
                "--no-renames", "--diff-filter=A", "--reverse", "--format=%ct", "--name-only", "--",
                f"{directory}/",
            ],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            check=True,
        )
        added: dict[str, dt.datetime] = {}
        when = dt.datetime.fromtimestamp(0, dt.timezone.utc)
        for line in listed.stdout.splitlines():
            if line.isdigit():
                when = dt.datetime.fromtimestamp(int(line), dt.timezone.utc)
            elif Path(line).parent == directory:  # `drafts/` once held same-named files
                added.setdefault(Path(line).name, when)
        return cls(files, added)

    def ordered(self) -> list[str]:
        """Every file in the order `build.rs` applies it."""
        released = sorted((name for name in self.files if RELEASED.match(name)), key=_released_key)
        merged = [name for name in self.added if SLUG.match(name) and name in self.files]
        stragglers = sorted(
            name for name in self.files if SLUG.match(name) and name not in self.added
        )
        return released + merged + stragglers

    def file_of(self, migration: str) -> str | None:
        """The file holding `migration`: its slug, or the released batch holding its draft."""
        if f"{migration}.sql" in self.files:
            return f"{migration}.sql"
        for name, content in self.files.items():
            if RELEASED.match(name) and (
                name.split("_", 1)[1].removesuffix(".sql") == migration
                or migration in DRAFT_MARKER.findall(content.decode())
            ):
                return name
        return None

    def replay(self, before: str | None = None) -> sqlite3.Connection:
        """A store with every migration applied, or only those before `before`."""
        ordered = self.ordered()
        if before is not None:
            ordered = ordered[: ordered.index(before)]
        conn = sqlite3.connect(":memory:")
        conn.execute(
            "CREATE TABLE schema_migrations (version TEXT PRIMARY KEY, applied_at INTEGER NOT NULL)"
        )
        for name in ordered:
            _apply(conn, name, self.files[name].decode())
        return conn


def _apply(conn: sqlite3.Connection, name: str, sql: str) -> None:
    try:
        conn.executescript(sql)
    except sqlite3.Error as error:
        _fail(
            f"{name} does not apply over the schema before it"
            f" (SQLite {sqlite3.sqlite_version}): {error}"
        )


def _schema(conn: sqlite3.Connection) -> Schema:
    rows = conn.execute(
        "SELECT type, name, tbl_name, sql FROM sqlite_master"
        " WHERE name NOT LIKE 'sqlite_%' AND name != 'schema_migrations'"
    ).fetchall()
    tables: dict[str, dict[str, Column]] = {}
    views: dict[str, dict[str, Column]] = {}
    unique_indexes: dict[str, UniqueIndex] = {}
    for kind, name, table, sql in rows:
        if kind == "table":
            tables[name] = _columns(conn, name)
        elif kind == "view":
            views[name] = _columns(conn, name)
        elif kind == "index" and sql and sql.upper().startswith("CREATE UNIQUE"):
            predicate = re.search(r"\bWHERE\b(.*)", sql, re.IGNORECASE | re.DOTALL)
            unique_indexes[name] = UniqueIndex(
                table=table,
                columns=tuple(
                    column for _, _, column in conn.execute(f"PRAGMA index_info({_quote(name)})")
                ),
                predicate=predicate.group(1).strip() if predicate else None,
            )
    return Schema(tables, views, unique_indexes)


def _quote(identifier: str) -> str:
    return '"' + identifier.replace('"', '""') + '"'


def _columns(conn: sqlite3.Connection, relation: str) -> dict[str, Column]:
    """A table's or view's columns; a view is read like the table it presents."""
    return {
        column: Column(
            declared_type=declared_type.upper(),
            required=bool(required),
            default=default,
            primary_key=bool(primary_key),
        )
        for _, column, declared_type, required, default, primary_key, _ in conn.execute(
            f"PRAGMA table_xinfo({_quote(relation)})"
        )
    }


def _compare(before: Schema, after: Schema) -> tuple[list[str], list[str]]:
    """What a migration dropped, and what else breaks SQL written against `before`."""
    drops: list[str] = []
    breaks: list[str] = []
    for table, old_columns in before.tables.items():
        new_columns = after.tables.get(table)
        if new_columns is None:
            drops.append(f"table {table}")
            continue
        for column, old in old_columns.items():
            new = new_columns.get(column)
            if new is None:
                drops.append(f"column {table}.{column}")
            elif new.declared_type != old.declared_type or new.primary_key != old.primary_key:
                breaks.append(f"column {table}.{column} changed its type or key role")
            elif new.required and not old.required:
                breaks.append(
                    f"column {table}.{column} became NOT NULL; an older writer may still write NULL"
                )
        for column, new in new_columns.items():
            if column not in old_columns and not new.optional:
                breaks.append(
                    f"new column {table}.{column} is required without a default;"
                    " an older INSERT omits it"
                )
    # A view is only read, so its definition may change as long as it keeps its columns.
    for view, old_columns in before.views.items():
        new_columns = after.views.get(view)
        if new_columns is None:
            drops.append(f"view {view}")
            continue
        drops.extend(
            f"column {view}.{column}" for column in old_columns.keys() - new_columns.keys()
        )
    for name, index in after.unique_indexes.items():
        if name in before.unique_indexes or index.table not in before.tables:
            continue
        old_columns = before.tables[index.table]
        added = {
            column: new
            for column, new in after.tables[index.table].items()
            if column not in old_columns
        }
        if index.admits_older_rows(added):
            breaks.append(f"unique index {name} on {index.table}; an older writer may violate it")
    return drops, breaks


def _check_staging(local: dict[str, bytes], main: dict[str, bytes], unmerged: list[str]) -> None:
    """A new slug is additive, or a dated contract that exports what it drops."""
    catalog = Catalog.of_origin_main(main)
    conn = catalog.replay()
    for name in unmerged:
        sql = local[name].decode()
        before = _schema(conn)
        _apply(conn, name, sql)
        drops, breaks = _compare(before, _schema(conn))
        if breaks:
            _fail(
                f"{name} is not additive: {'; '.join(breaks)}. Stage it: add the new shape now"
                f" and drop the old one in a contract {STAGING_DAYS} days after that merges"
            )
        if drops:
            _check_contract(name, sql, drops, catalog)


def _check_contract(name: str, sql: str, drops: list[str], catalog: Catalog) -> None:
    """A contract names an aged expand step, exports, and drops only what that step superseded."""
    dropped = ", ".join(drops)
    expand = EXPAND_MARKER.search(sql)
    if not expand:
        _fail(
            f"{name} drops {dropped} without naming its expand step. Destructive change is"
            f" staged: expand (merge the new shape), wait {STAGING_DAYS} days for older lf to"
            " exit, export and drop. Add `-- expand: <migration>` and `-- exports: <where>`"
        )
    exports = EXPORTS_MARKER.search(sql)
    if not exports:
        _fail(
            f"{name} drops {dropped} without `-- exports: <where>` naming where its preflight"
            " writes that data before the SQL runs (settings-cutover.json is the template)"
        )
    step = expand.group(1)
    expand_file = catalog.file_of(step)
    if expand_file is None:
        _fail(f"{name} names expand step {step!r}, which is not on origin/main")
    if expand_file not in catalog.added:
        _fail(f"{expand_file} has no first-parent commit on origin/main")
    merged_at = catalog.added[expand_file]
    age = dt.datetime.now(dt.timezone.utc) - merged_at
    if age < dt.timedelta(days=STAGING_DAYS):
        _fail(
            f"{name} contracts {step}, which merged {age.days} days ago; its contract clears on"
            f" {merged_at + dt.timedelta(days=STAGING_DAYS):%Y-%m-%d} ({STAGING_DAYS} days)"
        )
    left_behind = _schema(catalog.replay(before=expand_file)).members()
    for item in drops:
        if item not in left_behind:
            _fail(
                f"{name} drops {item}, which {step} did not leave behind;"
                " a contract drops only what its expand step superseded"
            )
    print(f"{name}: contract of {step} ({age.days} days), exports {exports.group(1)}")


def main() -> None:
    local = _local_migrations()
    released = _released_names(local)
    for name in local:
        slug = SLUG.match(name)
        if slug and slug.group(1) in released:
            _fail(f"{name} reuses the released migration name {slug.group(1)!r}")
    main_migrations = _migrations_at("origin/main")
    unmerged = _check_against_main(local, main_migrations)
    if main_migrations is not None and unmerged:
        _check_staging(local, main_migrations, unmerged)
    _check_runtime_sql()
    # A merge-queue group may carry several Tasks' migrations at once.
    print(f"unmerged: {', '.join(unmerged)}" if unmerged else "no unmerged migration")


if __name__ == "__main__":
    main()
