#!/usr/bin/env python3
"""Freeze the draft migration set into one release-scoped canonical batch.

    uv run python scripts/canonicalize_migrations.py 0.12.2 --release-cut
    uv run python scripts/canonicalize_migrations.py 0.12.2 --check

The release cut is the single publication boundary that turns drafts into
canonical migrations. `lf release run` invokes this inside its release worktree,
after the version bump and before the commit, so the generated files are part of
the release PR and run under real Rust CI before the queue merges and tags. It:

  1. reads every `rust/loopflow/src/store/migrations/drafts/<name>__<id>.sql`;
  2. rejects missing, cyclic, or self dependencies, and two drafts that share a
     readable name in the same cut;
  3. topologically orders the set (dependency edges, ties broken by name), a
     total order that does not depend on merge timing, PR number, or wall clock;
  4. concatenates the ordered SQL into the release's single canonical
     `<major>.<minor>.<patch>.001_release.sql` batch, retaining `-- draft:`
     provenance markers;
  5. installs the batch *atomically*: it plans everything in memory first, then
     writes the canonical file, replaces the MIGRATIONS registry, and deletes
     the drafts — and on any failure restores the tree byte-for-byte.

A dependency may name another draft in the cut or an already-released migration
(a released upstream is an ancestor of the whole cut, so it imposes no in-cut
ordering). Deterministic and retry-safe: the same draft set and version always
produce the same ids, files, and diff, so an aborted release regenerates
identically. The draft id is authoring-time identity only; it never appears in
canonical output. An empty draft set is a no-op.

Writing requires explicit release-cut authority. CI may materialize the same tree
in its disposable checkout with `--materialize-for-tests`; when the active
package version has already published a batch, that authority models the next
patch and bumps the disposable Cargo workspace version with it. All other
invocations are previews. Stdlib only, so the release path needs no Python
environment — only an interpreter.
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
PACKAGE_MANIFEST = REPO_ROOT / "Cargo.toml"
MIGRATION_NAME = re.compile(r"^(\d+)\.(\d+)\.(?:(\d+)\.)?(\d{3})_([a-z0-9_]+)\.sql$")
# `<name>__<id>.sql`; the readable name never contains `__`, so the last `__`
# separates it from the immutable 128-bit token (32 hex chars).
DRAFT_FILE = re.compile(r"^([a-z][a-z0-9_]*)__([0-9a-f]{32})\.sql$")
DRAFT_ID = re.compile(r"^[0-9a-f]{32}$")
# `[ \t]` rather than `\s`: `\s` matches newlines, so an empty `-- depends_on:`
# value would swallow the newline and capture the next SQL line.
DRAFT_HEADER_NAME = re.compile(r"^--[ \t]*name:[ \t]*([a-z][a-z0-9_]*)[ \t]*$", re.MULTILINE)
DRAFT_HEADER_ID = re.compile(r"^--[ \t]*id:[ \t]*(.*)$", re.MULTILINE)
DRAFT_HEADER_DEPENDS = re.compile(r"^--[ \t]*depends_on:[ \t]*(.*)$", re.MULTILINE)
HEADER_LINE = re.compile(r"^--[ \t]*(name|id|depends_on):")
DRAFT_MARKER = re.compile(r"^--[ \t]*draft:[ \t]*([a-z][a-z0-9_]*)[ \t]*$", re.MULTILINE)
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


def _released_names() -> set[str]:
    """Draft identities already published, including names inside release batches."""
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
            _fail(
                f"draft {path.name} is not `<snake_case_name>__<id>.sql` "
                "— run scripts/new_migration.py"
            )
        name, file_id = match.group(1), match.group(2)
        text = path.read_text()
        if DRAFT_MARKER.search(text):
            _fail(f"draft {path.name} uses reserved `-- draft:` release provenance")
        header = DRAFT_HEADER_NAME.search(text)
        if not header:
            _fail(f"draft {path.name} has no `-- name:` header")
        if header.group(1) != name:
            _fail(f"draft {path.name} header names {header.group(1)!r}, not {name!r}")
        id_header = DRAFT_HEADER_ID.search(text)
        if not id_header:
            _fail(f"draft {path.name} has no `-- id:` header")
        header_id = id_header.group(1).strip()
        if not DRAFT_ID.fullmatch(header_id):
            _fail(
                f"draft {path.name} id {header_id!r} is not a 128-bit token "
                "(32 hex chars) — run scripts/new_migration.py"
            )
        if header_id != file_id:
            _fail(f"draft {path.name} header id {header_id!r} disagrees with its filename")
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


def _entry(major: int, minor: int, patch: int) -> str:
    return (
        f"    Migration {{\n"
        f"        id: MigrationId {{\n"
        f"            major: {major},\n"
        f"            minor: {minor},\n"
        f"            patch: Some({patch}),\n"
        f"            ordinal: 1,\n"
        f"        }},\n"
        f'        name: "release",\n'
        f'        sql: include_str!("migrations/{major}.{minor}.{patch}.001_release.sql"),\n'
        f"    }},\n"
    )


def _batch_sql(drafts: list[Draft]) -> str:
    blocks = []
    for draft in drafts:
        body = draft.sql.rstrip("\n")
        block = f"-- draft: {draft.name}"
        if body:
            block += f"\n{body}"
        blocks.append(block)
    return "\n\n".join(blocks) + "\n"


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


def _test_package_manifest(current: str, next_version: str) -> str:
    if not PACKAGE_MANIFEST.is_file():
        _fail(f"test materialization cannot find {PACKAGE_MANIFEST.name}")
    lines = PACKAGE_MANIFEST.read_text().splitlines(keepends=True)
    in_workspace_package = False
    for index, line in enumerate(lines):
        stripped = line.strip()
        if stripped == "[workspace.package]":
            in_workspace_package = True
            continue
        if stripped.startswith("["):
            in_workspace_package = False
        if not in_workspace_package:
            continue
        body = line.rstrip("\r\n")
        newline = line[len(body) :]
        match = re.match(r'^(\s*version\s*=\s*")([^"]+)(".*)$', body)
        if match is None:
            continue
        if match.group(2) != current:
            _fail(
                f"{PACKAGE_MANIFEST.name} workspace version {match.group(2)!r} "
                f"does not match requested test version {current!r}"
            )
        lines[index] = f"{match.group(1)}{next_version}{match.group(3)}{newline}"
        return "".join(lines)
    _fail(f"{PACKAGE_MANIFEST.name} has no [workspace.package] version")


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


def _install(
    canonical: Path,
    sql: str,
    drafts: list[Draft],
    registry_text: str,
    package_manifest_text: str | None,
) -> None:
    """Write the planned batch atomically: on any failure, restore byte-for-byte.

    Order matters for rollback: create canonical files, replace the registry, then
    delete drafts last. A failure at any step undoes what came before — restoring
    deleted drafts, the original registry, and removing created files.
    """
    original_registry = MIGRATIONS_RS.read_bytes()
    original_manifest = PACKAGE_MANIFEST.read_bytes() if package_manifest_text else None
    deleted: list[tuple[Path, bytes]] = []
    registry_written = False
    manifest_written = False
    canonical_written = False
    try:
        if canonical.exists():
            raise RuntimeError(f"{canonical.name} already exists")
        canonical.write_text(sql)
        canonical_written = True
        _atomic_write(MIGRATIONS_RS, registry_text)
        registry_written = True
        if package_manifest_text is not None:
            _atomic_write(PACKAGE_MANIFEST, package_manifest_text)
            manifest_written = True
        for draft in drafts:
            path = DRAFTS_DIR / draft.filename
            data = path.read_bytes()
            path.unlink()
            deleted.append((path, data))
    except BaseException as error:
        for path, data in deleted:
            path.write_bytes(data)
        if manifest_written:
            assert original_manifest is not None
            PACKAGE_MANIFEST.write_bytes(original_manifest)
        if registry_written:
            MIGRATIONS_RS.write_bytes(original_registry)
        if canonical_written:
            try:
                canonical.unlink()
            except FileNotFoundError:
                pass
        _fail(f"install failed; tree restored byte-for-byte ({error})")


def main() -> None:
    check = False
    write_mode: str | None = None
    positional: list[str] = []
    for argument in sys.argv[1:]:
        if argument in ("--check", "--dry-run"):
            check = True
        elif argument in ("--release-cut", "--materialize-for-tests"):
            if write_mode is not None:
                print("choose one canonical migration write authority", file=sys.stderr)
                raise SystemExit(2)
            write_mode = argument
        else:
            positional.append(argument)
    if len(positional) != 1:
        print(
            "usage: canonicalize_migrations.py <version> "
            "[--check|--release-cut|--materialize-for-tests]",
            file=sys.stderr,
        )
        raise SystemExit(2)
    if check and write_mode is not None:
        print("--check cannot be combined with a write authority", file=sys.stderr)
        raise SystemExit(2)
    if not check and write_mode is None:
        print(
            "refusing to create canonical migrations outside the release cut; "
            "use --check to preview",
            file=sys.stderr,
        )
        raise SystemExit(2)
    version = VERSION.match(positional[0])
    if not version:
        print(f"version {positional[0]!r} is not major.minor.patch", file=sys.stderr)
        raise SystemExit(2)
    major, minor, patch = (int(version.group(index)) for index in range(1, 4))

    drafts = _read_drafts()
    if not drafts:
        print("no drafts to canonicalize")
        return

    ordered = _order(drafts)
    release_files = []
    for path in MIGRATIONS_DIR.iterdir():
        match = MIGRATION_NAME.match(path.name)
        if not match or match.group(3) is None:
            continue
        namespace = tuple(int(match.group(index)) for index in range(1, 4))
        if namespace == (major, minor, patch):
            release_files.append(path.name)
    package_manifest_text = None
    if release_files:
        if write_mode != "--materialize-for-tests":
            _fail(
                f"release {major}.{minor}.{patch} already has canonical migration(s): "
                f"{', '.join(sorted(release_files))}"
            )
        current = f"{major}.{minor}.{patch}"
        patch += 1
        next_version = f"{major}.{minor}.{patch}"
        next_file = MIGRATIONS_DIR / f"{next_version}.001_release.sql"
        if next_file.exists():
            _fail(f"test release {next_version} already has canonical migration {next_file.name}")
        package_manifest_text = _test_package_manifest(current, next_version)
        print(f"test materialization advances the disposable package {current} -> {next_version}")

    canonical = MIGRATIONS_DIR / f"{major}.{minor}.{patch}.001_release.sql"
    print(f"canonicalizing {len(ordered)} draft(s) into {major}.{minor}.{patch}.001_release:")
    for draft in ordered:
        depends = ", ".join(draft.depends_on) if draft.depends_on else "(none)"
        print(f"  draft {draft.name} [{depends}]")
    if check:
        return

    registry_text = _new_registry_text(_entry(major, minor, patch))
    _install(canonical, _batch_sql(ordered), ordered, registry_text, package_manifest_text)
    print(
        f"wrote 1 canonical migration from {len(ordered)} draft(s); "
        "run scripts/check_migrations.py to verify"
    )


if __name__ == "__main__":
    main()
