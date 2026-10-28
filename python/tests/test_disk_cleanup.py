import os
import subprocess
import sys
import time
from pathlib import Path

import pytest

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "disk-cleanup.sh"

pytestmark = pytest.mark.skipif(sys.platform != "darwin", reason="macOS maintenance script")


def _write(path: Path, age_days: float, size: int = 4096) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(b"x" * size)
    _age(path, age_days)


def _age(path: Path, age_days: float) -> None:
    stamp = time.time() - age_days * 86400
    os.utime(path, (stamp, stamp))


def _age_tree(root: Path, age_days: float) -> None:
    for directory, _, _ in sorted(os.walk(root), key=lambda entry: -len(entry[0])):
        _age(Path(directory), age_days)


def _run(tmp_path: Path) -> str:
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    for tool in ("lf", "uv", "xcrun"):
        stub = bin_dir / tool
        stub.write_text("#!/bin/sh\nexit 0\n")
        stub.chmod(0o755)
    env = {
        "HOME": str(tmp_path / "home"),
        "LF_HOME": str(tmp_path / "home/.lf"),
        "LOOPFLOW_SRC": str(tmp_path / "home/src"),
        "TMPDIR": str(tmp_path),
        "PATH": f"{bin_dir}:/usr/bin:/bin:/usr/sbin:/sbin",
    }
    result = subprocess.run(
        ["bash", str(SCRIPT)], env=env, capture_output=True, text=True, timeout=600
    )
    assert result.returncode == 0, result.stdout + result.stderr
    return result.stdout


def test_home_rotation_removes_week_old_material_and_keeps_the_store(tmp_path: Path) -> None:
    lf = tmp_path / "home/.lf"
    _write(lf / "loopflow.db", 30)
    _write(lf / "loopflow.db-wal", 30)
    _write(lf / "accounts/codex/a/sessions/old.jsonl", 60)
    _write(lf / "loopflow.db.backup-0.14.7-0123456789abcdef", 20)
    _write(lf / "loopflow.db.backup-0.14.7-0123456789abcdef-shm", 20)
    _write(lf / "loopflow.db.backup-0.14.8-fedcba9876543210", 10)
    _write(lf / "runs/ab/oldest/events.jsonl", 40)
    _write(lf / "runs/ef/fresh/events.jsonl", 0)
    _write(lf / "logs/old.log", 30)
    _write(lf / "logs/held.log", 30)
    _write(lf / "copies/.tmpAbC123/loopflow.db", 3)
    _age_tree(lf / "runs/ab", 40)

    with open(lf / "logs/held.log", "a"):
        out = _run(tmp_path)

    for gone in (
        "runs/ab/oldest",
        "logs/old.log",
        "loopflow.db.backup-0.14.7-0123456789abcdef",
        "loopflow.db.backup-0.14.7-0123456789abcdef-shm",
    ):
        assert not (lf / gone).exists(), f"{gone} survived:\n{out}"
    for kept in (
        "loopflow.db",
        "loopflow.db-wal",
        "accounts/codex/a/sessions/old.jsonl",
        "loopflow.db.backup-0.14.8-fedcba9876543210",
        "runs/ef/fresh/events.jsonl",
        "logs/held.log",
        "copies/.tmpAbC123",
    ):
        assert (lf / kept).exists(), f"{kept} was removed:\n{out}"
    assert "removed captures" in (lf / "logs/disk.log").read_text()


def test_idle_ignored_build_output_goes_and_recent_or_tracked_output_stays(
    tmp_path: Path,
) -> None:
    repo = tmp_path / "home/src/kata"
    repo.mkdir(parents=True)
    subprocess.run(["git", "init", "-q", str(repo)], check=True)
    (repo / ".gitignore").write_text("target/\nnode_modules/\n")
    _write(repo / "target/debug/kata", 3)
    _write(repo / "node_modules/left-pad/index.js", 0)
    _write(repo / ".venv/bin/python", 3)
    _age_tree(repo / "target", 3)
    _age_tree(repo / ".venv", 3)

    out = _run(tmp_path)

    assert not (repo / "target").exists(), out
    assert (repo / "node_modules").exists(), out
    assert (repo / ".venv").exists(), out
    assert "== repository kata" in out


def test_linked_worktree_loses_output_idle_for_four_hours(tmp_path: Path) -> None:
    src = tmp_path / "home/src"
    repo = src / "kata"
    repo.mkdir(parents=True)
    git = ["git", "-c", "user.name=t", "-c", "user.email=t@t"]
    subprocess.run(["git", "init", "-q", str(repo)], check=True)
    (repo / ".gitignore").write_text("target/\n")
    subprocess.run([*git, "-C", str(repo), "add", "."], check=True)
    subprocess.run([*git, "-C", str(repo), "commit", "-qm", "init"], check=True)
    idle, recent = src / "kata.idle", src / "kata.recent"
    for wt in (idle, recent):
        subprocess.run(["git", "-C", str(repo), "worktree", "add", "-q", str(wt)], check=True)
    _write(idle / "target/debug/kata", 0.25)
    _age_tree(idle / "target", 0.25)
    _write(recent / "target/debug/kata", 0.1)
    _age_tree(recent / "target", 0.1)

    out = _run(tmp_path)

    assert not (idle / "target").exists(), out
    assert (recent / "target").exists(), out
    assert "written in the last 4 hours" in out
