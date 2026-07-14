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
MIGRATIONS_RS = "rust/loopflow/src/store/migrations.rs"


def _git(repo: Path, *args: str) -> None:
    subprocess.run(["git", *args], cwd=repo, check=True, capture_output=True)


def _registry(*entries: tuple[int, int, int, str]) -> str:
    """A MIGRATIONS registry in Rust, shaped as migrations.rs writes it."""
    body = "".join(
        f"""Migration {{
    id: MigrationId {{
        major: {major},
        minor: {minor},
        ordinal: {ordinal},
    }},
    name: "{name}",
    sql: include_str!("migrations/{major}.{minor}.{ordinal:03d}_{name}.sql"),
}}, """
        for major, minor, ordinal, name in entries
    )
    return f"const MIGRATIONS: &[Migration] = &[{body}];\n"


def _register(repo: Path, *entries: tuple[int, int, int, str]) -> None:
    (repo / MIGRATIONS_RS).write_text(_registry(*entries))


@pytest.fixture
def repo(tmp_path: Path) -> Path:
    """A repo tagged v0.10.1 with one shipped migration, `0.10.001_initial.sql`."""
    (tmp_path / "scripts").mkdir()
    (tmp_path / "scripts/check_migrations.py").write_bytes(SCRIPT.read_bytes())
    (tmp_path / MIGRATIONS).mkdir(parents=True)
    (tmp_path / "Cargo.toml").write_text('[workspace.package]\nversion = "0.10.1"\n')
    (tmp_path / "pyproject.toml").write_text('[project]\nversion = "0.10.1"\n')
    (tmp_path / MIGRATIONS / "0.10.001_initial.sql").write_text("CREATE TABLE waves (id TEXT);\n")
    _register(tmp_path, (0, 10, 1, "initial"))

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
    _register(repo, (0, 10, 1, "initial"), (0, 10, 2, "add_note"))

    result = check(repo)
    assert result.returncode == 0, result.stderr


def test_a_migration_file_nobody_registered_fails(repo: Path):
    """An unregistered migration never runs — shipping one is shipping a no-op."""
    (repo / MIGRATIONS / "0.10.002_add_note.sql").write_text("ALTER TABLE waves ADD note TEXT;\n")

    result = check(repo)
    assert result.returncode == 1
    assert "not in the MIGRATIONS registry" in result.stderr


def test_a_registry_entry_without_its_file_fails(repo: Path):
    _register(repo, (0, 10, 1, "initial"), (0, 10, 2, "add_note"))

    result = check(repo)
    assert result.returncode == 1
    assert "has no file" in result.stderr


def test_duplicate_registry_ids_fail(repo: Path):
    _register(repo, (0, 10, 1, "initial"), (0, 10, 1, "initial"))

    result = check(repo)
    assert result.returncode == 1
    assert "collides" in result.stderr


def test_a_registry_entry_whose_id_disagrees_with_its_file_fails(repo: Path):
    (repo / MIGRATIONS_RS).write_text(
        _registry((0, 10, 2, "initial")).replace(
            "migrations/0.10.002_initial.sql", "migrations/0.10.001_initial.sql"
        )
    )

    result = check(repo)
    assert result.returncode == 1
    assert "must agree" in result.stderr


def test_a_registry_entry_whose_name_disagrees_with_its_file_fails(repo: Path):
    (repo / MIGRATIONS_RS).write_text(
        _registry((0, 10, 1, "setup")).replace(
            "migrations/0.10.001_setup.sql", "migrations/0.10.001_initial.sql"
        )
    )

    result = check(repo)
    assert result.returncode == 1
    assert "must agree" in result.stderr


def test_a_registry_out_of_id_order_fails(repo: Path):
    (repo / MIGRATIONS / "0.10.002_add_note.sql").write_text("ALTER TABLE waves ADD note TEXT;\n")
    _register(repo, (0, 10, 2, "add_note"), (0, 10, 1, "initial"))

    result = check(repo)
    assert result.returncode == 1
    assert "not in id order" in result.stderr


def test_editing_a_shipped_migration_fails(repo: Path):
    (repo / MIGRATIONS / "0.10.001_initial.sql").write_text("CREATE TABLE waves (id INTEGER);\n")

    result = check(repo)
    assert result.returncode == 1
    assert "has been edited" in result.stderr


def test_renaming_a_shipped_migration_fails(repo: Path):
    """Even a rename the registry agrees with: the id is what shipped."""
    (repo / MIGRATIONS / "0.10.001_initial.sql").rename(repo / MIGRATIONS / "0.10.001_setup.sql")
    _register(repo, (0, 10, 1, "setup"))

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
