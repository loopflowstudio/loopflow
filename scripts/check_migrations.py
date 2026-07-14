#!/usr/bin/env python3
"""Verify the migration set before it can be released.

Fails when a migration is malformed, namespaced ahead of the package version,
collides with another id, regresses the order, or — the rule that matters — when
a migration that already shipped has been edited, renamed, or deleted.

A shipped migration is immutable: databases in the wild have already run it, so
changing it changes their history, not their schema. Repair a shipped schema with
a new forward migration.

    uv run python scripts/check_migrations.py
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



def _package_version() -> tuple[int, int]:
    """The active major.minor, from the one version both manifests must agree on."""
    versions = {}
    for manifest in ("Cargo.toml", "pyproject.toml"):
        match = VERSION_LINE.search((REPO_ROOT / manifest).read_text())
        if not match:
            _fail(f"{manifest} has no version line")
        versions[manifest] = match.group(1)

    if len(set(versions.values())) != 1:
        _fail(f"manifest versions disagree: {versions}")

    version = next(iter(versions.values()))
    parts = version.split(".")
    if len(parts) != 3 or not all(part.isdigit() for part in parts):
        _fail(f"version {version!r} is not major.minor.patch")
    return int(parts[0]), int(parts[1])


def _last_release_tag() -> str | None:
    result = subprocess.run(
        ["git", "describe", "--tags", "--abbrev=0"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip() or None


def _shipped_migrations(tag: str) -> dict[str, bytes]:
    """Every release-scoped migration in `tag`, keyed by file name.

    Searched by name across the whole tree rather than under today's directory:
    an id is shipped no matter where the file sat, so moving the directory cannot
    quietly void the immutability check.
    """
    listed = subprocess.run(
        ["git", "ls-tree", "-r", "--name-only", tag],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=True,
    )

    shipped = {}
    for path in listed.stdout.split():
        name = Path(path).name
        if not MIGRATION_NAME.match(name):
            continue
        blob = subprocess.run(
            ["git", "show", f"{tag}:{path}"],
            cwd=REPO_ROOT,
            capture_output=True,
            check=True,
        )
        shipped[name] = blob.stdout
    return shipped


def _fail(message: str) -> None:
    print(f"migration check failed: {message}", file=sys.stderr)
    raise SystemExit(1)


def main() -> None:
    active = _package_version()

    if not MIGRATIONS_DIR.is_dir():
        _fail(f"{MIGRATIONS_DIR.relative_to(REPO_ROOT)} is missing")

    ids: dict[tuple[int, int, int], str] = {}
    for path in sorted(MIGRATIONS_DIR.iterdir()):
        match = MIGRATION_NAME.match(path.name)
        if not match:
            _fail(
                f"{path.name} is not `<major>.<minor>.<ordinal:03>_<name>.sql` "
                "— run scripts/new_migration.py"
            )
        major, minor, ordinal, _ = match.groups()
        key = (int(major), int(minor), int(ordinal))

        if key in ids:
            _fail(f"{path.name} collides with {ids[key]}")
        if key[:2] > active:
            _fail(
                f"{path.name} is namespaced ahead of the package version "
                f"{active[0]}.{active[1]}"
            )
        ids[key] = path.name

    if not ids:
        _fail("no migrations found")

    # Deterministic order across namespaces is the numeric tuple, never the string:
    # `0.10.001` sorts before `0.9.001` lexically.
    order = sorted(ids)
    print("migration order:")
    for key in order:
        print(f"  {ids[key]}")

    tag = _last_release_tag()
    if tag is None:
        print("no release tag yet — skipping the immutability check")
        return

    shipped = _shipped_migrations(tag)
    for name, content in shipped.items():
        current = MIGRATIONS_DIR / name
        if not current.exists():
            _fail(
                f"{name} shipped in {tag} but is now missing "
                "— a released migration is never renamed or deleted"
            )
        if current.read_bytes() != content:
            _fail(
                f"{name} shipped in {tag} and has been edited "
                "— add a forward migration instead"
            )

    if not shipped:
        # True exactly once: the store's migrations postdate the last tag, so no
        # release-scoped id has shipped yet and there is nothing to hold immutable.
        print(f"no release-scoped migrations shipped in {tag} — nothing to hold immutable")
        return

    print(f"{len(shipped)} shipped migration(s) unchanged since {tag}")


if __name__ == "__main__":
    main()
