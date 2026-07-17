"""The release cut: drafts become a canonical, ordered, ordinal-assigned tail."""

import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/canonicalize_migrations.py"
MIGRATIONS = Path("rust/loopflow/src/store/migrations")
DRAFTS = MIGRATIONS / "drafts"
MIGRATIONS_RS = Path("rust/loopflow/src/store/migrations.rs")

REGISTRY = """const MIGRATIONS: &[Migration] = &[
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 1,
        },
        name: "initial",
        sql: include_str!("migrations/0.11.001_initial.sql"),
    },
];
"""


@pytest.fixture
def repo(tmp_path: Path) -> Path:
    (tmp_path / "scripts").mkdir()
    (tmp_path / "scripts/canonicalize_migrations.py").write_bytes(SCRIPT.read_bytes())
    (tmp_path / MIGRATIONS).mkdir(parents=True)
    (tmp_path / MIGRATIONS / "0.11.001_initial.sql").write_text("CREATE TABLE waves (id TEXT);\n")
    (tmp_path / MIGRATIONS_RS).write_text(REGISTRY)
    (tmp_path / DRAFTS).mkdir()
    return tmp_path


def draft(repo: Path, name: str, depends_on: str = "", body: str = "SELECT 1;\n") -> None:
    (repo / DRAFTS / f"{name}.sql").write_text(f"-- name: {name}\n-- depends_on: {depends_on}\n{body}")


def run(repo: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, "scripts/canonicalize_migrations.py", *args],
        cwd=repo,
        capture_output=True,
        text=True,
    )


def canonical_files(repo: Path) -> list[str]:
    return sorted(p.name for p in (repo / MIGRATIONS).glob("*.sql"))


def test_empty_draft_set_is_a_noop(repo: Path) -> None:
    before = (repo / MIGRATIONS_RS).read_text()
    result = run(repo, "0.11.30")
    assert result.returncode == 0, result.stderr
    assert "no drafts" in result.stdout
    assert (repo / MIGRATIONS_RS).read_text() == before


def test_two_independent_drafts_get_a_contiguous_tail_ordered_by_name(repo: Path) -> None:
    draft(repo, "add_wave_colour")
    draft(repo, "add_task_priority")

    result = run(repo, "0.11.30")

    assert result.returncode == 0, result.stderr
    # Contiguous after the last released ordinal (001), ordered by name.
    assert canonical_files(repo) == [
        "0.11.001_initial.sql",
        "0.11.002_add_task_priority.sql",
        "0.11.003_add_wave_colour.sql",
    ]
    # Drafts are consumed, leaving none behind.
    assert list((repo / DRAFTS).glob("*.sql")) == []
    registry = (repo / MIGRATIONS_RS).read_text()
    assert 'name: "add_task_priority"' in registry
    assert 'name: "add_wave_colour"' in registry
    assert registry.rstrip().endswith("];")


def test_a_dependency_orders_before_its_dependent_against_name_order(repo: Path) -> None:
    # By name, `a_backfill` sorts before `z_setup`; the dependency inverts that.
    draft(repo, "z_setup")
    draft(repo, "a_backfill", depends_on="z_setup")

    result = run(repo, "0.11.30")

    assert result.returncode == 0, result.stderr
    assert canonical_files(repo) == [
        "0.11.001_initial.sql",
        "0.11.002_z_setup.sql",
        "0.11.003_a_backfill.sql",
    ]


def test_the_canonical_file_carries_the_body_without_the_header(repo: Path) -> None:
    draft(repo, "add_wave_colour", body="ALTER TABLE waves ADD COLUMN colour TEXT;\n")

    run(repo, "0.11.30")

    written = (repo / MIGRATIONS / "0.11.002_add_wave_colour.sql").read_text()
    assert written == "ALTER TABLE waves ADD COLUMN colour TEXT;\n"
    assert "-- name:" not in written


def test_a_minor_bump_starts_a_fresh_ordinal_sequence(repo: Path) -> None:
    draft(repo, "add_wave_colour")

    result = run(repo, "0.12.0")

    assert result.returncode == 0, result.stderr
    assert (repo / MIGRATIONS / "0.12.001_add_wave_colour.sql").exists()


def test_a_cycle_fails_before_writing_anything(repo: Path) -> None:
    draft(repo, "a", depends_on="b")
    draft(repo, "b", depends_on="a")
    before = (repo / MIGRATIONS_RS).read_text()

    result = run(repo, "0.11.30")

    assert result.returncode == 1
    assert "cycle" in result.stderr
    assert (repo / MIGRATIONS_RS).read_text() == before
    assert canonical_files(repo) == ["0.11.001_initial.sql"]
    assert {p.name for p in (repo / DRAFTS).glob("*.sql")} == {"a.sql", "b.sql"}


def test_check_mode_writes_nothing(repo: Path) -> None:
    draft(repo, "add_wave_colour")
    before = (repo / MIGRATIONS_RS).read_text()

    result = run(repo, "0.11.30", "--check")

    assert result.returncode == 0, result.stderr
    assert "0.11.002_add_wave_colour" in result.stdout
    assert (repo / MIGRATIONS_RS).read_text() == before
    assert (repo / DRAFTS / "add_wave_colour.sql").exists()


def test_re_running_the_same_release_is_deterministic(tmp_path: Path) -> None:
    def build(name: str) -> Path:
        repo = tmp_path / name
        (repo / "scripts").mkdir(parents=True)
        (repo / "scripts/canonicalize_migrations.py").write_bytes(SCRIPT.read_bytes())
        (repo / MIGRATIONS).mkdir(parents=True)
        (repo / MIGRATIONS / "0.11.001_initial.sql").write_text("CREATE TABLE waves (id TEXT);\n")
        (repo / MIGRATIONS_RS).write_text(REGISTRY)
        (repo / DRAFTS).mkdir()
        draft(repo, "add_wave_colour")
        draft(repo, "add_task_priority")
        return repo

    first, second = build("first"), build("second")
    assert run(first, "0.11.30").returncode == 0
    assert run(second, "0.11.30").returncode == 0

    assert canonical_files(first) == canonical_files(second)
    assert (first / MIGRATIONS_RS).read_text() == (second / MIGRATIONS_RS).read_text()


def test_an_abandoned_release_leaves_drafts_regenerable(repo: Path) -> None:
    draft(repo, "add_wave_colour")
    # A failed release never merged its ids; --check proves the plan without
    # publishing, and a later real run regenerates the same id.
    assert "0.11.002_add_wave_colour" in run(repo, "0.11.30", "--check").stdout
    run(repo, "0.11.30")
    assert (repo / MIGRATIONS / "0.11.002_add_wave_colour.sql").exists()
