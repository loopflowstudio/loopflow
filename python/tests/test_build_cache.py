"""Derived cache configuration follows Git's worktrees without daemon control."""

import json
import os
import socket
import subprocess
import sys
import tempfile
from pathlib import Path

import build_cache
import pytest
from build_cache import configure_cargo, install_hook, refresh


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
    refresh(repo, config, {"cache": {"disk": {"size": 123456}}})
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


def test_cargo_configuration_keeps_unrelated_bytes_and_is_idempotent(tmp_path):
    config = tmp_path / "config.toml"
    original = (
        "# machine\n[build]\njobs = 2 # keep\n"
        'rustc-wrapper = "stock" # wrapper\n\n[env]\n'
        'OTHER = { value = "yes", force = true } # keep\n\n[net]\nretry=3\n'
    )
    config.write_text(original)
    configure_cargo(config, Path("/stable/sccache"), {"SCCACHE_CONF": "/cache/config"})
    expected = original.replace('"stock"', '"/stable/sccache"').replace(
        "\n[net]", 'SCCACHE_CONF = "/cache/config"\n\n[net]'
    )
    assert config.read_text() == expected
    before = config.stat().st_mtime_ns
    configure_cargo(config, Path("/stable/sccache"), {"SCCACHE_CONF": "/cache/config"})
    assert config.stat().st_mtime_ns == before


def test_worktree_hook_keeps_existing_content_and_runs_fail_soft(tmp_path):
    repo = tmp_path / "repo"
    repo.mkdir()
    _git(repo, "init", "-q")
    hooks = tmp_path / "hooks"
    hooks.mkdir()
    _git(repo, "config", "core.hooksPath", str(hooks))
    hook = hooks / "post-checkout"
    original = "#!/bin/sh\n# existing hook\ntrue\n"
    hook.write_text(original)
    install_hook(repo)
    installed = hook.read_text()
    install_hook(repo)
    assert hook.read_text() == installed
    assert installed.startswith(original)
    assert installed.count("# loopflow build cache begin") == 1
    assert subprocess.run([str(hook)], cwd=repo).returncode == 0


@pytest.mark.parametrize(
    "running,loaded,expected", [(False, False, 0), (True, False, 3), (True, True, 0)]
)
def test_status_classifies_loaded_roots(tmp_path, monkeypatch, running, loaded, expected):
    monkeypatch.setenv("HOME", str(tmp_path))
    wrapper = tmp_path / ".local/share/loopflow/sccache" / build_cache.REVISION / "bin/sccache"
    wrapper.parent.mkdir(parents=True)
    wrapper.touch()
    wrapper.chmod(0o755)
    socket = tmp_path / "cache.sock"
    if running:
        socket.touch()
    config = tmp_path / "cache.json"
    config.write_text(json.dumps({"basedirs": [str(tmp_path)]}))
    monkeypatch.setattr(
        build_cache,
        "configured_cache",
        lambda: (wrapper, {"SCCACHE_CONF": str(config), "SCCACHE_SERVER_UDS": str(socket)}),
    )
    stats = {
        "basedirs": [str(tmp_path) + "/"] if loaded else [],
        "stats": {},
        "cache_size": 10,
        "max_cache_size": 100,
    }

    def check_output(argv, **kwargs):
        return str(tmp_path) if argv[0] == "git" else json.dumps(stats)

    def run(argv, **kwargs):
        return subprocess.CompletedProcess(
            argv,
            0,
            "123\n" if argv[0] == "lsof" else "03:10\n" if argv[-1] == "etime=" else f"{wrapper}\n",
            "",
        )

    monkeypatch.setattr(subprocess, "check_output", check_output)
    monkeypatch.setattr(subprocess, "run", run)
    result = build_cache.cache_status(tmp_path)
    assert result["exit_code"] == expected
    assert result["loaded_roots"] == ([str(tmp_path)] if running and loaded else [])
    assert (result["daemon"] is not None) == running
    assert result["idle_seconds"] is None


def test_missing_wrapper_is_actionable(tmp_path, monkeypatch):
    monkeypatch.setattr(build_cache, "configured_cache", lambda: (tmp_path / "missing", {}))
    monkeypatch.setattr(build_cache, "inventory_path", lambda: tmp_path / "inventory")
    monkeypatch.setattr(subprocess, "check_output", lambda *args, **kwargs: str(tmp_path))
    assert build_cache.cache_status(tmp_path)["exit_code"] == 1


def test_git_worktree_add_registers_the_new_root(tmp_path, monkeypatch):
    repo = tmp_path / "repo"
    (repo / "scripts").mkdir(parents=True)
    source = Path(__file__).resolve().parents[2] / "scripts/build_cache.py"
    (repo / "scripts/build_cache.py").write_text(source.read_text())
    _git(repo, "init", "-q")
    _git(repo, "add", ".")
    _git(
        repo,
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=fixture@example.com",
        "commit",
        "-qm",
        "fixture",
    )
    install_hook(repo)
    commands = tmp_path / "bin"
    commands.mkdir()
    # Keep the socket short on macOS; pytest's canonical temp path can exceed it.

    with tempfile.TemporaryDirectory(prefix="lf-hook-", dir="/tmp") as short:
        notify = Path(short) / "ready.sock"
        stub = commands / "uv"
        stub.write_text(
            f"#!{sys.executable}\nimport os, socket, subprocess, sys\n"
            "result = subprocess.run([sys.executable, *sys.argv[3:]])\n"
            "with socket.socket(socket.AF_UNIX) as client:\n"
            '    client.connect(os.environ["CACHE_TEST_NOTIFY"])\n'
            "    client.sendall(str(result.returncode).encode())\n"
        )
        stub.chmod(0o755)
        monkeypatch.setenv("PATH", f"{commands}:{os.environ['PATH']}")
        monkeypatch.setenv("XDG_CACHE_HOME", str(tmp_path / "cache"))
        monkeypatch.setenv("CACHE_TEST_NOTIFY", str(notify))
        with socket.socket(socket.AF_UNIX) as listener:
            listener.bind(str(notify))
            listener.listen(1)
            listener.settimeout(10)
            tree = tmp_path / "new tree"
            _git(repo, "worktree", "add", "--detach", str(tree))
            with listener.accept()[0] as client:
                assert client.recv(100) == b"0"
        config = json.loads((tmp_path / "cache/loopflow/sccache.json").read_text())
        assert set(config["basedirs"]) == {str(repo.resolve()), str(tree.resolve())}
