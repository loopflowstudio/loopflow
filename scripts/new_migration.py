#!/usr/bin/env python3
"""Create a draft migration under a stable name, with no canonical ordinal.

    uv run python scripts/new_migration.py add_wave_colour
    uv run python scripts/new_migration.py backfill_x --depends-on add_wave_colour

Writes `rust/loopflow/src/store/migrations/drafts/<name>.sql` carrying a
`-- name:` / `-- depends_on:` header and a SQL body to fill in. A draft has no
ordinal: the release cut (`lf release run`) is the single boundary that orders
the accumulated drafts and assigns canonical `<major>.<minor>.<ordinal>` ids.

Because a draft carries no ordinal, two branches authored concurrently never
contend for or renumber one, and this script performs no `git fetch` or rebase.
Ordering that matters — a data migration that must run after another — is declared
with `--depends-on`, not by a serial number.

Stdlib only, so no Python environment is needed to author a migration.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).parent.parent
MIGRATIONS_DIR = REPO_ROOT / "rust/loopflow/src/store/migrations"
DRAFTS_DIR = MIGRATIONS_DIR / "drafts"
NAME = re.compile(r"^[a-z][a-z0-9_]*$")
MIGRATION_NAME = re.compile(r"^(\d+)\.(\d+)\.(\d{3})_([a-z0-9_]+)\.sql$")
DRAFT_NAME = re.compile(r"^([a-z][a-z0-9_]*)\.sql$")


def _released_names() -> set[str]:
    if not MIGRATIONS_DIR.is_dir():
        return set()
    return {
        match.group(4)
        for path in MIGRATIONS_DIR.iterdir()
        if (match := MIGRATION_NAME.match(path.name))
    }


def _draft_names() -> set[str]:
    if not DRAFTS_DIR.is_dir():
        return set()
    return {
        match.group(1)
        for path in DRAFTS_DIR.iterdir()
        if (match := DRAFT_NAME.match(path.name))
    }


def main() -> None:
    args = sys.argv[1:]
    depends_on: list[str] = []
    positional: list[str] = []
    index = 0
    while index < len(args):
        argument = args[index]
        if argument == "--depends-on":
            index += 1
            if index >= len(args):
                print("--depends-on needs a comma-separated list", file=sys.stderr)
                raise SystemExit(2)
            depends_on = [part.strip() for part in args[index].split(",") if part.strip()]
        else:
            positional.append(argument)
        index += 1

    if len(positional) != 1 or not NAME.fullmatch(positional[0]):
        print("usage: new_migration.py <snake_case_name> [--depends-on a,b]", file=sys.stderr)
        raise SystemExit(2)
    name = positional[0]

    released = _released_names()
    drafts = _draft_names()
    if name in released:
        print(f"{name} is already a released migration name", file=sys.stderr)
        raise SystemExit(1)
    if name in drafts:
        print(f"draft {name} already exists", file=sys.stderr)
        raise SystemExit(1)
    for dependency in depends_on:
        if not NAME.fullmatch(dependency):
            print(f"dependency {dependency!r} is not a snake_case name", file=sys.stderr)
            raise SystemExit(1)
        if dependency == name:
            print("a draft cannot depend on itself", file=sys.stderr)
            raise SystemExit(1)
        if dependency not in drafts:
            print(
                f"dependency {dependency!r} names no existing draft "
                "(a draft depends only on other drafts)",
                file=sys.stderr,
            )
            raise SystemExit(1)

    DRAFTS_DIR.mkdir(parents=True, exist_ok=True)
    path = DRAFTS_DIR / f"{name}.sql"
    dependency_line = ", ".join(depends_on)
    path.write_text(f"-- name: {name}\n-- depends_on: {dependency_line}\n")

    depends_on_rust = "&[" + ", ".join(f'"{d}"' for d in depends_on) + "]"
    print(f"created {path.relative_to(REPO_ROOT)}")
    print("\nwrite the SQL, then register the draft in the DRAFTS slice in")
    print("rust/loopflow/src/store/migrations.rs:\n")
    print(
        f"""    DraftMigration {{
        name: "{name}",
        depends_on: {depends_on_rust},
        sql: include_str!("migrations/drafts/{name}.sql"),
    }},"""
    )


if __name__ == "__main__":
    main()
