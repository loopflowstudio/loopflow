"""The architecture map rejects unexplained durable and public boundaries."""

import importlib.util
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/check_architecture.py"
SPEC = importlib.util.spec_from_file_location("architecture_check", SCRIPT)
assert SPEC and SPEC.loader
architecture = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = architecture
SPEC.loader.exec_module(architecture)


def _write(root: Path, path: str, text: str) -> None:
    target = root / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(text)


@pytest.fixture
def repo(tmp_path: Path) -> Path:
    _write(
        tmp_path,
        "docs/architecture.md",
        (
            """# Architecture

<!-- architecture-map:start -->
"""
            "| Concept | Truth and authority | Data structure | Persistence | "
            "Process owner | Public surface | External edge |\n"
            "| --- | --- | --- | --- | --- | --- | --- |\n"
            "| **Wave** | Goal | [`Wave`](../rust/loopflow/src/wave/types.rs) | "
            "`schema_migrations`, `waves` | `lf` | `lf wave`, `wave GET /health` | "
            "`provider:linear`, `exec:git` |\n"
            """<!-- architecture-map:end -->

<!-- architecture-projections:start -->
| Projection | Authority copied | Freshness and consumer |
| --- | --- | --- |
<!-- architecture-projections:end -->

<!-- architecture-shims:start -->
| Seam | Current concept | Source and removal boundary |
| --- | --- | --- |
<!-- architecture-shims:end -->

<!-- architecture-vocabulary:start -->
| Retired term | Allowed scopes | Current language |
| --- | --- | --- |
| `Project Session` | — | Project Work |
<!-- architecture-vocabulary:end -->
"""
        ),
    )
    _write(tmp_path, "rust/loopflow/src/wave/types.rs", "pub struct Wave;\n")
    _write(
        tmp_path,
        "rust/loopflow/src/lf/mod.rs",
        """pub enum Commands {
    Wave,
}
""",
    )
    _write(
        tmp_path,
        "rust/loopflow/Cargo.toml",
        """[[bin]]
name = "lf"
path = "src/bin/lf.rs"
""",
    )
    _write(
        tmp_path,
        "rust/loopflow/src/wave/server.rs",
        'fn router() { Router::new().route("/health", get(health)); }\n',
    )
    _write(tmp_path, "rust/loopflow/src/lfd/mod.rs", "fn router() {}\n")
    _write(
        tmp_path,
        "rust/loopflow/src/provider_auth/mod.rs",
        """pub enum Provider { Linear }
impl Provider {
    pub fn as_str(self) -> &'static str {
        match self { Self::Linear => "linear" }
    }
}
""",
    )
    _write(
        tmp_path,
        "rust/loopflow/src/edge.rs",
        'fn git() { let _ = std::process::Command::new("git"); }\n',
    )
    _write(
        tmp_path,
        "rust/loopflow/src/store/migrations.rs",
        """const MIGRATIONS: &[Migration] = &[
    Migration {
        sql: include_str!("migrations/0.1.001_initial.sql"),
    },
];
""",
    )
    _write(
        tmp_path,
        "rust/loopflow/src/store/migrations/0.1.001_initial.sql",
        "CREATE TABLE waves (id TEXT PRIMARY KEY);\n",
    )
    return tmp_path


def _errors(repo: Path) -> str:
    return "\n".join(architecture.check_repository(repo).errors)


def test_clean_fixture_has_no_unexplained_architecture(repo: Path) -> None:
    report = architecture.check_repository(repo)

    assert report.ok, "\n".join(report.errors)


def test_current_repository_has_no_unexplained_architecture() -> None:
    report = architecture.check_repository(ROOT)

    assert report.ok, "\n".join(report.errors)


def test_new_durable_table_must_join_the_map(repo: Path) -> None:
    _write(
        repo,
        "rust/loopflow/src/store/migrations/drafts/add_mirror.sql",
        """-- name: add_mirror
-- id: draft_add_mirror
-- depends_on: none
CREATE TABLE mirrors (id TEXT);
""",
    )

    assert "SQLite owner/mirror missing from map: mirrors" in _errors(repo)


def test_new_root_command_must_join_a_public_concept(repo: Path) -> None:
    commands = repo / "rust/loopflow/src/lf/mod.rs"
    commands.write_text(commands.read_text().replace("    Wave,", "    Wave,\n    Task,"))

    assert "public API missing from map: lf task" in _errors(repo)


def test_new_binary_must_join_a_process_boundary(repo: Path) -> None:
    manifest = repo / "rust/loopflow/Cargo.toml"
    manifest.write_text(
        manifest.read_text()
        + """
[[bin]]
name = "keeper"
path = "src/bin/keeper.rs"
"""
    )

    assert "process boundary missing from map: keeper" in _errors(repo)


def test_new_http_route_must_name_its_process_owner(repo: Path) -> None:
    server = repo / "rust/loopflow/src/wave/server.rs"
    server.write_text(
        'fn router() { Router::new().route("/health", get(health))'
        '.route("/events", get(events)); }\n'
    )

    assert "HTTP route missing from map: wave GET /events" in _errors(repo)


def test_new_provider_must_join_an_external_edge(repo: Path) -> None:
    providers = repo / "rust/loopflow/src/provider_auth/mod.rs"
    providers.write_text(
        providers.read_text()
        .replace("Provider { Linear }", "Provider { Linear, Codex }")
        .replace(
            'Self::Linear => "linear"',
            'Self::Linear => "linear", Self::Codex => "codex"',
        )
    )

    assert "provider edge missing from map: provider:codex" in _errors(repo)


def test_one_owner_cannot_be_mapped_twice(repo: Path) -> None:
    architecture_doc = repo / "docs/architecture.md"
    architecture_doc.write_text(
        architecture_doc.read_text().replace(
            "<!-- architecture-map:end -->",
            "| **Other** | Goal | — | `waves` | — | — | — |\n<!-- architecture-map:end -->",
        )
    )

    assert "SQLite owner/mirror mapped more than once: waves" in _errors(repo)


def test_one_concept_name_cannot_describe_two_rows(repo: Path) -> None:
    architecture_doc = repo / "docs/architecture.md"
    architecture_doc.write_text(
        architecture_doc.read_text().replace(
            "<!-- architecture-map:end -->",
            "| **Wave** — alternate definition | — | — | — | — | — | — |\n"
            "<!-- architecture-map:end -->",
        )
    )

    assert "concept named more than once: Wave" in _errors(repo)


def test_retired_runtime_language_fails_outside_named_history(repo: Path) -> None:
    _write(repo, "README.md", "Start a project session.\n")

    assert "stale vocabulary 'Project Session' at README.md:1" in _errors(repo)


def test_retired_runtime_language_fails_in_website_source(repo: Path) -> None:
    _write(repo, "website/main.py", 'MODEL = "Project Session"\n')

    assert "stale vocabulary 'Project Session' at website/main.py:1" in _errors(repo)


def test_generated_website_docs_do_not_duplicate_the_authoritative_scan(repo: Path) -> None:
    _write(repo, "website/docs/architecture.md", "Project Session\n")

    assert architecture.check_repository(repo).ok


def test_named_historical_sql_scope_accepts_retired_language(repo: Path) -> None:
    architecture_doc = repo / "docs/architecture.md"
    architecture_doc.write_text(
        architecture_doc.read_text().replace(
            "| `Project Session` | — | Project Work |",
            "| `Project Session` | `rust/loopflow/src/store/migrations/` | Project Work |",
        )
    )
    migration = repo / "rust/loopflow/src/store/migrations/0.1.001_initial.sql"
    migration.write_text(migration.read_text() + "-- Project Session history\n")

    assert architecture.check_repository(repo).ok


def test_unused_historical_scope_fails(repo: Path) -> None:
    architecture_doc = repo / "docs/architecture.md"
    architecture_doc.write_text(
        architecture_doc.read_text().replace(
            "| `Project Session` | — | Project Work |",
            "| `Project Session` | `release/` | Project Work |",
        )
    )

    assert "unused vocabulary scope 'release/' for Project Session" in _errors(repo)


def test_new_compatibility_marker_must_join_the_seam_inventory(repo: Path) -> None:
    _write(repo, "scripts/bridge.py", "# architecture-shim: surprise\n")

    assert "compatibility seam missing from map: shim:surprise" in _errors(repo)


def test_declared_compatibility_seam_needs_a_source_marker(repo: Path) -> None:
    architecture_doc = repo / "docs/architecture.md"
    architecture_doc.write_text(
        architecture_doc.read_text().replace(
            "<!-- architecture-shims:end -->",
            "| `shim:ghost` | Current model | Remove when history expires. |\n"
            "<!-- architecture-shims:end -->",
        )
    )

    assert "compatibility seam has no source marker: shim:ghost" in _errors(repo)


def test_new_dto_projection_must_join_the_projection_inventory(repo: Path) -> None:
    _write(repo, "tests/fixtures/dto/example.json", "{}\n")

    assert "read projection missing from map: tests/fixtures/dto/" in _errors(repo)


def test_dead_authoritative_link_fails(repo: Path) -> None:
    architecture_doc = repo / "docs/architecture.md"
    architecture_doc.write_text(
        architecture_doc.read_text().replace("wave/types.rs", "wave/missing.rs")
    )

    assert "architecture link has no target" in _errors(repo)
