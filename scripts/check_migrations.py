#!/usr/bin/env python3
"""Verify the migration set before it can be released.

Fails when a migration is malformed, namespaced ahead of the package version,
collides with another id, is not registered in Rust (or is registered under a
different id or name), diverges from current main, or — the rule that matters —
when a migration that already shipped has been edited, renamed, or deleted.

A shipped migration is immutable: databases in the wild have already run it, so
changing it changes their history, not their schema. Repair a shipped schema with
a new forward migration.

    uv run python scripts/check_migrations.py

Stdlib only, so `lf release` can run it directly without a Python environment.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).parent.parent
MIGRATIONS_DIR = REPO_ROOT / "rust/loopflow/src/store/migrations"
MIGRATIONS_RS = REPO_ROOT / "rust/loopflow/src/store/migrations.rs"
MIGRATION_NAME = re.compile(r"^(\d+)\.(\d+)\.(\d{3})_([a-z0-9_]+)\.sql$")
VERSION_LINE = re.compile(r'^version = "([^"]+)"', re.MULTILINE)

# One `Migration { .. }` entry of the MIGRATIONS registry in migrations.rs.
REGISTRY_ENTRY = re.compile(
    r"""Migration\s*\{\s*
        id:\s*MigrationId\s*\{\s*
            major:\s*(?P<major>\d+),\s*
            minor:\s*(?P<minor>\d+),\s*
            ordinal:\s*(?P<ordinal>\d+),?\s*
        \},\s*
        name:\s*"(?P<name>[a-z0-9_]+)",\s*
        sql:\s*include_str!\("migrations/(?P<file>[^"]+)"\),?\s*
    \}""",
    re.VERBOSE,
)


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


def _registry() -> list[tuple[tuple[int, int, int], str, str]]:
    """The MIGRATIONS registry in Rust, as `(id, name, file)` in declaration order.

    Read from the source rather than from a build, so the release gate needs no
    toolchain — and so a migration file that nobody registered, which would never
    run, is caught here rather than shipping as a silent no-op.
    """
    source = MIGRATIONS_RS.read_text()
    start = source.find("const MIGRATIONS: &[Migration] = &[")
    if start == -1:
        _fail(f"{MIGRATIONS_RS.name} declares no MIGRATIONS registry")
    end = source.find("];", start)
    if end == -1:
        _fail(f"the MIGRATIONS registry in {MIGRATIONS_RS.name} is unterminated")

    entries = []
    for match in REGISTRY_ENTRY.finditer(source[start:end]):
        key = (
            int(match.group("major")),
            int(match.group("minor")),
            int(match.group("ordinal")),
        )
        entries.append((key, match.group("name"), match.group("file")))
    return entries


def _check_registry(files: dict[tuple[int, int, int], str]) -> None:
    """The registry and the directory must name the same migrations, identically."""
    entries = _registry()

    registered: dict[tuple[int, int, int], str] = {}
    for key, name, file in entries:
        expected = f"{key[0]}.{key[1]}.{key[2]:03d}_{name}.sql"
        if file != expected:
            _fail(
                f"registry entry {expected} includes {file} "
                "— the id, the name, and the file must agree"
            )
        if not (MIGRATIONS_DIR / file).is_file():
            _fail(f"registry entry {file} has no file in {MIGRATIONS_DIR.name}/")
        if key in registered:
            _fail(
                f"registry entry {file} collides with {registered[key]} "
                f"at {key[0]}.{key[1]}.{key[2]:03d}"
            )
        registered[key] = file

    for key, name in sorted(files.items()):
        if key not in registered:
            _fail(
                f"{name} is not in the MIGRATIONS registry in {MIGRATIONS_RS.name} "
                "— an unregistered migration never runs"
            )

    order = [key for key, _, _ in entries]
    if order != sorted(order):
        _fail(f"the MIGRATIONS registry in {MIGRATIONS_RS.name} is not in id order")


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


def _migrations_at_ref(ref: str) -> dict[tuple[int, int, int], tuple[str, bytes]]:
    exists = subprocess.run(
        ["git", "rev-parse", "--verify", "--quiet", ref],
        cwd=REPO_ROOT,
        capture_output=True,
    )
    if exists.returncode != 0:
        return {}
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
        check=True,
    )
    migrations = {}
    for relative in listed.stdout.splitlines():
        name = Path(relative).name
        match = MIGRATION_NAME.match(name)
        if not match:
            continue
        major, minor, ordinal, _ = match.groups()
        content = subprocess.run(
            ["git", "show", f"{ref}:{relative}"],
            cwd=REPO_ROOT,
            capture_output=True,
            check=True,
        ).stdout
        migrations[(int(major), int(minor), int(ordinal))] = (name, content)
    return migrations


def _check_current_main(local: dict[tuple[int, int, int], str]) -> None:
    """Main is already durable history even before its next release tag."""
    canonical = _migrations_at_ref("origin/main")
    if not canonical:
        if os.environ.get("GITHUB_ACTIONS") == "true":
            _fail("origin/main is unavailable in CI; the integration-history check cannot run")
        print("origin/main unavailable — skipping the integration-history check")
        return
    for key, (canonical_name, canonical_content) in canonical.items():
        local_name = local.get(key)
        if local_name is None:
            _fail(f"{canonical_name} exists on origin/main but is missing from this branch")
        if local_name != canonical_name:
            identity = f"{key[0]}.{key[1]}.{key[2]:03d}"
            _fail(
                f"{local_name} collides with {canonical_name} on origin/main at {identity}; "
                "rebase and allocate a new migration"
            )
        if (MIGRATIONS_DIR / local_name).read_bytes() != canonical_content:
            _fail(f"{local_name} differs from origin/main and is already durable history")


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

    _check_registry(ids)
    _check_current_main(ids)

    # Deterministic order across namespaces is the numeric tuple, never the string:
    # `0.10.001` sorts before `0.9.001` lexically.
    print("migration order:")
    for key in sorted(ids):
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
