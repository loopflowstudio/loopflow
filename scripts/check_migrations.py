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

    uv run python scripts/check_migrations.py

Stdlib only, so CI can run it without a Python environment.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).parent.parent
MIGRATIONS_DIR = REPO_ROOT / "rust/loopflow/src/store/migrations"
RELEASED = re.compile(r"^\d+\.\d+\.(?:\d+\.)?\d{3}_[a-z0-9_]+\.sql$")
SLUG = re.compile(r"^([a-z][a-z0-9_]*)\.sql$")
DRAFT_MARKER = re.compile(r"^--[ \t]*draft:[ \t]*([a-z][a-z0-9_]*)[ \t]*$", re.MULTILINE)


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


def _check_against_main(local: dict[str, bytes]) -> list[str]:
    """Merged bytes are durable history; returns this branch's unmerged slugs."""
    main = _migrations_at("origin/main")
    if main is None:
        if os.environ.get("LOOPFLOW_REQUIRE_ORIGIN_MAIN") == "1":
            _fail("origin/main is unavailable in CI; the integration-history check cannot run")
        print("origin/main unavailable — skipping the integration-history check")
        return []
    for name, content in main.items():
        if name not in local:
            _fail(f"{name} is on origin/main but missing here — a merged migration is never deleted")
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


def main() -> None:
    local = _local_migrations()
    released = _released_names(local)
    for name in local:
        slug = SLUG.match(name)
        if slug and slug.group(1) in released:
            _fail(f"{name} reuses the released migration name {slug.group(1)!r}")
    unmerged = _check_against_main(local)
    _check_runtime_sql()
    # A merge-queue group may carry several Tasks' migrations at once.
    print(f"unmerged: {', '.join(unmerged)}" if unmerged else "no unmerged migration")


if __name__ == "__main__":
    main()
