"""Local Rust tests use CI's schema without changing the active checkout."""

import ast
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/materialize_rust_tests.py"
AMBIENT_WORK_AUTHORITY = (
    "LF_AS",
    "LF_BIN",
    "LF_RUN_ID",
    "LF_CAPTURE_KEY",
    "LF_AGENT_CALLER",
    "LF_WAVE_ID",
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
    (repo / "tracked.txt").write_text("staged\n")
    _git(repo, "add", "tracked.txt")
    (repo / "tracked.txt").write_text("dirty\n")
    (repo / "untracked.txt").write_text("new\n")
    probe = (
        "from pathlib import Path; import os; import sys; "
        "assert Path('tracked.txt').read_text() == 'dirty\\n'; "
        "assert Path('untracked.txt').read_text() == 'new\\n'; "
        "assert Path('materialized.txt').read_text() == 'ready'; "
        f"assert all(name not in os.environ for name in {AMBIENT_WORK_AUTHORITY!r}); "
        "assert Path(os.environ['LF_HOME']).name == '.lf-test-home'; "
        "Path('command-ran.txt').write_text('yes'); "
        f"sys.exit({exit_code})"
    )
    environment = os.environ.copy()
    environment.update({name: "must-not-escape" for name in AMBIENT_WORK_AUTHORITY})

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


@pytest.fixture
def cargo_probe(repo: Path, tmp_path: Path) -> Path:
    tools = tmp_path / "tools"
    tools.mkdir()
    cargo = tools / "cargo"
    cargo.write_text(
        f"#!{sys.executable}\n"
        + """import json
import os
import sys
from pathlib import Path

assert not any(name.startswith("LF_") and name != "LF_HOME" for name in os.environ)
assert "LOOPFLOW_TEST_LF" not in os.environ
assert os.environ["LOOPFLOW_BUILD_PROVENANCE"] == "development"
assert os.environ["LOOPFLOW_MIGRATION_AUTHORITY"] == "validation_only"
assert "LOOPFLOW_RELEASE_TAG" not in os.environ
assert sys.stdin.read() == ""
assert Path("tracked.txt").read_text() == "dirty\\n"
assert Path("untracked.txt").read_text() == "new\\n"
assert Path("materialized.txt").read_text() == "ready"
mode = os.environ.get("PROBE_MODE", "success")
if mode == "failure":
    sys.exit(9)
target = Path(os.environ["CARGO_TARGET_DIR"])
if "--target-dir" in sys.argv:
    target = Path(sys.argv[sys.argv.index("--target-dir") + 1])
assert target.is_absolute()
binary = target / "artifact-from-json"
binary.parent.mkdir(parents=True, exist_ok=True)
binary.write_text('#!/bin/sh\\nmkdir -p "$LF_HOME"\\n'
                  'printf "dirty/new/ready\\\\n" | tee "$LF_HOME/fixture"\\n')
binary.chmod(0o755)
artifact = {"reason": "compiler-artifact", "target": {"name": "lf", "kind": ["bin"]},
            "profile": {"test": False}, "executable": str(binary)}
test_artifact = dict(artifact, profile={"test": True}, executable=str(binary) + "-harness")
print(json.dumps(test_artifact))
if mode != "missing":
    print(json.dumps(artifact))
if mode == "ambiguous":
    artifact["executable"] += "-other"
    print(json.dumps(artifact))
if mode == "unavailable":
    binary.unlink()
"""
    )
    cargo.chmod(0o755)
    (repo / "tracked.txt").write_text("dirty\n")
    (repo / "untracked.txt").write_text("new\n")
    return tools


@pytest.mark.parametrize("target", [None, "target", "custom-target", "command-target"])
def test_prepared_cli_uses_exact_tree_and_artifact_with_isolated_homes(
    repo: Path, cargo_probe: Path, target: str | None
) -> None:
    environment = os.environ.copy()
    environment["PATH"] = str(cargo_probe) + os.pathsep + environment["PATH"]
    environment["LOOPFLOW_TEST_LF"] = "/stale/installed/lf"
    environment["LF_UNKNOWN_AUTHORITY"] = "must-not-escape"
    environment["LOOPFLOW_BUILD_PROVENANCE"] = "release"
    environment["LOOPFLOW_MIGRATION_AUTHORITY"] = "published"
    environment["LOOPFLOW_RELEASE_TAG"] = "v0.12.3"
    environment.pop("CARGO_TARGET_DIR", None)
    if target is not None and target != "command-target":
        environment["CARGO_TARGET_DIR"] = target
    report = repo / "homes.json"
    probe = """import json, os, subprocess, sys, tempfile
from pathlib import Path
assert subprocess.check_output([os.environ["LOOPFLOW_TEST_LF"]], text=True) == "dirty/new/ready\\n"
assert not any(name.startswith("LF_") and name != "LF_HOME" for name in os.environ)
assert sys.stdin.read() == ""
with tempfile.TemporaryDirectory() as a, tempfile.TemporaryDirectory() as b:
    for home in (a, b):
        env = dict(os.environ, LF_HOME=home)
        subprocess.run([os.environ["LOOPFLOW_TEST_LF"]], env=env, check=True, capture_output=True)
        assert Path(home, "fixture").read_text() == "dirty/new/ready\\n"
    Path(REPORT).write_text(json.dumps([a, b]))
""".replace("REPORT", repr(str(report)))
    result = subprocess.run(
        [
            sys.executable,
            "scripts/materialize_rust_tests.py",
            "--prepare-cli",
            "--",
            sys.executable,
            "-c",
            probe,
            *(["--target-dir=custom-target"] if target == "command-target" else []),
        ],
        cwd=repo,
        env=environment,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stderr
    homes = json.loads(report.read_text())
    assert homes[0] != homes[1]
    assert all(not Path(home).exists() for home in homes)
    expected = (
        repo
        / ("custom-target" if target in {"custom-target", "command-target"} else "target")
        / "materialized-tests"
    )
    assert (expected / "artifact-from-json").exists()
    assert not (repo / "materialized.txt").exists()
    assert "loopflow-materialized-" not in _git(repo, "worktree", "list", "--porcelain").stdout


@pytest.mark.parametrize("mode", ["failure", "missing", "ambiguous", "unavailable"])
def test_preparation_failure_never_runs_tests_and_removes_worktree(
    repo: Path, cargo_probe: Path, mode: str
) -> None:
    environment = dict(
        os.environ, PATH=str(cargo_probe) + os.pathsep + os.environ["PATH"], PROBE_MODE=mode
    )
    environment.pop("CARGO_TARGET_DIR", None)
    result = subprocess.run(
        [
            sys.executable,
            "scripts/materialize_rust_tests.py",
            "--prepare-cli",
            "--",
            sys.executable,
            "-c",
            "raise RuntimeError('tests ran after failed preparation')",
        ],
        cwd=repo,
        env=environment,
        capture_output=True,
        text=True,
    )
    assert result.returncode != 0
    assert "tests ran after failed preparation" not in result.stderr
    assert "materialized Rust test setup failed" in result.stderr
    assert "loopflow-materialized-" not in _git(repo, "worktree", "list", "--porcelain").stdout


def test_prepared_cli_builds_real_cargo_artifact_offline_without_touching_source(
    repo: Path,
) -> None:
    if shutil.which("cargo") is None:
        pytest.skip("Cargo unavailable; Rust gate owns the real artifact probe")
    (repo / "Cargo.toml").write_text(
        '[workspace]\nmembers = ["."]\n[workspace.package]\nversion = "0.12.3"\n'
        '[package]\nname = "loopflow"\nversion.workspace = true\nedition = "2021"\n'
        '[[bin]]\nname = "lf"\npath = "src/main.rs"\n'
    )
    (repo / "src").mkdir()
    (repo / "src/main.rs").write_text(
        'fn main() { print!("{}{}{}", include_str!("../tracked.txt"), '
        'include_str!("../untracked.txt"), include_str!("../materialized.txt")); }\n'
    )
    (repo / "tracked.txt").write_text("dirty\n")
    (repo / "untracked.txt").write_text("new\n")
    environment = os.environ.copy()
    environment.pop("CARGO_TARGET_DIR", None)
    result = subprocess.run(
        [
            sys.executable,
            "scripts/materialize_rust_tests.py",
            "--prepare-cli",
            "--",
            sys.executable,
            "-c",
            "import os, subprocess; "
            "assert subprocess.check_output([os.environ['LOOPFLOW_TEST_LF']], text=True) "
            "== 'dirty\\nnew\\nready'",
            "--offline",
        ],
        cwd=repo,
        env=environment,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stderr
    assert not (repo / "Cargo.lock").exists()
    assert not (repo / "materialized.txt").exists()
    assert "loopflow-materialized-" not in _git(repo, "worktree", "list", "--porcelain").stdout
