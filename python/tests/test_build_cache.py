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
from build_cache import configure_cargo, install_hook, write_inventory


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
    write_inventory(repo, config, {"cache": {"disk": {"size": 123456}}})
    assert json.loads(config.read_text())["basedirs"] == [str(repo.resolve())]
    worktree = tmp_path / "later\nworktree with spaces"
    _git(repo, "worktree", "add", "--detach", str(worktree))
    write_inventory(worktree, config)
    assert set(json.loads(config.read_text())["basedirs"]) == {
        str(repo.resolve()),
        str(worktree.resolve()),
    }
    other = tmp_path / "other repo"
    other.mkdir()
    _git(other, "init", "-q")
    write_inventory(other, config)
    _git(repo, "worktree", "remove", str(worktree))
    with monkeypatch.context() as hook:
        hook.setenv("GIT_DIR", str(repo / ".git"))
        write_inventory(repo, config)
    assert set(json.loads(config.read_text())["basedirs"]) == {
        str(repo.resolve()),
        str(other.resolve()),
    }
    previous = config.stat().st_mtime_ns
    (repo / "nested").mkdir()
    write_inventory(repo / "nested", config)
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


@pytest.fixture
def pinned_cache(tmp_path, monkeypatch):
    monkeypatch.setenv("HOME", str(tmp_path))
    monkeypatch.setenv("CARGO_HOME", str(tmp_path / "cargo"))
    monkeypatch.setenv("XDG_CACHE_HOME", str(tmp_path / "cache"))
    for key in os.environ.copy():
        if key.startswith("SCCACHE_"):
            monkeypatch.delenv(key)

    wrapper = tmp_path / ".local/share/loopflow/sccache" / build_cache.REVISION / "bin/sccache"
    wrapper.parent.mkdir(parents=True)
    wrapper.touch(mode=0o755)
    env = build_cache.cache_environment()
    with tempfile.TemporaryDirectory(dir="/tmp") as short:
        env["SCCACHE_SERVER_UDS"] = str(Path(short) / "cache.sock")
        configure_cargo(build_cache.cargo_configuration(), wrapper, env)
        yield wrapper


def _cache_command(repo, *args):
    return subprocess.run(
        [sys.executable, build_cache.__file__, *args], cwd=repo, text=True, capture_output=True
    )


def test_register_recovers_a_worktree_after_a_skipped_hook(tmp_path, pinned_cache):
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
        "initial",
    )
    install_hook(repo)
    tree = tmp_path / "skipped hook"
    _git(repo, "-c", "core.hooksPath=/dev/null", "worktree", "add", "--detach", str(tree))

    config = build_cache.inventory_path()
    status = _cache_command(tree, "status")
    assert status.returncode == 3, status.stderr
    assert "missing from the root list; run register" in status.stdout
    assert not config.parent.exists()

    _cache_command(tree, "register").check_returncode()
    status = _cache_command(tree, "status", "--json")
    status.check_returncode()
    state = json.loads(status.stdout)
    assert set(state["listed_roots"]) == {str(repo.resolve()), str(tree.resolve())}
    assert state["loaded_roots"] == []
    assert state["daemon"] is None
    assert state["diagnostic"] == "this checkout is listed for the next daemon start"


def test_missing_wrapper_is_actionable(tmp_path, monkeypatch):
    monkeypatch.setattr(build_cache, "configured_cache", lambda: (tmp_path / "missing", {}))
    monkeypatch.setattr(build_cache, "inventory_path", lambda: tmp_path / "inventory")
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
    monkeypatch.setenv("XDG_CACHE_HOME", str(tmp_path / "private"))
    monkeypatch.setenv("SCCACHE_CONF", str(build_cache.inventory_path()))
    monkeypatch.setenv("SCCACHE_SERVER_UDS", str(tmp_path / "private/cache.sock"))
    install_hook(repo)
    tree = tmp_path / "new tree"
    _git(repo, "worktree", "add", "--detach", str(tree))
    config = json.loads(build_cache.inventory_path().read_text())
    assert set(config["basedirs"]) == {str(repo.resolve()), str(tree.resolve())}


@pytest.fixture
def serving_cache(tmp_path, monkeypatch, pinned_cache):
    _git(tmp_path, "init", "-q")
    marker = tmp_path / "running"
    marker.touch()
    monkeypatch.setenv("SCCACHE_TEST_MARKER", str(marker))
    monkeypatch.setenv("SCCACHE_TEST_STATS", '{"basedirs":[]}')
    monkeypatch.setenv("SCCACHE_IDLE_TIMEOUT", "120")
    pinned_cache.write_text(
        "#!/bin/sh\n"
        'case "$1" in\n'
        '--show-stats) printf "%s\\n" "$SCCACHE_TEST_STATS";;\n'
        '--stop-server) rm "$SCCACHE_TEST_MARKER";;\n'
        "*) exit 1;;\nesac\n"
    )
    monkeypatch.setattr(build_cache, "daemon_pid", lambda _: 123 if marker.exists() else None)
    monkeypatch.setattr(build_cache.sys, "platform", "darwin")
    # Report the stand-in executable as the socket owner.
    original_run = subprocess.run

    def run(argv, **kwargs):
        if argv[0] == "ps":
            value = "03:10" if argv[-1] == "etime=" else str(pinned_cache)
            return subprocess.CompletedProcess(argv, 0, value, "")
        return original_run(argv, **kwargs)

    monkeypatch.setattr(subprocess, "run", run)
    return marker


def test_register_stops_the_stale_daemon(tmp_path, serving_cache):
    write_inventory(tmp_path, build_cache.inventory_path())
    state = build_cache.cache_status(tmp_path, *build_cache.configured_cache())
    assert state["exit_code"] == 3
    assert state["listed_roots"] == [str(tmp_path.resolve())]
    assert state["loaded_roots"] == []
    assert "daemon lacks this checkout; run register" in state["diagnostic"]
    assert state["configured_idle_timeout_seconds"] == 120
    assert serving_cache.exists()

    build_cache.register(tmp_path)
    assert not serving_cache.exists()
    assert build_cache.cache_status(tmp_path, *build_cache.configured_cache())["exit_code"] == 0


def test_register_restores_missing_inventory_while_the_root_is_loaded(
    tmp_path, serving_cache, monkeypatch
):
    root = str(tmp_path.resolve())
    monkeypatch.setenv("SCCACHE_TEST_STATS", json.dumps({"basedirs": [root + "/"]}))
    state = build_cache.cache_status(tmp_path, *build_cache.configured_cache())
    assert state["exit_code"] == 3
    assert state["listed_roots"] == []
    assert state["loaded_roots"] == [root]

    build_cache.register(tmp_path)
    assert serving_cache.exists()
    assert build_cache.cache_status(tmp_path, *build_cache.configured_cache())["exit_code"] == 0


def test_register_lists_the_root_for_a_server_side_daemon(tmp_path, serving_cache, monkeypatch):
    monkeypatch.setenv("SCCACHE_CLIENT_SIDE", "0")
    with pytest.raises(ValueError, match="client-side"):
        build_cache.register(tmp_path)
    state = build_cache.cache_status(tmp_path, *build_cache.configured_cache())
    assert state["exit_code"] == 3
    assert state["listed_roots"] == [str(tmp_path.resolve())]
    assert "restart your daemon" in state["diagnostic"]
    assert serving_cache.exists()


def test_register_leaves_a_stale_caller_owned_daemon_running(tmp_path, serving_cache, monkeypatch):
    monkeypatch.setenv("SCCACHE_BASEDIRS", str(tmp_path.resolve()))
    build_cache.register(tmp_path)
    state = build_cache.cache_status(tmp_path, *build_cache.configured_cache())
    assert state["exit_code"] == 3
    assert "restart your daemon" in state["diagnostic"]
    assert not build_cache.inventory_path().exists()
    assert serving_cache.exists()


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


@pytest.mark.parametrize("source", ["json", "toml", "SCCACHE_BASEDIRS"])
def test_caller_supplied_root_configuration_is_read_only(
    tmp_path, monkeypatch, pinned_cache, source
):
    _git(tmp_path, "init", "-q")
    config = tmp_path / f"custom.{source}"
    if source == "SCCACHE_BASEDIRS":
        monkeypatch.setenv(source, "/other")
    else:
        original = '{"basedirs":["/other"]}' if source == "json" else 'basedirs = ["/other"]\n'
        config.write_text(original)
        monkeypatch.setenv("SCCACHE_CONF", str(config))

    _cache_command(tmp_path, "register").check_returncode()
    status = _cache_command(tmp_path, "status", "--json")
    assert status.returncode == 3, status.stderr
    state = json.loads(status.stdout)
    assert state["listed_roots"] == ["/other"]
    assert not state["managed_roots"]
    assert state["diagnostic"].endswith(
        f"add it to {source if source == 'SCCACHE_BASEDIRS' else config}"
    )
    if source != "SCCACHE_BASEDIRS":
        assert config.read_text() == original
    assert not build_cache.inventory_path().parent.exists()
