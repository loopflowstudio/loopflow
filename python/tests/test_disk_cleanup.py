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


def _run(tmp_path: Path, stubs: dict[str, str] | None = None, exit_code: int = 0) -> str:
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    for tool, body in {"lf": "exit 0", "uv": "exit 0", "xcrun": "exit 0", **(stubs or {})}.items():
        stub = bin_dir / tool
        stub.write_text(f"#!/bin/sh\n{body}\n")
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
    assert result.returncode == exit_code, result.stdout + result.stderr
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


@pytest.mark.parametrize("held", [False, True])
def test_backup_with_recent_or_open_sidecar_is_kept(tmp_path: Path, held: bool) -> None:
    lf = tmp_path / "home/.lf"
    old = lf / "loopflow.db.backup-old"
    sidecar = Path(f"{old}-wal")
    _write(old, 30)
    _write(sidecar, 30 if held else 0)
    _write(lf / "loopflow.db.backup-new", 10)

    # A read-only handle leaves the sidecar's old timestamp intact.
    with sidecar.open("rb") if held else open(os.devnull, "rb"):
        out = _run(tmp_path)

    assert old.exists(), out
    assert sidecar.exists(), out


def test_cache_and_trash_keep_open_or_recent_contents_and_log_removals(tmp_path: Path) -> None:
    home = tmp_path / "home"
    for root in (home / "Library/Caches", home / ".Trash"):
        for name, age in (("old", 40), ("held", 40), ("recent", 0)):
            _write(root / name / "data", age)
            _age_tree(root / name, 40)

    with (home / "Library/Caches/held/data").open("rb"), (home / ".Trash/held/data").open("rb"):
        out = _run(tmp_path)

    for root in (home / "Library/Caches", home / ".Trash"):
        assert not (root / "old").exists(), out
        assert (root / "held/data").exists(), out
        assert (root / "recent/data").exists(), out
    log = (home / ".lf/logs/disk.log").read_text()
    assert "removed cache" in log
    assert "removed trash" in log


def test_failed_open_file_inspection_stops_all_cleanup(tmp_path: Path) -> None:
    cache = tmp_path / "home/Library/Caches/old/data"
    _write(cache, 40)
    _age_tree(cache.parent, 40)

    out = _run(tmp_path, {"lsof": "exit 1"}, exit_code=1)

    assert cache.exists(), out
    assert "open-file inspection failed; stopping cleanup" in out


def test_failed_modification_inspection_keeps_material(tmp_path: Path) -> None:
    cache = tmp_path / "home/Library/Caches/old/data"
    _write(cache, 40)
    _age_tree(cache.parent, 40)

    out = _run(
        tmp_path, {"find": 'case "$1" in */Caches/old) exit 1;; esac\nexec /usr/bin/find "$@"'}
    )

    assert cache.exists(), out
    assert "modification inspection failed" in out


def test_process_in_checkout_keeps_its_idle_build_output(tmp_path: Path) -> None:
    repo = tmp_path / "home/src/kata"
    repo.mkdir(parents=True)
    subprocess.run(["git", "init", "-q", str(repo)], check=True)
    (repo / ".gitignore").write_text("target/\n")
    _write(repo / "target/debug/kata", 3)
    _age_tree(repo / "target", 3)
    process = subprocess.Popen(["/bin/sleep", "600"], cwd=repo)
    try:
        out = _run(tmp_path)
        assert (repo / "target/debug/kata").exists(), out
        assert f"kept {repo} (held open)" in out
    finally:
        process.terminate()
        process.wait(timeout=10)
