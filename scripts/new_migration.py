#!/usr/bin/env python3
"""Create this Task's draft migration, or point at the one it already has.

    uv run python scripts/new_migration.py add_wave_colour
    uv run python scripts/new_migration.py backfill_x --depends-on add_wave_colour

Writes `rust/loopflow/src/store/migrations/drafts/<name>.sql`. A Task keeps one
draft and edits it in place until it lands, so when the branch already carries a
draft that its merge base with main lacks, this prints that file and creates
nothing. A draft has no ordinal: the release cut (`lf repo release run`) orders
the accumulated drafts and publishes one canonical
`<major>.<minor>.<patch>.001_release` batch.

The file is the draft's registration; there is nothing to paste into
`migration_catalog.rs`. Ordering against another Task's draft or a released
migration is declared with `--depends-on`, not a serial number.

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
DRAFTS_DIR = MIGRATIONS_DIR / "drafts"
NAME = re.compile(r"^[a-z][a-z0-9_]*$")
MIGRATION_NAME = re.compile(r"^(\d+)\.(\d+)\.(?:(\d+)\.)?(\d{3})_([a-z0-9_]+)\.sql$")
DRAFT_MARKER = re.compile(r"^--[ \t]*draft:[ \t]*([a-z][a-z0-9_]*)[ \t]*$", re.MULTILINE)
DRAFT_FILE = re.compile(r"^([a-z][a-z0-9_]*)\.sql$")


def _released_names() -> set[str]:
    if not MIGRATIONS_DIR.is_dir():
        return set()
    names = set()
    for path in MIGRATIONS_DIR.iterdir():
        match = MIGRATION_NAME.match(path.name)
        if not match:
            continue
        if match.group(3) is None:
            names.add(match.group(5))
        else:
            names.update(DRAFT_MARKER.findall(path.read_text()))
    return names


def _draft_names() -> set[str]:
    if not DRAFTS_DIR.is_dir():
        return set()
    return {
        match.group(1) for path in DRAFTS_DIR.iterdir() if (match := DRAFT_FILE.match(path.name))
    }


def _git(*args: str) -> str | None:
    try:
        result = subprocess.run(
            ["git", *args], cwd=REPO_ROOT, capture_output=True, text=True, check=False
        )
    except OSError:
        return None
    return result.stdout if result.returncode == 0 else None


def _task_drafts(drafts: set[str]) -> list[str]:
    """Drafts this branch added: present here, absent at its merge base with main."""
    for main in ("origin/main", "main"):
        base = _git("merge-base", "HEAD", main)
        if base is None:
            continue
        listing = _git(
            "ls-tree", "--name-only", f"{base.strip()}:{DRAFTS_DIR.relative_to(REPO_ROOT)}"
        )
        inherited = {
            match.group(1)
            for line in (listing or "").splitlines()
            if (match := DRAFT_FILE.match(line))
        }
        return sorted(drafts - inherited)
    return []


def _name(value: str) -> str:
    if not NAME.fullmatch(value):
        raise argparse.ArgumentTypeError(f"{value!r} is not a snake_case name")
    return value


def main() -> None:
    parser = argparse.ArgumentParser(description="Create this Task's draft migration.")
    parser.add_argument("name", type=_name, metavar="snake_case_name")
    parser.add_argument("--depends-on", default="", metavar="a,b")
    args = parser.parse_args()
    name = args.name
    depends_on = [part.strip() for part in args.depends_on.split(",") if part.strip()]

    drafts = _draft_names()
    existing = _task_drafts(drafts)
    if existing:
        path = DRAFTS_DIR / f"{existing[0]}.sql"
        print(f"this Task already has {path.relative_to(REPO_ROOT)}")
        print("\nedit it in place; a Task keeps one draft until it lands.")
        return

    released = _released_names()
    if name in released or name in drafts:
        print(f"{name} is already a migration name", file=sys.stderr)
        raise SystemExit(1)
    for dependency in depends_on:
        if dependency == name:
            print("a draft cannot depend on itself", file=sys.stderr)
            raise SystemExit(1)
        if dependency not in drafts and dependency not in released:
            print(
                f"dependency {dependency!r} names no draft or released migration",
                file=sys.stderr,
            )
            raise SystemExit(1)

    DRAFTS_DIR.mkdir(parents=True, exist_ok=True)
    path = DRAFTS_DIR / f"{name}.sql"
    path.write_text(f"-- depends_on: {', '.join(depends_on)}\n" if depends_on else "")

    print(f"created {path.relative_to(REPO_ROOT)}")
    print("\nwrite the SQL there and keep editing this one file until the Task lands.")


if __name__ == "__main__":
    main()
