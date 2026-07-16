import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/new_migration.py"
MIGRATIONS = Path("rust/loopflow/src/store/migrations")


def _git(repo: Path, *args: str) -> None:
    subprocess.run(["git", *args], cwd=repo, check=True, capture_output=True)


def test_allocator_counts_migrations_already_on_origin_main(tmp_path: Path) -> None:
    (tmp_path / "scripts").mkdir()
    (tmp_path / "scripts/new_migration.py").write_bytes(SCRIPT.read_bytes())
    (tmp_path / MIGRATIONS).mkdir(parents=True)
    (tmp_path / "Cargo.toml").write_text('[workspace.package]\nversion = "0.11.1"\n')
    (tmp_path / MIGRATIONS / "0.11.001_initial.sql").write_text("SELECT 1;\n")
    _git(tmp_path, "init", "-q")
    _git(tmp_path, "config", "user.email", "test@example.com")
    _git(tmp_path, "config", "user.name", "test")
    _git(tmp_path, "add", ".")
    _git(tmp_path, "commit", "-qm", "base")

    (tmp_path / MIGRATIONS / "0.11.002_main.sql").write_text("SELECT 2;\n")
    _git(tmp_path, "add", ".")
    _git(tmp_path, "commit", "-qm", "main migration")
    _git(tmp_path, "update-ref", "refs/remotes/origin/main", "HEAD")
    _git(tmp_path, "checkout", "-qb", "feature", "HEAD~1")

    result = subprocess.run(
        [sys.executable, "scripts/new_migration.py", "feature"],
        cwd=tmp_path,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    assert (tmp_path / MIGRATIONS / "0.11.003_feature.sql").exists()
