#!/usr/bin/env python3
"""Create a draft migration under a stable name and an immutable id, no ordinal.

    uv run python scripts/new_migration.py add_wave_colour
    uv run python scripts/new_migration.py backfill_x --depends-on add_wave_colour

Writes `rust/loopflow/src/store/migrations/drafts/<name>__<id>.sql` carrying a
`-- name:` / `-- id:` / `-- depends_on:` header and a SQL body to fill in. The
`<id>` is an immutable 128-bit token (32 hex chars) minted here; the `<name>` is
the readable label and the `--depends-on` handle. 128 bits is materially
collision-resistant — two branches would need on the order of 2**64 same-name
drafts before a birthday collision, so distinct files are a guarantee, not a
hope. A draft has no ordinal: the release cut
(`lf release run`) is the single boundary that orders the accumulated drafts and
publishes one canonical `<major>.<minor>.<patch>.001_release` batch.

Because two branches authoring the same readable name mint different ids, they
write different files and never collide or share an edit — and this script edits
no shared Rust registry. A draft's registration *is* its file; canonicalization
discovers it by scanning the directory. There is nothing to paste into
`migrations.rs`; the release cut appends the canonical `Migration` entries it
generates. This script performs no `git fetch` or rebase. Ordering that matters is
declared with `--depends-on` — against another draft or an already-released
migration name — not by a serial number.

Stdlib only, so no Python environment is needed to author a migration.
"""

from __future__ import annotations

import re
import secrets
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).parent.parent
MIGRATIONS_DIR = REPO_ROOT / "rust/loopflow/src/store/migrations"
DRAFTS_DIR = MIGRATIONS_DIR / "drafts"
NAME = re.compile(r"^[a-z][a-z0-9_]*$")
MIGRATION_NAME = re.compile(r"^(\d+)\.(\d+)\.(?:(\d+)\.)?(\d{3})_([a-z0-9_]+)\.sql$")
DRAFT_MARKER = re.compile(r"^--[ \t]*draft:[ \t]*([a-z][a-z0-9_]*)[ \t]*$", re.MULTILINE)
# A draft file is `<name>__<id>.sql`; the name never contains `__`, so the last
# `__` separates the readable name from the immutable 128-bit token (32 hex chars).
DRAFT_ID = re.compile(r"^[0-9a-f]{32}$")
DRAFT_FILE = re.compile(r"^([a-z][a-z0-9_]*)__([0-9a-f]{32})\.sql$")


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

    if len(positional) != 1 or not NAME.fullmatch(positional[0]) or "__" in positional[0]:
        print(
            "usage: new_migration.py <snake_case_name> [--depends-on a,b]\n"
            "(the name is a single-underscore snake_case label; `__` is reserved)",
            file=sys.stderr,
        )
        raise SystemExit(2)
    name = positional[0]

    released = _released_names()
    drafts = _draft_names()
    if name in released:
        print(f"{name} is already a released migration name", file=sys.stderr)
        raise SystemExit(1)
    # A same-name *draft* is allowed: it mints a distinct id, so the files differ and
    # neither branch shares an edit. The clash (if it survives to one release) is a
    # canonicalization-time error, never a merge conflict here.
    for dependency in depends_on:
        if not NAME.fullmatch(dependency) or "__" in dependency:
            print(f"dependency {dependency!r} is not a snake_case name", file=sys.stderr)
            raise SystemExit(1)
        if dependency == name:
            print("a draft cannot depend on itself", file=sys.stderr)
            raise SystemExit(1)
        if dependency not in drafts and dependency not in released:
            print(
                f"dependency {dependency!r} names no draft or released migration "
                "(a draft depends on another draft or an already-released migration)",
                file=sys.stderr,
            )
            raise SystemExit(1)

    draft_id = secrets.token_hex(16)
    DRAFTS_DIR.mkdir(parents=True, exist_ok=True)
    path = DRAFTS_DIR / f"{name}__{draft_id}.sql"
    dependency_line = ", ".join(depends_on)
    path.write_text(f"-- name: {name}\n-- id: {draft_id}\n-- depends_on: {dependency_line}\n")

    print(f"created {path.relative_to(REPO_ROOT)}")
    print("\nwrite the SQL below the header. Nothing else to do — the file is the")
    print("draft's registration, and the release cut assigns its canonical id.")


if __name__ == "__main__":
    main()
