"""Git registration and Cargo configuration for the Machine cache."""

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


def test_roots_follow_creation_removal_and_multiple_repositories(tmp_path, monkeypatch):
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
    with monkeypatch.context() as hook:
        hook.setenv("GIT_DIR", str(repo / ".git"))
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
    assert installed.startswith("#!/bin/sh\n")
    assert installed.endswith("# existing hook\ntrue\n")
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
    monkeypatch.setattr(build_cache, "daemon_pid", lambda address: 123 if running else None)
    monkeypatch.setattr(build_cache.sys, "platform", "darwin")
    config = tmp_path / "cache.json"
    config.write_text(json.dumps({"basedirs": [], "cache": {"disk": {"size": 123}}}))
    monkeypatch.setattr(build_cache, "worktree_roots", lambda repo: {str(tmp_path)})
    monkeypatch.setattr(
        build_cache,
        "configured_cache",
        lambda: (
            wrapper,
            {
                "SCCACHE_CONF": str(config),
                "SCCACHE_SERVER_UDS": str(socket),
                "SCCACHE_IDLE_TIMEOUT": "120",
            },
        ),
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
            "03:10\n" if argv[-1] == "etime=" else f"{wrapper}\n",
            "",
        )

    monkeypatch.setattr(subprocess, "check_output", check_output)
    monkeypatch.setattr(subprocess, "run", run)
    result = build_cache.cache_status(tmp_path, *build_cache.configured_cache())
    assert result["exit_code"] == expected
    assert result["loaded_roots"] == ([str(tmp_path)] if running and loaded else [])
    assert (result["daemon"] is not None) == running
    assert result["configured_idle_timeout_seconds"] == 120
    assert result["listed_roots"] == [str(tmp_path)]
    assert json.loads(config.read_text())["cache"]["disk"]["size"] == 123


def test_missing_wrapper_is_actionable(tmp_path, monkeypatch):
    monkeypatch.setattr(build_cache, "configured_cache", lambda: (tmp_path / "missing", {}))
    monkeypatch.setattr(build_cache, "inventory_path", lambda: tmp_path / "inventory")
    monkeypatch.setattr(build_cache, "worktree_roots", lambda repo: {str(tmp_path)})
    monkeypatch.setattr(subprocess, "check_output", lambda *args, **kwargs: str(tmp_path))
    assert build_cache.cache_status(tmp_path, *build_cache.configured_cache())["exit_code"] == 1


def test_git_worktree_add_registers_even_an_older_revision_before_return(tmp_path, monkeypatch):
    repo = tmp_path / "repo"
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
        "before cache installation",
    )
    monkeypatch.setenv("CARGO_HOME", str(tmp_path / "cargo"))
    monkeypatch.setenv("SCCACHE_CONF", str(tmp_path / "private/cache.json"))
    monkeypatch.setenv("SCCACHE_SERVER_UDS", str(tmp_path / "private/cache.sock"))
    install_hook(repo)
    tree = tmp_path / "new tree"
    _git(repo, "worktree", "add", "--detach", str(tree))
    config = json.loads((tmp_path / "private/cache.json").read_text())
    assert set(config["basedirs"]) == {str(repo.resolve()), str(tree.resolve())}


@pytest.mark.parametrize(
    "loaded,client_side,stopped", [(False, "1", True), (True, "1", False), (False, "0", False)]
)
def test_registration_only_stops_an_unloaded_client_side_daemon(
    tmp_path, monkeypatch, loaded, client_side, stopped
):
    marker = tmp_path / "running"
    marker.touch()
    wrapper = tmp_path / "sccache"
    wrapper.write_text('#!/bin/sh\n[ "$1" = --stop-server ] && rm "$SCCACHE_TEST_MARKER"\n')
    wrapper.chmod(0o755)
    env = {
        "SCCACHE_CONF": str(tmp_path / "cache.json"),
        "SCCACHE_CLIENT_SIDE": client_side,
        "SCCACHE_TEST_MARKER": str(marker),
    }
    monkeypatch.setattr(build_cache, "configured_cache", lambda: (wrapper, env))
    monkeypatch.setattr(
        build_cache,
        "cache_status",
        lambda *args: {"exit_code": 0 if loaded else 3, "daemon": {"pid": 1}},
    )
    if not loaded and client_side != "1":
        with pytest.raises(ValueError, match="client-side"):
            build_cache.register(tmp_path)
    else:
        build_cache.register(tmp_path)
    assert marker.exists() == (not stopped)


def test_socket_reports_the_replacement_after_the_listener_is_unlinked():
    with tempfile.TemporaryDirectory(dir="/tmp") as short, socket.socket(socket.AF_UNIX) as old:
        address = str(Path(short) / "cache.sock")
        old.bind(address)
        old.listen()
        assert build_cache.daemon_pid(address) == os.getpid()
        os.unlink(address)
        assert build_cache.daemon_pid(address) is None
        with subprocess.Popen(
            [
                sys.executable,
                "-c",
                "import socket, sys; s=socket.socket(socket.AF_UNIX); s.bind(sys.argv[1]); "
                "s.listen(); print('ready', flush=True); sys.stdin.read()",
                address,
            ],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True,
        ) as replacement:
            try:
                assert replacement.stdout.readline() == "ready\n"
                assert build_cache.daemon_pid(address) == replacement.pid
            finally:
                replacement.communicate()


def test_registration_observes_the_same_private_socket_and_inventory_as_cargo(
    tmp_path, monkeypatch
):
    monkeypatch.setenv("CARGO_HOME", str(tmp_path))
    monkeypatch.setenv("SCCACHE_SERVER_UDS", "/private/socket")
    monkeypatch.setenv("SCCACHE_CONF", "/private/inventory")
    configure_cargo(
        tmp_path / "config.toml",
        Path("/pinned/sccache"),
        {"SCCACHE_SERVER_UDS": "/machine/socket", "SCCACHE_CONF": "/machine/inventory"},
    )
    _, env = build_cache.configured_cache()
    assert env["SCCACHE_SERVER_UDS"] == "/private/socket"
    assert env["SCCACHE_CONF"] == "/private/inventory"
