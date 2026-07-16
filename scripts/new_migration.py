#!/usr/bin/env python3
"""Create the next migration in the active release namespace.

    uv run python scripts/new_migration.py add_wave_colour

Writes `rust/loopflow/src/store/migrations/<major>.<minor>.<NNN>_<name>.sql` using
the major.minor from Cargo.toml and the next free ordinal in that namespace, then
prints the entry to append to MIGRATIONS in `store/migrations.rs`. Never renumber
or edit an existing file: once published to main, an id is history.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).parent.parent
MIGRATIONS_DIR = REPO_ROOT / "rust/loopflow/src/store/migrations"
MIGRATION_NAME = re.compile(r"^(\d+)\.(\d+)\.(\d{3})_([a-z0-9_]+)\.sql$")
VERSION_LINE = re.compile(r'^version = "([^"]+)"', re.MULTILINE)


def _authoritative_migration_names() -> list[str]:
    default = subprocess.run(
        ["git", "symbolic-ref", "--quiet", "refs/remotes/origin/HEAD"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
    ).stdout.strip()
    ref = default or "refs/remotes/origin/main"
    branch = ref.removeprefix("refs/remotes/origin/")
    subprocess.run(
        ["git", "fetch", "--quiet", "origin", branch],
        cwd=REPO_ROOT,
        capture_output=True,
    )
    listed = subprocess.run(
        [
            "git",
            "ls-tree",
            "-r",
            "--name-only",
            ref,
            "--",
            str(MIGRATIONS_DIR.relative_to(REPO_ROOT)),
        ],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
    )
    return [Path(path).name for path in listed.stdout.splitlines()] if listed.returncode == 0 else []


def main() -> None:
    if len(sys.argv) != 2 or not re.fullmatch(r"[a-z0-9_]+", sys.argv[1]):
        print("usage: new_migration.py <snake_case_name>", file=sys.stderr)
        raise SystemExit(2)
    name = sys.argv[1]

    match = VERSION_LINE.search((REPO_ROOT / "Cargo.toml").read_text())
    if not match:
        print("Cargo.toml has no version line", file=sys.stderr)
        raise SystemExit(1)
    major, minor, _ = match.group(1).split(".")

    names = [path.name for path in MIGRATIONS_DIR.iterdir()]
    names.extend(_authoritative_migration_names())
    used = [
        int(parsed.group(3))
        for migration_name in names
        if (parsed := MIGRATION_NAME.match(migration_name))
        and (parsed.group(1), parsed.group(2)) == (major, minor)
    ]
    ordinal = max(used, default=0) + 1

    path = MIGRATIONS_DIR / f"{major}.{minor}.{ordinal:03d}_{name}.sql"
    path.write_text(f"-- {name}\n")

    print(f"created {path.relative_to(REPO_ROOT)}")
    print("\nappend to MIGRATIONS in rust/loopflow/src/store/migrations.rs:\n")
    print(
        f"""    Migration {{
        id: MigrationId {{
            major: {major},
            minor: {minor},
            ordinal: {ordinal},
        }},
        name: "{name}",
        sql: include_str!("migrations/{path.name}"),
    }},"""
    )


if __name__ == "__main__":
    main()
