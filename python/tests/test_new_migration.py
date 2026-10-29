"""A Task authors one migration file and is pointed back at it afterwards."""

import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/new_migration.py"
MIGRATIONS = Path("rust/loopflow/src/store/migrations")


@pytest.fixture
def repo(tmp_path: Path) -> Path:
    """A git repo on main with one released batch and one merged slug."""
    (tmp_path / "scripts").mkdir()
    (tmp_path / "scripts/new_migration.py").write_bytes(SCRIPT.read_bytes())
    (tmp_path / MIGRATIONS).mkdir(parents=True)
    (tmp_path / MIGRATIONS / "0.11.1.001_release.sql").write_text(
        "-- draft: initial\nCREATE TABLE waves (id TEXT);\n"
    )
    (tmp_path / MIGRATIONS / "task_graphs.sql").write_text("CREATE TABLE graphs (id TEXT);\n")
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


def test_new_migration_writes_one_empty_file_named_by_its_slug(repo: Path) -> None:
    result = run(repo, "add_wave_colour")

    assert result.returncode == 0, result.stderr
    assert (repo / MIGRATIONS / "add_wave_colour.sql").read_text() == ""


def test_a_task_with_a_migration_is_pointed_back_at_it(repo: Path) -> None:
    git(repo, "checkout", "-q", "-b", "task")
    assert run(repo, "add_wave_colour").returncode == 0
    commit(repo, "migration")

    result = run(repo, "another_one")

    assert result.returncode == 0
    assert "already has" in result.stdout
    assert "add_wave_colour.sql" in result.stdout
    assert not (repo / MIGRATIONS / "another_one.sql").exists()


@pytest.mark.parametrize("name", ["initial", "task_graphs"])
def test_a_name_already_in_use_is_refused(repo: Path, name: str) -> None:
    result = run(repo, name)

    assert result.returncode == 1
    assert "already a migration name" in result.stderr


def test_a_name_must_be_snake_case(repo: Path) -> None:
    assert run(repo, "Add-Colour").returncode == 2
