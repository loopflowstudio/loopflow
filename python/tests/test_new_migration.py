import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/new_migration.py"
MIGRATIONS = Path("rust/loopflow/src/store/migrations")
DRAFTS = MIGRATIONS / "drafts"


@pytest.fixture
def repo(tmp_path: Path) -> Path:
    """A repo with one released migration and no drafts."""
    (tmp_path / "scripts").mkdir()
    (tmp_path / "scripts/new_migration.py").write_bytes(SCRIPT.read_bytes())
    (tmp_path / MIGRATIONS).mkdir(parents=True)
    (tmp_path / MIGRATIONS / "0.11.001_initial.sql").write_text("CREATE TABLE waves (id TEXT);\n")
    return tmp_path


def run(repo: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, "scripts/new_migration.py", *args],
        cwd=repo,
        capture_output=True,
        text=True,
    )


def test_new_migration_writes_a_draft_with_no_ordinal(repo: Path) -> None:
    result = run(repo, "add_wave_colour")

    assert result.returncode == 0, result.stderr
    draft = repo / DRAFTS / "add_wave_colour.sql"
    assert draft.exists()
    assert draft.read_text().startswith("-- name: add_wave_colour\n-- depends_on: \n")
    # No canonical ordinal was allocated, and nothing under the release namespace moved.
    assert list((repo / MIGRATIONS).glob("0.11.*.sql")) == [repo / MIGRATIONS / "0.11.001_initial.sql"]
    assert "DraftMigration" in result.stdout


def test_new_migration_never_fetches_or_touches_git(repo: Path) -> None:
    # A bare directory, no git repo at all: the allocator used to fetch origin/main,
    # which would fail here. A draft carries no ordinal, so nothing is counted.
    assert not (repo / ".git").exists()
    result = run(repo, "add_task_priority")
    assert result.returncode == 0, result.stderr
    assert (repo / DRAFTS / "add_task_priority.sql").exists()


def test_new_migration_records_declared_dependencies(repo: Path) -> None:
    run(repo, "add_wave_colour")
    result = run(repo, "backfill_colour", "--depends-on", "add_wave_colour")

    assert result.returncode == 0, result.stderr
    draft = repo / DRAFTS / "backfill_colour.sql"
    assert draft.read_text().startswith("-- name: backfill_colour\n-- depends_on: add_wave_colour\n")


def test_new_migration_rejects_a_dependency_on_no_draft(repo: Path) -> None:
    result = run(repo, "backfill_colour", "--depends-on", "add_wave_colour")
    assert result.returncode == 1
    assert "names no existing draft" in result.stderr


def test_new_migration_rejects_a_name_colliding_with_a_released_migration(repo: Path) -> None:
    result = run(repo, "initial")
    assert result.returncode == 1
    assert "already a released migration name" in result.stderr


def test_new_migration_rejects_a_duplicate_draft(repo: Path) -> None:
    run(repo, "add_wave_colour")
    result = run(repo, "add_wave_colour")
    assert result.returncode == 1
    assert "already exists" in result.stderr
