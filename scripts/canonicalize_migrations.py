#!/usr/bin/env python3
"""Freeze the draft migration set into a canonical, ordered, ordinal-assigned tail.

    uv run python scripts/canonicalize_migrations.py 0.11.30           # patch cut
    uv run python scripts/canonicalize_migrations.py 0.12.0 --check    # plan only

The release cut is the single publication boundary that turns drafts into
canonical migrations. This is what `lf release run` invokes inside its release
worktree, before the migration gate and the PR. It:

  1. reads every `rust/loopflow/src/store/migrations/drafts/<name>.sql`;
  2. rejects missing, cyclic, or self dependencies;
  3. topologically orders the set (dependency edges, ties broken by name), a
     total order that does not depend on merge timing, PR number, or wall clock;
  4. assigns the next contiguous ordinals in the namespace of the version being
     cut — a patch continues the current `<major>.<minor>`, a minor/major bump
     starts a fresh sequence at ordinal 1;
  5. writes `<major>.<minor>.<ordinal>_<name>.sql` with the draft's SQL body,
     appends a `Migration { .. }` entry to the MIGRATIONS registry, and deletes
     the draft.

Deterministic and retry-safe: the same draft set and version always produce the
same ids, files, and diff, so an aborted release regenerates identically. An
empty draft set is a no-op.

Stdlib only, so the release path needs no Python environment — only an interpreter.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).parent.parent
MIGRATIONS_DIR = REPO_ROOT / "rust/loopflow/src/store/migrations"
DRAFTS_DIR = MIGRATIONS_DIR / "drafts"
MIGRATIONS_RS = REPO_ROOT / "rust/loopflow/src/store/migrations.rs"
MIGRATION_NAME = re.compile(r"^(\d+)\.(\d+)\.(\d{3})_([a-z0-9_]+)\.sql$")
DRAFT_FILE = re.compile(r"^([a-z][a-z0-9_]*)\.sql$")
# `[ \t]` rather than `\s`: `\s` matches newlines, so an empty `-- depends_on:`
# value would swallow the newline and capture the next SQL line.
DRAFT_HEADER_NAME = re.compile(r"^--[ \t]*name:[ \t]*([a-z][a-z0-9_]*)[ \t]*$", re.MULTILINE)
DRAFT_HEADER_DEPENDS = re.compile(r"^--[ \t]*depends_on:[ \t]*(.*)$", re.MULTILINE)
HEADER_LINE = re.compile(r"^--[ \t]*(name|depends_on):")
VERSION = re.compile(r"^v?(\d+)\.(\d+)\.(\d+)$")


class Draft:
    def __init__(self, name: str, depends_on: list[str], sql: str) -> None:
        self.name = name
        self.depends_on = depends_on
        self.sql = sql


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
    """The draft's SQL with its `-- name:` / `-- depends_on:` header stripped."""
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
            _fail(f"draft {path.name} is not `<snake_case_name>.sql`")
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
        drafts.append(Draft(name, dependencies, _draft_body(text)))
    return drafts


def _order(drafts: list[Draft]) -> list[Draft]:
    """Topological order with a name tie-break (Kahn). Deterministic and total."""
    by_name = {draft.name: draft for draft in drafts}
    released = _released_names()
    for draft in drafts:
        if draft.name in released:
            _fail(f"draft {draft.name} collides with a released migration of the same name")
        for dependency in draft.depends_on:
            if dependency == draft.name:
                _fail(f"draft {draft.name} depends on itself")
            if dependency not in by_name:
                _fail(f"draft {draft.name} depends on {dependency!r}, which is not a draft")

    indegree = {draft.name: len(set(draft.depends_on)) for draft in drafts}
    dependents: dict[str, list[str]] = {draft.name: [] for draft in drafts}
    for draft in drafts:
        for dependency in set(draft.depends_on):
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


def _registry_insert(entries: str) -> None:
    source = MIGRATIONS_RS.read_text()
    start = source.find("const MIGRATIONS: &[Migration] = &[")
    if start == -1:
        _fail(f"{MIGRATIONS_RS.name} declares no MIGRATIONS registry")
    end = source.find("];", start)
    if end == -1:
        _fail(f"the MIGRATIONS registry in {MIGRATIONS_RS.name} is unterminated")
    MIGRATIONS_RS.write_text(source[:end] + entries + source[end:])


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

    entries = ""
    for major_, minor_, ordinal, draft in plan:
        canonical = MIGRATIONS_DIR / f"{major_}.{minor_}.{ordinal:03d}_{draft.name}.sql"
        canonical.write_text(draft.sql)
        (DRAFTS_DIR / f"{draft.name}.sql").unlink()
        entries += _entry(major_, minor_, ordinal, draft.name)
    _registry_insert(entries)
    print(f"wrote {len(plan)} canonical migration(s); run scripts/check_migrations.py to verify")


if __name__ == "__main__":
    main()
