"""A Task authors one draft file and is pointed back at it afterwards."""

import os
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/new_migration.py"
MIGRATIONS = Path("rust/loopflow/src/store/migrations")
DRAFTS = MIGRATIONS / "drafts"
MIGRATIONS_RS = Path("rust/loopflow/src/store/migration_catalog.rs")

REGISTRY = """const MIGRATIONS: &[Migration] = &[
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 1,
        },
        name: "initial",
        sql: include_str!("migrations/0.11.001_initial.sql"),
    },
];
"""


@pytest.fixture
def repo(tmp_path: Path) -> Path:
    """A git repo on main with one released migration, a registry, and no drafts."""
    (tmp_path / "scripts").mkdir()
    (tmp_path / "scripts/new_migration.py").write_bytes(SCRIPT.read_bytes())
    (tmp_path / MIGRATIONS).mkdir(parents=True)
    (tmp_path / MIGRATIONS / "0.11.001_initial.sql").write_text("CREATE TABLE waves (id TEXT);\n")
    (tmp_path / MIGRATIONS_RS).write_text(REGISTRY)
    git(tmp_path, "init", "-q", "-b", "main")
    commit(tmp_path, "base")
    return tmp_path


def git(repo: Path, *args: str) -> None:
    subprocess.run(
        ["git", "-c", "user.name=t", "-c", "user.email=t@example.com", *args],
        cwd=repo,
        check=True,
        capture_output=True,
    )


def commit(repo: Path, message: str) -> None:
    git(repo, "add", "-A")
    git(repo, "commit", "-q", "-m", message)


def run(repo: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, "scripts/new_migration.py", *args],
        cwd=repo,
        capture_output=True,
        text=True,
    )


def draft_files(repo: Path) -> list[str]:
    return sorted(p.name for p in (repo / DRAFTS).glob("*.sql"))


def test_new_migration_writes_one_empty_draft_named_by_its_file(repo: Path) -> None:
    registry_before = (repo / MIGRATIONS_RS).read_text()

    result = run(repo, "add_wave_colour")

    assert result.returncode == 0, result.stderr
    assert draft_files(repo) == ["add_wave_colour.sql"]
    assert (repo / DRAFTS / "add_wave_colour.sql").read_text() == ""
    assert (repo / MIGRATIONS_RS).read_text() == registry_before


def test_a_second_request_points_at_the_tasks_existing_draft(repo: Path) -> None:
    git(repo, "checkout", "-q", "-b", "task")
    assert run(repo, "add_wave_colour").returncode == 0
    (repo / DRAFTS / "add_wave_colour.sql").write_text(
        "ALTER TABLE waves ADD COLUMN colour TEXT;\n"
    )
    commit(repo, "draft")

    result = run(repo, "drop_wave_colour")

    assert result.returncode == 0, result.stderr
    assert "add_wave_colour.sql" in result.stdout
    assert draft_files(repo) == ["add_wave_colour.sql"]
    assert "colour TEXT" in (repo / DRAFTS / "add_wave_colour.sql").read_text()


def test_a_task_gets_its_own_draft_beside_drafts_already_on_main(repo: Path) -> None:
    assert run(repo, "add_wave_colour").returncode == 0
    commit(repo, "another Task's draft lands")
    git(repo, "checkout", "-q", "-b", "task")

    result = run(repo, "backfill_colour", "--depends-on", "add_wave_colour")

    assert result.returncode == 0, result.stderr
    assert draft_files(repo) == ["add_wave_colour.sql", "backfill_colour.sql"]
    assert (repo / DRAFTS / "backfill_colour.sql").read_text() == "-- depends_on: add_wave_colour\n"


def test_new_migration_works_outside_git(tmp_path: Path) -> None:
    (tmp_path / "scripts").mkdir()
    (tmp_path / "scripts/new_migration.py").write_bytes(SCRIPT.read_bytes())
    (tmp_path / MIGRATIONS).mkdir(parents=True)

    result = subprocess.run(
        [sys.executable, "scripts/new_migration.py", "add_task_priority"],
        cwd=tmp_path,
        capture_output=True,
        text=True,
        env={"GIT_CEILING_DIRECTORIES": str(tmp_path.parent), "PATH": os.environ["PATH"]},
    )

    assert result.returncode == 0, result.stderr
    assert draft_files(tmp_path) == ["add_task_priority.sql"]


def test_depends_on_accepts_a_released_migration(repo: Path) -> None:
    result = run(repo, "backfill_colour", "--depends-on", "initial")
    assert result.returncode == 0, result.stderr


def test_depends_on_accepts_a_draft_published_inside_a_release_batch(repo: Path) -> None:
    (repo / MIGRATIONS / "0.11.2.001_release.sql").write_text(
        "-- draft: add_wave_colour\nSELECT 1;\n"
    )

    result = run(repo, "backfill_colour", "--depends-on", "add_wave_colour")

    assert result.returncode == 0, result.stderr


def test_depends_on_rejects_an_unknown_name(repo: Path) -> None:
    result = run(repo, "backfill_colour", "--depends-on", "nope")
    assert result.returncode == 1
    assert "no draft or released migration" in result.stderr


def test_new_migration_rejects_a_name_colliding_with_a_released_migration(repo: Path) -> None:
    result = run(repo, "initial")
    assert result.returncode == 1
    assert "already a migration name" in result.stderr
