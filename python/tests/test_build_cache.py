"""Derived cache configuration follows Git's worktrees without daemon control."""

import json
import subprocess
from pathlib import Path

from build_cache import VERBATIM_ENV_DEPS, refresh


def _git(directory: Path, *args: str) -> None:
    subprocess.run(["git", "-C", str(directory), *args], check=True, capture_output=True)


def test_roots_follow_creation_removal_and_multiple_repositories(tmp_path):
    repo = tmp_path / "first repo"
    repo.mkdir()
    _git(repo, "init", "-q")
    _git(
        repo,
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=fixture@example.com",
        "commit",
        "--allow-empty",
        "-qm",
        "initial",
    )
    config = tmp_path / "cache/config.json"
    env = refresh(repo, config, {"cache": {"disk": {"size": 123456}}})
    assert env["SCCACHE_BASEDIRS_VERBATIM_ENV_DEPS"] == VERBATIM_ENV_DEPS
    assert json.loads(config.read_text())["basedirs"] == [str(repo.resolve())]
    worktree = tmp_path / "later\nworktree with spaces"
    _git(repo, "worktree", "add", "--detach", str(worktree))
    refresh(worktree, config)
    assert set(json.loads(config.read_text())["basedirs"]) == {
        str(repo.resolve()),
        str(worktree.resolve()),
    }
    other = tmp_path / "other repo"
    other.mkdir()
    _git(other, "init", "-q")
    refresh(other, config)
    _git(repo, "worktree", "remove", str(worktree))
    refresh(repo, config)
    assert set(json.loads(config.read_text())["basedirs"]) == {
        str(repo.resolve()),
        str(other.resolve()),
    }
    previous = config.stat().st_mtime_ns
    (repo / "nested").mkdir()
    refresh(repo / "nested", config)
    assert config.stat().st_mtime_ns == previous
    assert json.loads(config.read_text())["cache"]["disk"]["size"] == 123456
