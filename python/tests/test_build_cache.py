"""Git registration and Cargo configuration for the Machine cache."""

import errno
import fcntl
import json
import os
import select
import socket
import subprocess
import sys
import tempfile
from contextlib import ExitStack
from pathlib import Path

import build_cache
import pytest
from build_cache import configure_cargo, install_hook, write_inventory


def _git(directory: Path, *args: str) -> None:
    subprocess.run(["git", "-C", str(directory), *args], check=True, capture_output=True)


@pytest.fixture
def repo(tmp_path):
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
    return repo


def test_roots_follow_creation_removal_and_multiple_repositories(repo, tmp_path, monkeypatch):
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
    original = '#!/bin/sh\nprintf "%s\\n" "$@" > hook-args\nexit 23\n'
    hook.write_text(original)
    hook.chmod(0o755)
    install_hook(repo)
    installed = hook.read_text()
    install_hook(repo)
    assert hook.read_text() == installed
    assert installed.startswith("#!/bin/sh\n")
    assert installed.endswith(original.split("\n", 1)[1])
    assert installed.count("# loopflow build cache begin") == 1
    (hooks / "loopflow-build-cache.py").unlink()
    assert subprocess.run([str(hook), "old", "new", "1"], cwd=repo).returncode == 23
    assert (repo / "hook-args").read_text().splitlines() == ["old", "new", "1"]


@pytest.mark.parametrize("symlink,executable", [(False, True), (True, True), (True, False)])
def test_git_checkout_chains_an_existing_python_hook(repo, pinned_cache, symlink, executable):
    hooks = repo / ".git/hooks"
    hook = hooks / "post-checkout"
    program = hooks / "original.py" if symlink else hook
    original = (
        b"#!/usr/bin/env python3\nimport sys\nfrom pathlib import Path\n"
        b'Path("hook-args").write_text(" ".join(sys.argv[1:]))\n'
    )
    program.write_bytes(original)
    program.chmod(0o755 if executable else 0o644)
    if symlink:
        hook.symlink_to(program.name)
    install_hook(repo)
    install_hook(repo)

    tree = repo.parent / "new tree"
    _git(repo, "worktree", "add", "--detach", str(tree))
    assert (hooks / "post-checkout.before-loopflow").read_bytes() == original
    assert str(tree.resolve()) in json.loads(build_cache.inventory_path().read_text())["basedirs"]
    if executable:
        revision = subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=repo, text=True
        ).strip()
        assert (tree / "hook-args").read_text().split() == ["0" * len(revision), revision, "1"]
    else:
        assert not (tree / "hook-args").exists()


def test_installer_uses_the_same_personal_configuration_as_cargo(tmp_path, monkeypatch):
    monkeypatch.setenv("CARGO_HOME", str(tmp_path))
    legacy = tmp_path / "config"
    modern = tmp_path / "config.toml"
    modern.write_text("[build]\njobs = 3\n")
    legacy.write_text("[build]\njobs = 2\n")
    configure_cargo(build_cache.cargo_configuration(), Path("/pinned/sccache"), {})
    assert 'rustc-wrapper = "/pinned/sccache"' in legacy.read_text()
    assert modern.read_text() == "[build]\njobs = 3\n"
    assert build_cache.configured_cache()[0] == Path("/pinned/sccache")


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


def test_register_recovers_a_worktree_after_a_skipped_hook(repo, tmp_path, pinned_cache):
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


def test_missing_wrapper_is_actionable(repo, pinned_cache):
    pinned_cache.unlink()
    status = _cache_command(repo, "status", "--json")
    assert status.returncode == 1
    assert "run install" in json.loads(status.stdout)["diagnostic"]


def test_git_worktree_add_registers_even_an_older_revision_before_return(
    repo, tmp_path, pinned_cache
):
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


def test_status_reads_a_fresh_daemon_before_its_first_cache_write(
    tmp_path, serving_cache, monkeypatch, capsys
):
    write_inventory(tmp_path, build_cache.inventory_path())
    stats = {
        "basedirs": [str(tmp_path.resolve())],
        "stats": {"cache_hits": {"counts": {}}, "cache_misses": {"counts": {}}},
        "cache_size": None,
        "max_cache_size": 10 * 1024**3,
    }
    monkeypatch.setenv("SCCACHE_TEST_STATS", json.dumps(stats))
    monkeypatch.chdir(tmp_path)
    monkeypatch.setattr(sys, "argv", ["build_cache.py", "status"])
    with pytest.raises(SystemExit) as exit:
        build_cache.main()
    assert exit.value.code == 0
    output = capsys.readouterr().out
    assert "roots    1 of 1 loaded; this checkout loaded" in output
    assert "cache    unmeasured" in output


# --- Target seeding ---


@pytest.fixture
def cargo_repo(tmp_path, monkeypatch):
    """A repository whose app depends on a build-script crate outside the checkout."""
    monkeypatch.setenv("HOME", str(tmp_path))
    monkeypatch.setenv("CARGO_HOME", str(tmp_path / "cargo"))
    monkeypatch.setenv("XDG_CACHE_HOME", str(tmp_path / "cache"))
    monkeypatch.setenv("RUSTC_WRAPPER", "")
    monkeypatch.setenv("CARGO_INCREMENTAL", "1")
    for key in os.environ.copy():
        if key.startswith(("SCCACHE_", "CARGO_TARGET_", "CARGO_BUILD_")):
            monkeypatch.delenv(key)
    vendor = tmp_path / "vendor/scripted"
    (vendor / "src").mkdir(parents=True)
    (vendor / "Cargo.toml").write_text(
        '[package]\nname="scripted"\nversion="0.1.0"\nedition="2021"\n'
    )
    (vendor / "build.rs").write_text('fn main() { println!("cargo:rerun-if-changed=build.rs"); }\n')
    (vendor / "src/lib.rs").write_text("pub fn value() -> u8 { 1 }\n")
    repo = tmp_path / "source"
    (repo / "src").mkdir(parents=True)
    (repo / ".gitignore").write_text("/target\n")
    (repo / "Cargo.toml").write_text(
        '[package]\nname="app"\nversion="0.1.0"\nedition="2021"\n'
        f"[dependencies]\nscripted={{path={json.dumps(str(vendor))}}}\n"
    )
    (repo / "src/main.rs").write_text('fn main() { println!("{}", scripted::value()); }\n')
    _git(repo, "init", "-q")
    _cargo_build(repo)
    _git(repo, "add", ".")
    _git(repo, "-c", "user.name=Fixture", "-c", "user.email=f@example.com", "commit", "-qm", "init")
    return repo


@pytest.fixture
def cloning(tmp_path):
    """Skip where the temporary filesystem has no copy-on-write clones (ext4)."""
    probe = tmp_path / "clone-probe"
    probe.mkdir()
    try:
        build_cache._clone(probe, tmp_path / "clone-probe-copy")
    except (OSError, subprocess.CalledProcessError):
        pytest.skip("this filesystem cannot clone")


def _add_worktree(repo: Path, name: str) -> Path:
    tree = repo.parent / name
    _git(repo, "worktree", "add", "--detach", str(tree))
    return tree


def _cargo_build(directory: Path) -> dict[str, bool]:
    """Build and return each compiled unit's freshness by crate name."""
    result = subprocess.run(
        ["cargo", "build", "--offline", "--message-format=json"],
        cwd=directory,
        capture_output=True,
        text=True,
        stdin=subprocess.DEVNULL,
    )
    assert result.returncode == 0, result.stderr
    artifacts = [json.loads(line) for line in result.stdout.splitlines() if line.startswith("{")]
    return {
        item["target"]["name"]: item["fresh"]
        for item in artifacts
        if item.get("reason") == "compiler-artifact"
    }


def _seed_entries(tree: Path) -> tuple[str, ...]:
    return build_cache._seed_entries(tree / "target/debug")


def test_new_worktree_reuses_a_siblings_dependencies_and_rebuilds_its_own(cargo_repo, cloning):
    install_hook(cargo_repo)
    tree = _add_worktree(cargo_repo, "new tree")

    debug = tree / "target/debug"
    entries = sorted(path.name for path in debug.iterdir())
    assert entries == sorted(_seed_entries(cargo_repo))
    status = json.loads(_cache_command(tree, "status", "--json").stdout)
    assert status["seed"]["source"] == str(cargo_repo.resolve())
    assert _cargo_build(tree) == {"build-script-build": True, "scripted": True, "app": False}


def test_new_worktree_starts_cold_when_only_an_unbuilt_seed_is_idle(cargo_repo, cloning):
    install_hook(cargo_repo)
    unbuilt = _add_worktree(cargo_repo, "unbuilt")
    assert build_cache.seed_record(unbuilt)["source"] == str(cargo_repo.resolve())
    lock = cargo_repo / "target/debug/.cargo-build-lock"
    with lock.open() as held:
        fcntl.flock(held, fcntl.LOCK_EX)
        tree = _add_worktree(cargo_repo, "new tree")
        assert not (tree / "target").exists()
        assert not list((unbuilt / "target/debug").glob(".cargo*lock"))
    assert _cargo_build(tree)["scripted"] is False
    assert "seed     none" in _cache_command(tree, "status").stdout


def test_cargo_waits_while_a_built_source_is_locked(cargo_repo):
    debug = cargo_repo / "target/debug"
    (cargo_repo / "src/main.rs").write_text('fn main() { println!("rebuilt"); }\n')

    with ExitStack() as processes:
        with build_cache._quiescent(debug) as idle:
            assert idle
            cargo = processes.enter_context(
                subprocess.Popen(
                    ["cargo", "build", "--offline", "--color", "never"],
                    cwd=cargo_repo,
                    stdin=subprocess.DEVNULL,
                    stderr=subprocess.PIPE,
                    text=True,
                )
            )
            readable, _, _ = select.select([cargo.stderr], [], [], 60)
            assert readable, "Cargo did not report its lock wait"
            message = cargo.stderr.readline()
            assert "Blocking waiting for file lock" in message, message
        assert cargo.wait(timeout=60) == 0
    assert subprocess.check_output([debug / "app"], text=True).strip() == "rebuilt"


def test_a_failed_clone_leaves_no_target(cargo_repo, cloning, monkeypatch):
    tree = _add_worktree(cargo_repo, "new tree")
    clone = build_cache._clone

    def clone_then_fail(source, destination):
        clone(source, destination)
        raise OSError(errno.EXDEV, "cross-device", str(source))

    monkeypatch.setattr(build_cache, "_clone", clone_then_fail)
    with pytest.raises(OSError):
        build_cache.seed_target(tree)
    assert not (tree / "target").exists()
    assert _cargo_build(tree)["app"] is False


def test_an_existing_target_is_never_replaced(cargo_repo):
    tree = _add_worktree(cargo_repo, "new tree")
    (tree / "target").mkdir()
    assert build_cache.seed_target(tree) is None
    assert list((tree / "target").iterdir()) == []


def test_a_seeded_worktree_can_seed_others_after_cargo_builds_in_it(cargo_repo, cloning):
    install_hook(cargo_repo)
    built = _add_worktree(cargo_repo, "built")
    assert build_cache.seed_record(built)["source"] == str(cargo_repo.resolve())
    _cargo_build(built)
    with (cargo_repo / "target/debug/.cargo-build-lock").open() as held:
        fcntl.flock(held, fcntl.LOCK_EX)
        unbuilt = _add_worktree(cargo_repo, "unbuilt")
        assert build_cache.seed_record(unbuilt)["source"] == str(built.resolve())
        fingerprint = Path("target/debug", _seed_entries(built)[0])
        source_mtime = (built / fingerprint).stat().st_mtime_ns
        assert (unbuilt / fingerprint).stat().st_mtime_ns == source_mtime
        later = _add_worktree(cargo_repo, "later")
    assert json.loads(_cache_command(later, "status", "--json").stdout)["seed"]["source"] == str(
        built.resolve()
    )
    assert not list((unbuilt / "target/debug").glob(".cargo*lock"))


def test_a_target_in_cargos_unit_directory_layout_seeds_its_build_tree(tmp_path, cloning):
    # Cargo 1.100 keeps each unit's fingerprint and outputs under build/<package>/<hash>/.
    repo = tmp_path / "source"
    repo.mkdir()
    _git(repo, "init", "-q")
    _git(
        repo,
        "-c",
        "user.name=F",
        "-c",
        "user.email=f@example.com",
        "commit",
        "-qm",
        "i",
        "--allow-empty",
    )
    unit = repo / "target/debug/build/scripted/0123abcd/fingerprint"
    unit.mkdir(parents=True)
    (unit / "lib-scripted.json").write_text("{}")
    (repo / "target/debug/.cargo-build-lock").touch()
    (repo / "target/debug/scripted").touch()
    tree = _add_worktree(repo, "new tree")

    assert build_cache.seed_target(tree) == repo.resolve()
    assert [path.name for path in (tree / "target/debug").iterdir()] == ["build"]
    assert (tree / "target/debug/build/scripted/0123abcd/fingerprint/lib-scripted.json").is_file()
