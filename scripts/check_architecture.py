#!/usr/bin/env python3
"""Check Loopflow's bounded architecture map against the current tree.

    uv run python scripts/check_architecture.py
    uv run python scripts/check_architecture.py --json

The check is deliberately finite. It covers live SQLite tables, root CLI
families, executable/process entrypoints, Wave and Home-daemon HTTP routes,
provider kinds, literal Rust subprocess edges, declared read projections and
compatibility seams, and exact retired vocabulary. It does not claim that every
public Rust item is an architectural concept.
"""

from __future__ import annotations

import argparse
import fnmatch
import json
import re
import sqlite3
import sys
from collections import Counter
from dataclasses import asdict, dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
ARCHITECTURE = Path("docs/architecture-reference.md")
MIGRATIONS = Path("rust/loopflow/src/store/migrations")
MIGRATIONS_RS = Path("rust/loopflow/src/store/migrations.rs")
LF_MOD = Path("rust/loopflow/src/lf/mod.rs")
CRATE_MANIFEST = Path("rust/loopflow/Cargo.toml")
WAVE_SERVER = Path("rust/loopflow/src/wave/server.rs")
LFD_SERVER = Path("rust/loopflow/src/lfd/mod.rs")
PROVIDERS = Path("rust/loopflow/src/provider_auth/mod.rs")
FLOWS = Path(".lf/flows")

CODE_TOKEN = re.compile(r"`([^`]+)`")
MARKDOWN_LINK = re.compile(r"\[([^]]+)]\(([^)]+)\)")
MIGRATION_INCLUDE = re.compile(r'include_str!\("migrations/([^\"]+)"\)')
ROUTE = re.compile(r'\.route\(\s*"([^"]+)"\s*,\s*(get|post|put|delete|patch)\(', re.S)
COMMAND_EDGE = re.compile(r"(?:std::process::|tokio::process::)?Command::new\(\s*\"([^\"]+)\"")
SHIM_MARKER = re.compile(r"architecture-shim:\s*([a-z0-9-]+)")
HEADER_LINE = re.compile(r"^--[ \t]*(name|id|depends_on):")
DRAFT_NAME = re.compile(r"^--[ \t]*name:[ \t]*([a-z][a-z0-9_]*)[ \t]*$", re.MULTILINE)
DRAFT_DEPENDS = re.compile(r"^--[ \t]*depends_on:[ \t]*(.*)$", re.MULTILINE)
FLOW_OP = re.compile(r"^\s*-\s*op:\s*([a-z0-9_-]+)\s*$", re.MULTILINE)

TEXT_SUFFIXES = {".md", ".py", ".rs", ".sh", ".sql", ".swift", ".toml", ".yaml", ".yml"}
SCAN_ROOTS = (
    Path("README.md"),
    Path("PROMPTS.md"),
    Path("RELEASE_NOTES.md"),
    Path("STYLE.md"),
    Path("TESTING.md"),
    Path("VISUAL_DESIGN.md"),
    Path("deploy"),
    Path("docs"),
    Path("release"),
    Path("skills"),
    Path("scripts"),
    Path("python/loopflow"),
    Path("rust/loopflow/src"),
    Path("swift/Loopflow"),
    Path("swift/LoopflowMac"),
    Path("swift/DESIGN.md"),
    Path("swift/README.md"),
    Path("website"),
    Path(".lf"),
)
IGNORED_PARTS = {".git", ".venv", "node_modules", "target", "DerivedData", "__pycache__"}
IGNORED_PREFIXES = (Path("website/docs"),)


@dataclass(frozen=True)
class Coverage:
    name: str
    mapped: int
    discovered: int


@dataclass
class Report:
    coverage: list[Coverage]
    errors: list[str]

    @property
    def ok(self) -> bool:
        return not self.errors

    def to_dict(self) -> dict[str, object]:
        return {
            "schema_version": 1,
            "ok": self.ok,
            "coverage": [asdict(item) for item in self.coverage],
            "errors": self.errors,
        }


def _section(text: str, name: str) -> str:
    start = f"<!-- architecture-{name}:start -->"
    end = f"<!-- architecture-{name}:end -->"
    try:
        return text.split(start, 1)[1].split(end, 1)[0]
    except IndexError as error:
        raise ValueError(f"architecture document is missing {start} / {end}") from error


def _table(section: str) -> list[dict[str, str]]:
    lines = [line.strip() for line in section.splitlines() if line.strip().startswith("|")]
    if len(lines) < 2:
        raise ValueError("architecture section has no Markdown table")

    def cells(line: str) -> list[str]:
        return [cell.strip() for cell in line.strip().strip("|").split("|")]

    headers = [header.casefold() for header in cells(lines[0])]
    rows: list[dict[str, str]] = []
    for line in lines[2:]:
        values = cells(line)
        if len(values) != len(headers):
            raise ValueError(
                f"architecture table row has {len(values)} cells, expected {len(headers)}"
            )
        rows.append(dict(zip(headers, values, strict=True)))
    return rows


def _tokens(rows: list[dict[str, str]], *columns: str) -> Counter[str]:
    tokens: Counter[str] = Counter()
    for row in rows:
        for column in columns:
            tokens.update(CODE_TOKEN.findall(row.get(column, "")))
    return tokens


def _extract_braced(source: str, marker: str) -> str:
    start = source.find(marker)
    if start == -1:
        raise ValueError(f"source is missing {marker}")
    opening = source.find("{", start)
    if opening == -1:
        raise ValueError(f"{marker} has no opening brace")
    depth = 0
    for index in range(opening, len(source)):
        character = source[index]
        if character == "{":
            depth += 1
        elif character == "}":
            depth -= 1
            if depth == 0:
                return source[opening + 1 : index]
    raise ValueError(f"{marker} has no closing brace")


def _kebab(name: str) -> str:
    return re.sub(r"(?<!^)(?=[A-Z])", "-", name).lower()


def _discover_commands(root: Path) -> tuple[set[str], set[str]]:
    body = _extract_braced((root / LF_MOD).read_text(), "pub enum Commands")
    commands: set[str] = set()
    internal: set[str] = set()
    pending_attributes = ""
    attribute_depth = 0

    for line in body.splitlines():
        if line.startswith("    #[") or attribute_depth:
            pending_attributes += " " + line.strip()
            attribute_depth += line.count("[") - line.count("]")
            continue
        match = re.match(r"^    ([A-Z][A-Za-z0-9_]*)\s*(?:\{|\(|,)", line)
        if match is None:
            continue
        variant = match.group(1)
        if variant == "External":
            pending_attributes = ""
            continue
        custom = re.search(r'\bname\s*=\s*"([^"]+)"', pending_attributes)
        name = custom.group(1) if custom else _kebab(variant)
        token = f"lf {name}"
        commands.add(token)
        if name.startswith("__"):
            internal.add(token)
        pending_attributes = ""
    return commands, internal


def _discover_internal_flow_commands(root: Path, internal: set[str]) -> Counter[str]:
    commands: set[str] = set()
    flows = root / FLOWS
    if not flows.is_dir():
        return Counter()
    for path in sorted((*flows.glob("*.yaml"), *flows.glob("*.yml"))):
        for name in FLOW_OP.findall(path.read_text()):
            command = f"lf {name}"
            if command in internal:
                commands.add(command)
    return Counter(commands)


def _discover_binaries(root: Path) -> set[str]:
    binaries: set[str] = set()
    in_binary = False
    for line in (root / CRATE_MANIFEST).read_text().splitlines():
        stripped = line.strip()
        if stripped == "[[bin]]":
            in_binary = True
            continue
        if stripped.startswith("["):
            in_binary = False
            continue
        if not in_binary:
            continue
        match = re.match(r'^name\s*=\s*"([^"]+)"$', stripped)
        if match:
            binaries.add(match.group(1))
    return binaries


def _discover_routes(root: Path) -> set[str]:
    routes: set[str] = set()
    for owner, path in (("wave", WAVE_SERVER), ("lfd", LFD_SERVER)):
        source = (root / path).read_text()
        for route, method in ROUTE.findall(source):
            routes.add(f"{owner} {method.upper()} {route}")
    return routes


def _discover_providers(root: Path) -> set[str]:
    source = (root / PROVIDERS).read_text()
    implementation = _extract_braced(source, "pub fn as_str")
    mappings = re.findall(r'Self::[A-Za-z0-9_]+\s*=>\s*"([^"]+)"', implementation)
    return {f"provider:{name}" for name in mappings}


def _production_rust(source: str) -> str:
    pattern = re.compile(r"#\[cfg\(test\)]\s*mod\s+[A-Za-z0-9_]+\s*\{")
    while match := pattern.search(source):
        opening = source.find("{", match.start())
        depth = 0
        closing = None
        for index in range(opening, len(source)):
            if source[index] == "{":
                depth += 1
            elif source[index] == "}":
                depth -= 1
                if depth == 0:
                    closing = index
                    break
        if closing is None:
            return source[: match.start()]
        source = source[: match.start()] + source[closing + 1 :]
    return source


def _discover_executable_edges(root: Path) -> set[str]:
    edges: set[str] = set()
    source_root = root / "rust/loopflow/src"
    for path in sorted(source_root.rglob("*.rs")):
        relative = path.relative_to(root)
        if "tests" in relative.parts or MIGRATIONS in relative.parents:
            continue
        source = _production_rust(path.read_text())
        edges.update(f"exec:{name}" for name in COMMAND_EDGE.findall(source))
    return edges


def _registered_migrations(root: Path) -> list[Path]:
    source = (root / MIGRATIONS_RS).read_text()
    registry = source.split("const MIGRATIONS: &[Migration] = &[", 1)[1].split("];", 1)[0]
    return [root / MIGRATIONS / name for name in MIGRATION_INCLUDE.findall(registry)]


def _ordered_draft_sql(root: Path) -> list[str]:
    drafts_dir = root / MIGRATIONS / "drafts"
    if not drafts_dir.is_dir():
        return []
    drafts: dict[str, tuple[set[str], str]] = {}
    for path in sorted(drafts_dir.glob("*.sql")):
        text = path.read_text()
        name_match = DRAFT_NAME.search(text)
        if name_match is None:
            raise ValueError(f"draft {path.name} has no name")
        name = name_match.group(1)
        depends_match = DRAFT_DEPENDS.search(text)
        dependencies = set()
        if depends_match:
            raw = depends_match.group(1).strip()
            if raw and raw.casefold() != "none":
                dependencies = {part.strip() for part in raw.split(",") if part.strip()}
        sql = "\n".join(line for line in text.splitlines() if not HEADER_LINE.match(line))
        drafts[name] = (dependencies, sql)

    ordered: list[str] = []
    remaining = dict(drafts)
    while remaining:
        ready = sorted(
            name
            for name, (dependencies, _) in remaining.items()
            if not dependencies & remaining.keys()
        )
        if not ready:
            raise ValueError(f"draft dependency cycle among {', '.join(sorted(remaining))}")
        for name in ready:
            _, sql = remaining.pop(name)
            ordered.append(sql)
    return ordered


def _discover_tables(root: Path) -> set[str]:
    connection = sqlite3.connect(":memory:")
    try:
        connection.execute(
            "CREATE TABLE schema_migrations (version TEXT PRIMARY KEY, applied_at INTEGER NOT NULL)"
        )
        for index, path in enumerate(_registered_migrations(root)):
            connection.executescript(path.read_text())
            connection.execute(
                "INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (?, ?)",
                (path.stem, index),
            )
        for sql in _ordered_draft_sql(root):
            connection.executescript(sql)
        rows = connection.execute(
            "SELECT name FROM sqlite_master "
            "WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name"
        )
        return {row[0] for row in rows}
    finally:
        connection.close()


def _discover_shims(root: Path) -> set[str]:
    shims: set[str] = set()
    for path in _scan_files(root):
        source = _source_for_scan(path)
        shims.update(f"shim:{name}" for name in SHIM_MARKER.findall(source))
    return shims


def _shim_coverage(
    root: Path,
    mapped: Counter[str],
    errors: list[str],
) -> Coverage:
    discovered = _discover_shims(root)
    declared = {token for token in mapped if token.startswith("shim:")}
    dead = sorted(declared - discovered)
    if dead:
        errors.append(f"compatibility seam has no source marker: {', '.join(dead)}")
    return _cover("compatibility seam", discovered, mapped, errors)


def _projection_coverage(
    root: Path,
    rows: list[dict[str, str]],
    tables: set[str],
    errors: list[str],
) -> Coverage:
    mapped = _tokens(rows, "projection")
    discovered = set(mapped) & tables
    fixtures = root / "tests/fixtures"
    if fixtures.is_dir():
        discovered.update(
            f"{path.relative_to(root).as_posix()}/"
            for path in fixtures.iterdir()
            if path.is_dir() and any(path.glob("*.json"))
        )

    for row in rows:
        sources = CODE_TOKEN.findall(row.get("projection", ""))
        if not any(
            source in tables or (source.endswith("/") and (root / source.rstrip("/")).is_dir())
            for source in sources
        ):
            errors.append(f"read projection has no live source: {row.get('projection', '')}")
    return _cover("read projection", discovered, mapped, errors)


def _scan_files(root: Path) -> list[Path]:
    files: set[Path] = set()
    for configured in SCAN_ROOTS:
        path = root / configured
        if path.is_file():
            files.add(path)
            continue
        if not path.is_dir():
            continue
        for candidate in path.rglob("*"):
            relative = candidate.relative_to(root)
            if not candidate.is_file() or candidate.suffix not in TEXT_SUFFIXES:
                continue
            if IGNORED_PARTS.intersection(relative.parts):
                continue
            if any(prefix == relative or prefix in relative.parents for prefix in IGNORED_PREFIXES):
                continue
            files.add(candidate)
    return sorted(files)


def _source_for_scan(path: Path) -> str:
    source = path.read_text(errors="replace")
    return _production_rust(source) if path.suffix == ".rs" else source


def _scope_matches(relative: str, scope: str) -> bool:
    if scope.endswith("/"):
        return relative.startswith(scope)
    return fnmatch.fnmatch(relative, scope)


def _vocabulary_errors(root: Path, rows: list[dict[str, str]]) -> list[str]:
    errors: list[str] = []
    sources = [
        (path.relative_to(root).as_posix(), _source_for_scan(path))
        for path in _scan_files(root)
        if path.relative_to(root) != ARCHITECTURE
    ]
    for row in rows:
        patterns = CODE_TOKEN.findall(row.get("retired term", ""))
        scopes = CODE_TOKEN.findall(row.get("allowed scopes", ""))
        used_scopes: set[str] = set()
        for relative, source in sources:
            matching_scopes = {
                scope for scope in scopes if _scope_matches(relative, scope)
            }
            for pattern in patterns:
                case_sensitive = pattern.isupper()
                for number, line in enumerate(source.splitlines(), start=1):
                    matched = (
                        pattern in line if case_sensitive else pattern.casefold() in line.casefold()
                    )
                    if matched:
                        if matching_scopes:
                            used_scopes.update(matching_scopes)
                        else:
                            errors.append(f"stale vocabulary {pattern!r} at {relative}:{number}")
        for scope in sorted(set(scopes) - used_scopes):
            errors.append(
                f"unused vocabulary scope {scope!r} for {', '.join(patterns)}"
            )
    return errors


def _concept_errors(rows: list[dict[str, str]]) -> list[str]:
    errors: list[str] = []
    concepts: list[str] = []
    for row in rows:
        cell = row.get("concept", "")
        match = re.match(r"^\*\*([^*]+)\*\*", cell)
        if match is None:
            errors.append(f"architecture concept has no bold name: {cell}")
            continue
        concepts.append(match.group(1).strip())

    duplicates = sorted(name for name, count in Counter(concepts).items() if count > 1)
    if duplicates:
        errors.append(f"concept named more than once: {', '.join(duplicates)}")
    return errors


def _link_errors(root: Path, text: str) -> list[str]:
    errors: list[str] = []
    architecture_dir = (root / ARCHITECTURE).parent
    for label, target in MARKDOWN_LINK.findall(text):
        if "://" in target or target.startswith("#"):
            continue
        path_text = target.split("#", 1)[0]
        path = (architecture_dir / path_text).resolve()
        try:
            path.relative_to(root.resolve())
        except ValueError:
            errors.append(f"architecture link escapes the repository: {target}")
            continue
        if not path.exists():
            errors.append(f"architecture link has no target: {target}")
            continue
        symbol = label.strip("`")
        if path.is_file() and re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", symbol):
            if re.search(rf"\b{re.escape(symbol)}\b", path.read_text(errors="replace")) is None:
                errors.append(f"architecture link labels missing symbol {symbol} in {path_text}")
    return errors


def _wave_locator_errors(root: Path) -> list[str]:
    errors: list[str] = []
    source_root = root / "rust/loopflow/src"
    for path in sorted(source_root.rglob("*.rs")):
        source = _production_rust(path.read_text())
        for number, line in enumerate(source.splitlines(), start=1):
            if "get_wave_by_name(" in line:
                relative = path.relative_to(root)
                errors.append(
                    f"bare Wave lookup get_wave_by_name at {relative}:{number}; "
                    "resolve by WaveId or WaveLocator"
                )
            if "resolve_managed_wave_name" in line:
                relative = path.relative_to(root)
                errors.append(
                    f"name-only Wave resolver at {relative}:{number}; "
                    "resolve the Wave row and derive its display name at the leaf"
                )
    return errors


def _cover(
    name: str,
    discovered: set[str],
    mapped: Counter[str],
    errors: list[str],
) -> Coverage:
    missing = sorted(item for item in discovered if mapped[item] == 0)
    duplicates = sorted(item for item in discovered if mapped[item] > 1)
    if missing:
        errors.append(f"{name} missing from map: {', '.join(missing)}")
    if duplicates:
        errors.append(f"{name} mapped more than once: {', '.join(duplicates)}")
    covered = len(discovered) - len(missing) - len(duplicates)
    return Coverage(name=name, mapped=covered, discovered=len(discovered))


def check_repository(root: Path = REPO_ROOT) -> Report:
    errors: list[str] = []
    coverage: list[Coverage] = []
    architecture_path = root / ARCHITECTURE
    try:
        text = architecture_path.read_text()
        map_section = _section(text, "map")
        map_rows = _table(map_section)
        projections_section = _section(text, "projections")
        projection_rows = _table(projections_section)
        shims_section = _section(text, "shims")
        shim_rows = _table(shims_section)
        vocabulary_rows = _table(_section(text, "vocabulary"))

        errors.extend(_concept_errors(map_rows))

        persistence = _tokens(map_rows, "persistence")
        processes = _tokens(map_rows, "process owner")
        public = _tokens(map_rows, "public surface")
        edges = _tokens(map_rows, "external edge")
        shim_tokens = _tokens(shim_rows, "seam")

        commands, internal_commands = _discover_commands(root)
        command_locations = public + processes + shim_tokens
        coverage.append(
            _cover(
                "public API",
                commands - internal_commands,
                command_locations,
                errors,
            )
        )
        coverage.append(
            _cover(
                "process boundary",
                _discover_binaries(root) | internal_commands,
                processes + _discover_internal_flow_commands(root, internal_commands),
                errors,
            )
        )
        tables = _discover_tables(root)
        coverage.append(_cover("SQLite owner/mirror", tables, persistence, errors))
        coverage.append(_projection_coverage(root, projection_rows, tables, errors))
        coverage.append(_cover("HTTP route", _discover_routes(root), public, errors))
        coverage.append(_cover("provider edge", _discover_providers(root), edges, errors))
        coverage.append(_cover("subprocess edge", _discover_executable_edges(root), edges, errors))
        coverage.append(_shim_coverage(root, shim_tokens, errors))
        errors.extend(_vocabulary_errors(root, vocabulary_rows))
        errors.extend(_link_errors(root, map_section + projections_section + shims_section))
        errors.extend(_wave_locator_errors(root))
    except (OSError, ValueError, IndexError, sqlite3.Error) as error:
        errors.append(str(error))
    return Report(coverage=coverage, errors=errors)


def _print_report(report: Report) -> None:
    print("Architecture map coverage")
    for item in report.coverage:
        status = "PASS" if item.mapped == item.discovered else "FAIL"
        print(f"  {status}  {item.name}: {item.mapped}/{item.discovered}")
    if report.errors:
        print("\nUnexplained architecture drift:")
        for error in report.errors:
            print(f"- {error}")
        return
    print("\nZero unexplained owners, mirrors, shims, or stale vocabulary.")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="emit a versioned JSON report")
    args = parser.parse_args()
    report = check_repository()
    if args.json:
        print(json.dumps(report.to_dict(), indent=2, sort_keys=True))
    else:
        _print_report(report)
    return 0 if report.ok else 1


if __name__ == "__main__":
    sys.exit(main())
