#!/usr/bin/env python3
"""Create this Task's migration, or point at the one it already has.

    uv run python scripts/new_migration.py add_wave_colour

Writes `rust/loopflow/src/store/migrations/<name>.sql`. The file is the
migration's registration and its name is its ledger identity; `build.rs` orders
it by the first-parent commit on origin/main that adds it. A Task keeps one
migration and edits it in place until it merges, so when the branch already
carries one that origin/main lacks, this prints that file and creates nothing.

Stdlib only, so no Python environment is needed to author a migration.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).parent.parent
MIGRATIONS_DIR = REPO_ROOT / "rust/loopflow/src/store/migrations"
NAME = re.compile(r"^[a-z][a-z0-9_]*$")
SLUG_FILE = re.compile(r"^([a-z][a-z0-9_]*)\.sql$")
DRAFT_MARKER = re.compile(r"^--[ \t]*draft:[ \t]*([a-z][a-z0-9_]*)[ \t]*$", re.MULTILINE)


def _git(*args: str) -> str | None:
    try:
        result = subprocess.run(
            ["git", *args], cwd=REPO_ROOT, capture_output=True, text=True, check=False
        )
    except OSError:
        return None
    return result.stdout if result.returncode == 0 else None


def _names() -> set[str]:
    """Every slug and every name a released batch carries."""
    names = set()
    for path in MIGRATIONS_DIR.iterdir():
        if match := SLUG_FILE.match(path.name):
            names.add(match.group(1))
        elif path.suffix == ".sql":
            names.add(path.stem.split("_", 1)[1])
            names.update(DRAFT_MARKER.findall(path.read_text()))
    return names


def _unmerged() -> list[str]:
    """Slugs present here and absent on origin/main (or main without a remote)."""
    for main in ("origin/main", "main"):
        listing = _git("ls-tree", "--name-only", main, "--", f"{MIGRATIONS_DIR.relative_to(REPO_ROOT)}/")
        if listing is None:
            continue
        merged = {Path(line).name for line in listing.splitlines()}
        return sorted(
            path.name
            for path in MIGRATIONS_DIR.iterdir()
            if SLUG_FILE.match(path.name) and path.name not in merged
        )
    return []


def _name(value: str) -> str:
    if not NAME.fullmatch(value):
        raise argparse.ArgumentTypeError(f"{value!r} is not a snake_case name")
    return value


def main() -> None:
    parser = argparse.ArgumentParser(description="Create this Task's migration.")
    parser.add_argument("name", type=_name, metavar="snake_case_name")
    name = parser.parse_args().name

    existing = _unmerged()
    if existing:
        path = MIGRATIONS_DIR / existing[0]
        print(f"this Task already has {path.relative_to(REPO_ROOT)}")
        print("\nedit it in place; a Task keeps one migration until it merges.")
        return
    if name in _names():
        print(f"{name} is already a migration name", file=sys.stderr)
        raise SystemExit(1)

    path = MIGRATIONS_DIR / f"{name}.sql"
    path.write_text("")
    print(f"created {path.relative_to(REPO_ROOT)}")
    print("\nwrite the SQL there and keep editing this one file until the Task merges.")


if __name__ == "__main__":
    main()
