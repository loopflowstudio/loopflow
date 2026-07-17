#!/usr/bin/env python3
"""Freeze the draft migration set into a canonical, ordered, ordinal-assigned tail.

    uv run python scripts/canonicalize_migrations.py 0.11.30           # patch cut
    uv run python scripts/canonicalize_migrations.py 0.12.0 --check    # plan only

The release cut is the single publication boundary that turns drafts into
canonical migrations. `lf release run` invokes this inside its release worktree,
after the version bump and before the commit, so the generated files are part of
the release PR and run under real Rust CI before the queue merges and tags. It:

  1. reads every `rust/loopflow/src/store/migrations/drafts/<name>__<id>.sql`;
  2. rejects missing, cyclic, or self dependencies, and two drafts that share a
     readable name in the same cut;
  3. topologically orders the set (dependency edges, ties broken by name), a
     total order that does not depend on merge timing, PR number, or wall clock;
  4. assigns the next contiguous ordinals in the namespace of the version being
     cut — a patch continues the current `<major>.<minor>`, a minor/major bump
     starts a fresh sequence at ordinal 1;
  5. installs the tail *atomically*: it plans everything in memory first, then
     writes the canonical `<major>.<minor>.<ordinal>_<name>.sql` files, replaces
     the MIGRATIONS registry, and deletes the drafts — and on any failure restores
     the tree byte-for-byte.

A dependency may name another draft in the cut or an already-released migration
(a released upstream is an ancestor of the whole cut, so it imposes no in-cut
ordering). Deterministic and retry-safe: the same draft set and version always
produce the same ids, files, and diff, so an aborted release regenerates
identically. The draft id is authoring-time identity only; it never appears in
canonical output. An empty draft set is a no-op.

Stdlib only, so the release path needs no Python environment — only an interpreter.
"""

from __future__ import annotations

import os
import re
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).parent.parent
MIGRATIONS_DIR = REPO_ROOT / "rust/loopflow/src/store/migrations"
DRAFTS_DIR = MIGRATIONS_DIR / "drafts"
MIGRATIONS_RS = REPO_ROOT / "rust/loopflow/src/store/migrations.rs"
MIGRATION_NAME = re.compile(r"^(\d+)\.(\d+)\.(\d{3})_([a-z0-9_]+)\.sql$")
# `<name>__<id>.sql`; the readable name never contains `__`, so the last `__`
# separates it from the immutable token.
DRAFT_FILE = re.compile(r"^([a-z][a-z0-9_]*)__([0-9a-f]+)\.sql$")
# `[ \t]` rather than `\s`: `\s` matches newlines, so an empty `-- depends_on:`
# value would swallow the newline and capture the next SQL line.
DRAFT_HEADER_NAME = re.compile(r"^--[ \t]*name:[ \t]*([a-z][a-z0-9_]*)[ \t]*$", re.MULTILINE)
DRAFT_HEADER_DEPENDS = re.compile(r"^--[ \t]*depends_on:[ \t]*(.*)$", re.MULTILINE)
HEADER_LINE = re.compile(r"^--[ \t]*(name|id|depends_on):")
VERSION = re.compile(r"^v?(\d+)\.(\d+)\.(\d+)$")


class Draft:
    def __init__(self, name: str, filename: str, depends_on: list[str], sql: str, raw: str) -> None:
        self.name = name
        self.filename = filename
        self.depends_on = depends_on
        self.sql = sql
        self.raw = raw


def _fail(message: str) -> None:
    print(f"canonicalization failed: {message}", file=sys.stderr)
    raise SystemExit(1)


def _released() -> dict[tuple[int, int], list[int]]:
    """Ordinals already released, grouped by `(major, minor)` namespace."""
    namespaces: dict[tuple[int, int], list[int]] = {}
    if not MIGRATIONS_DIR.is_dir():
        return namespaces
    for path in MIGRATIONS_DIR.iterdir():
        match = MIGRATION_NAME.match(path.name)
        if match:
            major, minor, ordinal, _ = match.groups()
            namespaces.setdefault((int(major), int(minor)), []).append(int(ordinal))
    return namespaces


def _released_names() -> set[str]:
    if not MIGRATIONS_DIR.is_dir():
        return set()
    return {
        match.group(4)
        for path in MIGRATIONS_DIR.iterdir()
        if (match := MIGRATION_NAME.match(path.name))
    }


def _draft_body(text: str) -> str:
    """The draft's SQL with its `-- name:` / `-- id:` / `-- depends_on:` header stripped."""
    body = "\n".join(line for line in text.splitlines() if not HEADER_LINE.match(line))
    body = body.strip("\n")
    return body + "\n" if body else ""


def _read_drafts() -> list[Draft]:
    drafts: list[Draft] = []
    if not DRAFTS_DIR.is_dir():
        return drafts
    for path in sorted(DRAFTS_DIR.iterdir()):
        if path.is_dir() or path.suffix != ".sql":
            continue
        match = DRAFT_FILE.match(path.name)
        if not match:
            _fail(f"draft {path.name} is not `<snake_case_name>__<id>.sql` — run scripts/new_migration.py")
        name = match.group(1)
        text = path.read_text()
        header = DRAFT_HEADER_NAME.search(text)
        if not header:
            _fail(f"draft {path.name} has no `-- name:` header")
        if header.group(1) != name:
            _fail(f"draft {path.name} header names {header.group(1)!r}, not {name!r}")
        depends = DRAFT_HEADER_DEPENDS.search(text)
        dependencies: list[str] = []
        if depends:
            raw = depends.group(1).strip()
            if raw and raw.lower() != "none":
                dependencies = [part.strip() for part in raw.split(",") if part.strip()]
        drafts.append(Draft(name, path.name, dependencies, _draft_body(text), text))
    return drafts


def _order(drafts: list[Draft]) -> list[Draft]:
    """Topological order with a name tie-break (Kahn). Deterministic and total.

    A dependency resolves against the frozen draft set or an already-released
    migration name; a released upstream imposes no in-cut edge (it precedes the
    whole cut).
    """
    by_name: dict[str, Draft] = {}
    for draft in drafts:
        if draft.name in by_name:
            _fail(
                f"two drafts share the readable name {draft.name!r} in this cut "
                "— rename one before releasing"
            )
        by_name[draft.name] = draft

    released = _released_names()
    for draft in drafts:
        if draft.name in released:
            _fail(f"draft {draft.name} collides with a released migration of the same name")
        for dependency in draft.depends_on:
            if dependency == draft.name:
                _fail(f"draft {draft.name} depends on itself")
            if dependency in by_name or dependency in released:
                continue
            _fail(
                f"draft {draft.name} depends on {dependency!r}, which is neither a "
                "draft in this cut nor an already-released migration"
            )

    indegree = {
        draft.name: len({dep for dep in draft.depends_on if dep in by_name}) for draft in drafts
    }
    dependents: dict[str, list[str]] = {draft.name: [] for draft in drafts}
    for draft in drafts:
        for dependency in set(draft.depends_on):
            if dependency in by_name:
                dependents[dependency].append(draft.name)

    ready = sorted(name for name, degree in indegree.items() if degree == 0)
    order: list[str] = []
    while ready:
        name = ready.pop(0)
        order.append(name)
        for dependent in sorted(dependents[name]):
            indegree[dependent] -= 1
            if indegree[dependent] == 0:
                ready.append(dependent)
        ready.sort()

    if len(order) != len(drafts):
        stuck = sorted(set(by_name) - set(order))
        _fail(f"draft dependencies form a cycle among: {', '.join(stuck)}")
    return [by_name[name] for name in order]


def _entry(major: int, minor: int, ordinal: int, name: str) -> str:
    return (
        f"    Migration {{\n"
        f"        id: MigrationId {{\n"
        f"            major: {major},\n"
        f"            minor: {minor},\n"
        f"            ordinal: {ordinal},\n"
        f"        }},\n"
        f'        name: "{name}",\n'
        f'        sql: include_str!("migrations/{major}.{minor}.{ordinal:03d}_{name}.sql"),\n'
        f"    }},\n"
    )


def _new_registry_text(entries: str) -> str:
    """The full migrations.rs source with `entries` appended to the registry."""
    source = MIGRATIONS_RS.read_text()
    start = source.find("const MIGRATIONS: &[Migration] = &[")
    if start == -1:
        _fail(f"{MIGRATIONS_RS.name} declares no MIGRATIONS registry")
    end = source.find("];", start)
    if end == -1:
        _fail(f"the MIGRATIONS registry in {MIGRATIONS_RS.name} is unterminated")
    return source[:end] + entries + source[end:]


def _atomic_write(path: Path, data: str) -> None:
    """Replace `path` atomically via a temp file in the same directory + os.replace."""
    handle = tempfile.NamedTemporaryFile(
        "w", dir=path.parent, prefix=path.name + ".", suffix=".tmp", delete=False
    )
    try:
        handle.write(data)
        handle.flush()
        os.fsync(handle.fileno())
        handle.close()
        os.replace(handle.name, path)
    except BaseException:
        handle.close()
        try:
            os.unlink(handle.name)
        except FileNotFoundError:
            pass
        raise


def _install(plan: list[tuple[int, int, int, Draft]], registry_text: str) -> None:
    """Write the planned tail atomically: on any failure, restore byte-for-byte.

    Order matters for rollback: create canonical files, replace the registry, then
    delete drafts last. A failure at any step undoes what came before — restoring
    deleted drafts, the original registry, and removing created files.
    """
    original_registry = MIGRATIONS_RS.read_bytes()
    created: list[Path] = []
    deleted: list[tuple[Path, bytes]] = []
    registry_written = False
    try:
        for major, minor, ordinal, draft in plan:
            canonical = MIGRATIONS_DIR / f"{major}.{minor}.{ordinal:03d}_{draft.name}.sql"
            if canonical.exists():
                raise RuntimeError(f"{canonical.name} already exists")
            canonical.write_text(draft.sql)
            created.append(canonical)
        _atomic_write(MIGRATIONS_RS, registry_text)
        registry_written = True
        for _major, _minor, _ordinal, draft in plan:
            path = DRAFTS_DIR / draft.filename
            data = path.read_bytes()
            path.unlink()
            deleted.append((path, data))
    except BaseException as error:
        for path, data in deleted:
            path.write_bytes(data)
        if registry_written:
            MIGRATIONS_RS.write_bytes(original_registry)
        for path in created:
            try:
                path.unlink()
            except FileNotFoundError:
                pass
        _fail(f"install failed; tree restored byte-for-byte ({error})")


def main() -> None:
    check = False
    positional: list[str] = []
    for argument in sys.argv[1:]:
        if argument in ("--check", "--dry-run"):
            check = True
        else:
            positional.append(argument)
    if len(positional) != 1:
        print("usage: canonicalize_migrations.py <version> [--check]", file=sys.stderr)
        raise SystemExit(2)
    version = VERSION.match(positional[0])
    if not version:
        print(f"version {positional[0]!r} is not major.minor.patch", file=sys.stderr)
        raise SystemExit(2)
    major, minor = int(version.group(1)), int(version.group(2))

    drafts = _read_drafts()
    if not drafts:
        print("no drafts to canonicalize")
        return

    ordered = _order(drafts)
    released = _released()
    next_ordinal = max(released.get((major, minor), []), default=0) + 1

    plan = []
    for offset, draft in enumerate(ordered):
        ordinal = next_ordinal + offset
        plan.append((major, minor, ordinal, draft))

    print(f"canonicalizing {len(plan)} draft(s) into {major}.{minor}:")
    for major_, minor_, ordinal, draft in plan:
        depends = ", ".join(draft.depends_on) if draft.depends_on else "(none)"
        print(f"  {major_}.{minor_}.{ordinal:03d}_{draft.name}  <- draft {draft.name} [{depends}]")
    if check:
        return

    entries = "".join(
        _entry(major_, minor_, ordinal, draft.name) for major_, minor_, ordinal, draft in plan
    )
    registry_text = _new_registry_text(entries)
    _install(plan, registry_text)
    print(f"wrote {len(plan)} canonical migration(s); run scripts/check_migrations.py to verify")


if __name__ == "__main__":
    main()
