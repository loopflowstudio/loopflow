"""The merge gate: a migration on origin/main can never change."""

import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/check_migrations.py"
MIGRATIONS = Path("rust/loopflow/src/store/migrations")


def _git(repo: Path, *args: str) -> None:
    subprocess.run(
        ["git", "-c", "user.name=t", "-c", "user.email=t@example.com", *args],
        cwd=repo,
        check=True,
        capture_output=True,
    )


@pytest.fixture
def repo(tmp_path: Path) -> Path:
    """origin/main carries one released batch and one merged slug."""
    (tmp_path / "scripts").mkdir()
    (tmp_path / "scripts/check_migrations.py").write_bytes(SCRIPT.read_bytes())
    (tmp_path / MIGRATIONS).mkdir(parents=True)
    (tmp_path / MIGRATIONS / "0.10.1.001_release.sql").write_text(
        "-- draft: initial\nCREATE TABLE waves (id TEXT);\n"
    )
    (tmp_path / MIGRATIONS / "task_graphs.sql").write_text("CREATE TABLE graphs (id TEXT);\n")
    _git(tmp_path, "init", "-q", "-b", "main")
    _git(tmp_path, "add", ".")
    _git(tmp_path, "commit", "-qm", "main")
    _git(tmp_path, "update-ref", "refs/remotes/origin/main", "HEAD")
    return tmp_path


def check(repo: Path) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, "scripts/check_migrations.py"],
        cwd=repo,
        capture_output=True,
        text=True,
    )


def test_merged_history_left_alone_passes(repo: Path):
    result = check(repo)
    assert result.returncode == 0, result.stderr
    assert "no unmerged migration" in result.stdout


def test_new_slugs_pass_and_are_named(repo: Path):
    (repo / MIGRATIONS / "add_note.sql").write_text("ALTER TABLE waves ADD note TEXT;\n")
    (repo / MIGRATIONS / "add_colour.sql").write_text("ALTER TABLE waves ADD colour TEXT;\n")
    result = check(repo)
    assert result.returncode == 0, result.stderr
    assert "unmerged: add_colour.sql, add_note.sql" in result.stdout


@pytest.mark.parametrize("name", ["task_graphs.sql", "0.10.1.001_release.sql"])
def test_an_edited_merged_migration_fails(repo: Path, name: str):
    (repo / MIGRATIONS / name).write_text("SELECT 1;\n")
    result = check(repo)
    assert result.returncode == 1
    assert "differs from origin/main" in result.stderr


def test_a_deleted_merged_migration_fails(repo: Path):
    (repo / MIGRATIONS / "task_graphs.sql").unlink()
    result = check(repo)
    assert result.returncode == 1
    assert "never deleted" in result.stderr


def test_a_new_released_batch_fails(repo: Path):
    (repo / MIGRATIONS / "0.10.2.001_release.sql").write_text("SELECT 1;\n")
    result = check(repo)
    assert result.returncode == 1
    assert "adds a released batch" in result.stderr


@pytest.mark.parametrize("name", ["Add-Note.sql", "drafts"])
def test_misplaced_files_fail(repo: Path, name: str):
    path = repo / MIGRATIONS / name
    if name == "drafts":
        path.mkdir()
    else:
        path.write_text("SELECT 1;\n")
    assert check(repo).returncode == 1


def test_a_slug_reusing_a_released_name_fails(repo: Path):
    (repo / MIGRATIONS / "initial.sql").write_text("SELECT 1;\n")
    result = check(repo)
    assert result.returncode == 1
    assert "reuses the released migration name" in result.stderr


def test_runtime_sql_names_its_columns(repo: Path):
    (repo / "rust/loopflow/src/writer.rs").write_text(
        'const SQL: &str = "INSERT INTO waves VALUES (?1)";\n'
    )
    result = check(repo)
    assert result.returncode == 1
    assert "positional INSERT" in result.stderr
