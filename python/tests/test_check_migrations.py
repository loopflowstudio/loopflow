"""The release gate: a migration that already shipped can never change.

Each test builds a throwaway repo with a release tag, so the check runs against a
real tag rather than the one this repo happens to be on.
"""

import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/check_migrations.py"
MIGRATIONS = "rust/loopflow/src/store/migrations"


def _git(repo: Path, *args: str) -> None:
    subprocess.run(["git", *args], cwd=repo, check=True, capture_output=True)


@pytest.fixture
def repo(tmp_path: Path) -> Path:
    """A repo tagged v0.10.1 with one shipped migration, `0.10.001_initial.sql`."""
    (tmp_path / "scripts").mkdir()
    (tmp_path / "scripts/check_migrations.py").write_bytes(SCRIPT.read_bytes())
    (tmp_path / MIGRATIONS).mkdir(parents=True)
    (tmp_path / "Cargo.toml").write_text('[workspace.package]\nversion = "0.10.1"\n')
    (tmp_path / "pyproject.toml").write_text('[project]\nversion = "0.10.1"\n')
    (tmp_path / MIGRATIONS / "0.10.001_initial.sql").write_text("CREATE TABLE waves (id TEXT);\n")

    _git(tmp_path, "init", "-q")
    _git(tmp_path, "config", "user.email", "test@example.com")
    _git(tmp_path, "config", "user.name", "test")
    _git(tmp_path, "add", ".")
    _git(tmp_path, "commit", "-qm", "release")
    _git(tmp_path, "tag", "v0.10.1")
    return tmp_path


def check(repo: Path) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, "scripts/check_migrations.py"],
        cwd=repo,
        capture_output=True,
        text=True,
    )


def test_a_shipped_migration_left_alone_passes(repo: Path):
    result = check(repo)
    assert result.returncode == 0, result.stderr
    assert "unchanged since v0.10.1" in result.stdout


def test_appending_a_migration_to_the_active_namespace_passes(repo: Path):
    (repo / MIGRATIONS / "0.10.002_add_note.sql").write_text("ALTER TABLE waves ADD note TEXT;\n")

    result = check(repo)
    assert result.returncode == 0, result.stderr


def test_editing_a_shipped_migration_fails(repo: Path):
    (repo / MIGRATIONS / "0.10.001_initial.sql").write_text("CREATE TABLE waves (id INTEGER);\n")

    result = check(repo)
    assert result.returncode == 1
    assert "has been edited" in result.stderr


def test_renaming_a_shipped_migration_fails(repo: Path):
    (repo / MIGRATIONS / "0.10.001_initial.sql").rename(repo / MIGRATIONS / "0.10.001_setup.sql")

    result = check(repo)
    assert result.returncode == 1
    assert "is now missing" in result.stderr


def test_moving_the_migration_directory_does_not_void_the_check(repo: Path):
    """Relocating the directory cannot quietly retire the shipped ids inside it."""
    (repo / MIGRATIONS).rename(repo / "rust/loopflow/src/store/elsewhere")

    result = check(repo)
    assert result.returncode == 1
    assert "missing" in result.stderr


def test_a_migration_ahead_of_the_package_version_fails(repo: Path):
    (repo / MIGRATIONS / "0.11.001_too_new.sql").write_text("SELECT 1;\n")

    result = check(repo)
    assert result.returncode == 1
    assert "namespaced ahead" in result.stderr


def test_a_malformed_migration_name_fails(repo: Path):
    (repo / MIGRATIONS / "002_oops.sql").write_text("SELECT 1;\n")

    result = check(repo)
    assert result.returncode == 1
    assert "is not" in result.stderr


def test_manifests_disagreeing_on_the_version_fails(repo: Path):
    (repo / "pyproject.toml").write_text('[project]\nversion = "0.11.0"\n')

    result = check(repo)
    assert result.returncode == 1
    assert "disagree" in result.stderr
