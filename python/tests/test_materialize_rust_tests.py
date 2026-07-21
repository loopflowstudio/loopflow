"""Local Rust tests use CI's schema without changing the active checkout."""

import ast
import os
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/materialize_rust_tests.py"
AMBIENT_WORK_AUTHORITY = (
    "LF_RUN_CONTEXT",
    "LF_RUN_LEASE",
    "LF_WAVE_ID",
    "LF_ACCOUNT_LEASE",
)


def test_wrapper_preserves_the_declared_python_38_floor() -> None:
    assert 'requires-python = ">=3.8"' in (ROOT / "pyproject.toml").read_text()
    source = SCRIPT.read_text()
    tree = ast.parse(source, filename=str(SCRIPT), feature_version=8)
    imports = {
        alias.name
        for node in ast.walk(tree)
        if isinstance(node, ast.Import)
        for alias in node.names
    }
    imports.update(
        node.module
        for node in ast.walk(tree)
        if isinstance(node, ast.ImportFrom) and node.module is not None
    )
    assert "tomllib" not in imports


def _git(repo: Path, *arguments: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *arguments],
        cwd=repo,
        check=True,
        capture_output=True,
        text=True,
    )


@pytest.fixture
def repo(tmp_path: Path) -> Path:
    (tmp_path / "scripts").mkdir()
    (tmp_path / "scripts/materialize_rust_tests.py").write_bytes(SCRIPT.read_bytes())
    (tmp_path / "scripts/canonicalize_migrations.py").write_text(
        """from pathlib import Path
import sys

assert sys.argv[1:] == ["0.12.3", "--materialize-for-tests"]
Path("materialized.txt").write_text("ready")
"""
    )
    (tmp_path / "Cargo.toml").write_text('[workspace.package]\nversion = "0.12.3"\n')
    (tmp_path / "tracked.txt").write_text("committed\n")
    _git(tmp_path, "init", "-b", "main")
    _git(tmp_path, "config", "user.name", "Loopflow Tests")
    _git(tmp_path, "config", "user.email", "tests@loopflow.local")
    _git(tmp_path, "add", ".")
    _git(tmp_path, "commit", "-m", "fixture")
    return tmp_path


@pytest.mark.parametrize("exit_code", [0, 7])
def test_materialized_command_sees_exact_tree_and_always_cleans_up(
    repo: Path, exit_code: int
) -> None:
    (repo / "tracked.txt").write_text("dirty\n")
    _git(repo, "add", "tracked.txt")
    (repo / "untracked.txt").write_text("new\n")
    probe = (
        "from pathlib import Path; import os; import sys; "
        "assert Path('tracked.txt').read_text() == 'dirty\\n'; "
        "assert Path('untracked.txt').read_text() == 'new\\n'; "
        "assert Path('materialized.txt').read_text() == 'ready'; "
        f"assert all(name not in os.environ for name in {AMBIENT_WORK_AUTHORITY!r}); "
        "Path('command-ran.txt').write_text('yes'); "
        f"sys.exit({exit_code})"
    )
    environment = os.environ.copy()
    environment.update(
        {name: "must-not-escape" for name in AMBIENT_WORK_AUTHORITY}
    )

    result = subprocess.run(
        [
            sys.executable,
            "scripts/materialize_rust_tests.py",
            "--",
            sys.executable,
            "-c",
            probe,
        ],
        cwd=repo,
        capture_output=True,
        text=True,
        env=environment,
    )

    assert result.returncode == exit_code, result.stderr
    assert (repo / "tracked.txt").read_text() == "dirty\n"
    assert (repo / "untracked.txt").read_text() == "new\n"
    assert not (repo / "materialized.txt").exists()
    assert not (repo / "command-ran.txt").exists()
    worktrees = _git(repo, "worktree", "list", "--porcelain").stdout
    assert "loopflow-materialized-" not in worktrees
