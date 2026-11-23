"""Install the pinned Machine cache, seed and register worktrees, and inspect the serving daemon."""

import argparse
import ctypes
import fcntl
import json
import os
import shlex
import shutil
import socket
import struct
import subprocess
import sys
import tempfile
from contextlib import ExitStack, contextmanager
from pathlib import Path

import tomllib

REVISION = "ca81ec2a2566efc623692741440d99c710268841"
CACHE_ENV = {
    "SCCACHE_BASEDIRS_VERBATIM_ENV_DEPS": (
        "CARGO_MANIFEST_DIR,CARGO_MANIFEST_PATH,LOOPFLOW_BUILD_SOURCE_ROOT"
    ),
    "SCCACHE_CLIENT_SIDE": "1",
    "SCCACHE_ALLOW_INCREMENTAL": "1",
    "SCCACHE_MAX_FRAME_LENGTH": str(1024 * 1024 * 1024),
    "SCCACHE_IGNORE_SERVER_IO_ERROR": "1",
}


def _worktree_roots(repo: Path, *, env: dict[str, str] | None = None) -> set[str]:
    result = subprocess.run(
        ["git", "worktree", "list", "--porcelain", "-z"],
        cwd=repo,
        env=env,
        check=True,
        capture_output=True,
    )
    roots = set()
    for field in result.stdout.split(b"\0"):
        if field.startswith(b"worktree "):
            path = Path(os.fsdecode(field[len(b"worktree ") :]))
            if path.exists():
                roots.add(str(path.resolve()))
    return roots


def _outside_git_env() -> dict[str, str]:
    # Git hooks export the current repository even when cwd points elsewhere.
    return {
        k: v
        for k, v in os.environ.items()
        if k not in ("GIT_DIR", "GIT_COMMON_DIR", "GIT_WORK_TREE")
    }


def _configured_roots(settings: dict) -> list[str]:
    roots = settings.get("basedirs", []) if isinstance(settings, dict) else None
    if not isinstance(roots, list) or not all(isinstance(path, str) for path in roots):
        raise ValueError("cache configuration must be an object with a basedirs list of paths")
    return roots


def _replace_file(path: Path, content: str, mode: int | None = None) -> None:
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as output:
        temporary = Path(output.name)
        try:
            output.write(os.fsencode(content))
            output.flush()
            if mode is not None:
                temporary.chmod(mode)
            temporary.replace(path)
        finally:
            temporary.unlink(missing_ok=True)


@contextmanager
def _inventory_lock(config: Path):
    config.parent.mkdir(parents=True, exist_ok=True)
    with config.with_suffix(".lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        yield


def write_inventory(repo: Path, config: Path, settings: dict | None = None) -> None:
    with _inventory_lock(config):
        _write_inventory(repo, config, settings)


def _write_inventory(repo: Path, config: Path, settings: dict | None = None) -> None:
    roots = _worktree_roots(repo)
    original = config.read_text() if config.exists() else None
    saved = json.loads(original) if original is not None else {}
    # Keep other repositories using the same Machine cache. Rewrite each
    # repository's complete inventory so removed worktrees do not accumulate.
    other_env = _outside_git_env()
    for old in _configured_roots(saved):
        path = Path(old)
        if path.exists() and old not in roots:
            try:
                roots.update(_worktree_roots(path, env=other_env))
            except subprocess.CalledProcessError:
                continue
    selected = dict(saved if settings is None else settings)
    selected["basedirs"] = sorted(roots)
    encoded = json.dumps(selected, indent=2) + "\n"
    if original != encoded:
        _replace_file(config, encoded)


# --- Target seeding ---

# Cargo's freshness records, build-script outputs and compiled units. Incremental
# state is bound to its checkout's paths, and uplifted binaries would run another
# branch's code before the first build.
SEED_ENTRIES = (".fingerprint", "build", "deps")
CARGO_LOCKS = (".cargo-lock", ".cargo-build-lock", ".cargo-artifact-lock")
SEED_RECORD = ".loopflow-seed"


def _clone(source: Path, destination: Path) -> None:
    """Copy-on-write clone of a directory tree; raises instead of copying."""
    if sys.platform == "darwin":
        libc = ctypes.CDLL(None, use_errno=True)
        if libc.clonefile(os.fsencode(source), os.fsencode(destination), 0) != 0:
            code = ctypes.get_errno()
            raise OSError(code, os.strerror(code), str(source))
    else:
        subprocess.run(
            ["cp", "-a", "--reflink=always", str(source), str(destination)],
            check=True,
            capture_output=True,
        )


@contextmanager
def _quiescent(debug: Path):
    """Hold Cargo's locks, creating them when a seed has never been built."""
    with ExitStack() as locks:
        try:
            for name in CARGO_LOCKS:
                lock = locks.enter_context((debug / name).open("a"))
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError:
            yield False
        else:
            yield True


def _seed_sources(root: Path) -> list[Path]:
    """Sibling worktrees holding every seed entry, most recently built first."""
    candidates = [
        Path(path)
        for path in _worktree_roots(root)
        if Path(path) != root
        and all((Path(path) / "target/debug" / entry).is_dir() for entry in SEED_ENTRIES)
    ]
    # An unbuilt seed carries its source's mtime; at a tie the source wins.
    return sorted(
        candidates,
        key=lambda tree: (
            (tree / "target/debug/.fingerprint").stat().st_mtime_ns,
            not (tree / "target" / SEED_RECORD).exists(),
        ),
        reverse=True,
    )


def seed_target(repo: Path) -> Path | None:
    """Clone a sibling worktree's Cargo state into a checkout with no target."""
    root = Path(
        subprocess.check_output(
            ["git", "rev-parse", "--show-toplevel"], cwd=repo, text=True
        ).removesuffix("\n")
    ).resolve()
    target = root / "target"
    if os.path.lexists(target):
        return None
    for source in _seed_sources(root):
        debug = source / "target/debug"
        with _quiescent(debug) as idle:
            if not idle:
                continue
            target.mkdir()
            staging = target / "debug.seeding"
            try:
                staging.mkdir()
                for entry in SEED_ENTRIES:
                    _clone(debug / entry, staging / entry)
                # A cloned directory gets a fresh mtime; keep the source's so an
                # unbuilt seed never outranks the sibling that last built.
                built = (debug / ".fingerprint").stat()
                os.utime(staging / ".fingerprint", ns=(built.st_atime_ns, built.st_mtime_ns))
                # Atomic: a Cargo started meanwhile sees no target or a whole one.
                staging.rename(target / "debug")
            except (OSError, subprocess.CalledProcessError):
                shutil.rmtree(staging, ignore_errors=True)
                try:
                    target.rmdir()
                except OSError:
                    pass  # A Cargo started meanwhile owns it now.
                raise
        commit = subprocess.run(
            ["git", "rev-parse", "--short=10", "HEAD"],
            cwd=source,
            env=_outside_git_env(),
            capture_output=True,
            text=True,
        ).stdout.strip()
        record = {"source": str(source), "commit": commit or None}
        (target / SEED_RECORD).write_text(json.dumps(record) + "\n")
        return source
    return None


def _seed_record(root: Path) -> dict | None:
    try:
        return json.loads((root / "target" / SEED_RECORD).read_text())
    except (FileNotFoundError, json.JSONDecodeError):
        return None


def _default_configuration() -> dict:
    if sys.platform == "darwin":
        candidates = [
            Path.home() / "Library/Application Support/Mozilla.sccache/config",
            Path.home() / "Library/Preferences/Mozilla.sccache/config",
        ]
    else:
        directory = Path(os.environ.get("XDG_CONFIG_HOME", str(Path.home() / ".config")))
        candidates = [directory / "sccache/config"]
    for source in candidates:
        if source.exists():
            return tomllib.loads(source.read_text())
    return {}


def inventory_path() -> Path:
    return (
        Path(os.environ.get("XDG_CACHE_HOME", str(Path.home() / ".cache")))
        / "loopflow/sccache.json"
    )


def cache_environment(revision: str = REVISION) -> dict[str, str]:
    config = inventory_path()
    return {
        **CACHE_ENV,
        "SCCACHE_CONF": str(config),
        "SCCACHE_SERVER_UDS": str(config.parent / f"sccache-{revision[:8]}.sock"),
    }


def cargo_configuration() -> Path:
    directory = Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo")))
    # Cargo gives the extensionless configuration precedence when both exist.
    legacy = directory / "config"
    return legacy if legacy.is_file() else directory / "config.toml"


def configure_cargo(path: Path, wrapper: Path, env: dict[str, str]) -> None:
    import tomlkit  # Only the installer edits TOML. The installed Git hook uses the stdlib.

    original = path.read_text() if path.exists() else ""
    config = tomlkit.loads(original)
    if "build" not in config:
        config["build"] = tomlkit.table()
    config["build"]["rustc-wrapper"] = str(wrapper)
    if "env" not in config:
        config["env"] = tomlkit.table()
    for key, value in env.items():
        config["env"][key] = value
    encoded = tomlkit.dumps(config)
    if encoded != original:
        path.parent.mkdir(parents=True, exist_ok=True)
        _replace_file(path, encoded)


def install_hook(repo: Path) -> None:
    hook = Path(
        subprocess.check_output(
            ["git", "rev-parse", "--git-path", "hooks/post-checkout"], cwd=repo, text=True
        ).removesuffix("\n")
    )
    if not hook.is_absolute():
        hook = repo / hook
    begin, end = "# loopflow build cache begin", "# loopflow build cache end"
    # Worktrees can check out revisions that predate this installer.
    program = hook.parent / "loopflow-build-cache.py"
    hook.parent.mkdir(parents=True, exist_ok=True)
    source = Path(__file__).read_text()
    if not program.exists() or source != program.read_text():
        _replace_file(program, source)
    command = shlex.join([str(Path(sys.executable).resolve()), str(program.resolve())])
    quiet = "</dev/null >/dev/null 2>&1 || :"
    # A previous HEAD of all zeros (SHA-1 or SHA-256) means a new worktree.
    block = (
        f'{begin}\nif [ "${{3:-}}" = 1 ]; then\n'
        f'    case "${{1:-x}}" in *[!0]*) ;; *) {command} seed {quiet} ;; esac\n'
        f"    {command} register {quiet}\nfi\n{end}\n"
    )
    saved = hook.with_name("post-checkout.before-loopflow")
    original = os.fsdecode(hook.read_bytes()) if hook.is_file() else ""
    # Shell hooks can keep their original $0 and location. Other interpreters,
    # binaries and symlink targets must not receive shell source.
    shell_hook = original.startswith(("#!/bin/sh\n", "#!/bin/bash\n", "#!/usr/bin/env bash\n"))
    if hook.is_symlink() or (hook.exists() and not (shell_hook and os.access(hook, os.X_OK))):
        if os.path.lexists(saved):
            raise ValueError(f"cannot keep the existing hook: {saved} already exists")
        hook.rename(saved)
        original = ""
    body = original or "#!/bin/sh\n"
    if not original and os.path.lexists(saved):
        body += 'if [ -x "$0.before-loopflow" ]; then exec "$0.before-loopflow" "$@"; fi\n'
    if begin in body:
        start = body.index(begin)
        finish = body.index(end, start) + len(end)
        body = body[:start] + body[finish:].removeprefix("\n")
    shebang, _, rest = body.partition("\n")
    updated = shebang + "\n" + block + rest
    if updated != original:
        _replace_file(hook, updated, hook.stat().st_mode if hook.exists() else 0o755)


def _is_revision(value: str) -> bool:
    return len(value) == 40 and all(c in "0123456789abcdef" for c in value)


def _pin_directory(revision: str) -> Path:
    return Path.home() / ".local/share/loopflow/sccache" / revision


def install(repo: Path, revision: str) -> None:
    if not _is_revision(revision):
        raise ValueError("revision must be a full lowercase Git SHA")
    prefix = _pin_directory(revision)
    wrapper = prefix / "bin/sccache"
    if not wrapper.exists():
        env = {
            **os.environ,
            "RUSTC_WRAPPER": "",
            "RUSTC_WORKSPACE_WRAPPER": "",
            "SCCACHE_DISABLE": "1",
        }
        subprocess.run(
            [
                "cargo",
                "install",
                "sccache",
                "--git",
                "https://github.com/mozilla/sccache",
                "--rev",
                revision,
                "--locked",
                "--no-default-features",
                "--root",
                str(prefix),
            ],
            env=env,
            check=True,
            stdin=subprocess.DEVNULL,
        )
    subprocess.run([str(wrapper), "--version"], check=True)
    write_inventory(repo, inventory_path(), _default_configuration())
    install_hook(repo)
    configure_cargo(cargo_configuration(), wrapper, cache_environment(revision))
    # A daemon already serving other repositories must load this one too.
    register(repo)


def configured_cache() -> tuple[Path | None, dict[str, str]]:
    path = cargo_configuration()
    config = tomllib.loads(path.read_text()) if path.exists() else {}
    wrapper = config.get("build", {}).get("rustc-wrapper")
    env = {k: v for k, v in os.environ.items() if k.startswith("SCCACHE_")}
    for key, setting in config.get("env", {}).items():
        if not key.startswith("SCCACHE_"):
            continue
        entry = setting if isinstance(setting, dict) else {"value": setting}
        value = str(entry["value"])
        if entry.get("relative"):
            value = str(path.parent.parent / value)
        if entry.get("force") or key not in env:
            env[key] = value
    return Path(wrapper) if wrapper else None, env


def daemon_pid(address: str) -> int | None:
    # A draining daemon can still appear in lsof under the replacement's socket name.
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
        client.settimeout(5)
        try:
            client.connect(address)
        except (FileNotFoundError, ConnectionRefusedError):
            return None
        if sys.platform == "darwin":
            return client.getsockopt(0, 2)  # SOL_LOCAL, LOCAL_PEERPID from sys/un.h
        credentials = client.getsockopt(
            socket.SOL_SOCKET, socket.SO_PEERCRED, struct.calcsize("iII")
        )
        return struct.unpack("iII", credentials)[0]


def _manages_roots(env: dict[str, str]) -> bool:
    inventory = inventory_path()
    return (
        Path(env.get("SCCACHE_CONF", str(inventory))) == inventory and "SCCACHE_BASEDIRS" not in env
    )


def cache_status(repo: Path, wrapper: Path | None, env: dict[str, str]) -> dict:
    # A revision override is still a pin when its full SHA is carried in the stable path.
    pinned = bool(
        wrapper
        and wrapper.is_file()
        and os.access(wrapper, os.X_OK)
        and _is_revision(revision := wrapper.parent.parent.name)
        and wrapper == _pin_directory(revision) / "bin/sccache"
    )
    config = Path(env.get("SCCACHE_CONF", str(inventory_path())))
    managed = _manages_roots(env)
    if "SCCACHE_BASEDIRS" in env:
        listed = [root for root in env["SCCACHE_BASEDIRS"].split(os.pathsep) if root]
    elif config.exists():
        parse = json.loads if managed or config.suffix == ".json" else tomllib.loads
        listed = _configured_roots(parse(config.read_text()))
    else:
        listed = []
    listed = [str(Path(path)) for path in listed]
    root = subprocess.check_output(
        ["git", "rev-parse", "--show-toplevel"], cwd=repo, text=True
    ).removesuffix("\n")
    root = str(Path(root).resolve())
    result = {
        "wrapper": str(wrapper) if wrapper else None,
        "pinned": pinned,
        "listed_roots": listed,
        "managed_roots": managed,
        "root": root,
        "daemon": None,
        "loaded_roots": [],
        "stats": None,
        "seed": _seed_record(Path(root)),
        "configured_idle_timeout_seconds": int(env["SCCACHE_IDLE_TIMEOUT"])
        if "SCCACHE_IDLE_TIMEOUT" in env
        else None,
    }
    if not pinned:
        return {**result, "exit_code": 1, "diagnostic": "pinned wrapper unavailable; run install"}
    address = env.get("SCCACHE_SERVER_UDS")
    if address and (pid := daemon_pid(address)):
        executable = (
            os.readlink(f"/proc/{pid}/exe")
            if sys.platform.startswith("linux")
            else subprocess.run(
                ["ps", "-p", str(pid), "-o", "comm="], capture_output=True, text=True
            ).stdout.strip()
        )
        uptime = subprocess.run(
            ["ps", "-p", str(pid), "-o", "etime="], capture_output=True, text=True
        ).stdout.strip()
        result["daemon"] = {"pid": pid, "executable": executable, "uptime": uptime}
        if executable != str(wrapper):
            return {
                **result,
                "exit_code": 1,
                "diagnostic": "check SCCACHE_SERVER_UDS; socket served by another executable",
            }
        # --show-stats answers empty statistics if no server exists; never starts one.
        raw = subprocess.check_output(
            [str(wrapper), "--show-stats", "--stats-format", "json"],
            env={**os.environ, **env},
            text=True,
            timeout=10,
        )
        stats = json.loads(raw)
        result["loaded_roots"] = [str(Path(root)) for root in stats["basedirs"]]
        result["stats"] = stats
    if root not in listed:
        source = "SCCACHE_BASEDIRS" if "SCCACHE_BASEDIRS" in env else str(config)
        action = "run register" if managed else f"add it to {source}"
        note = f"this checkout is missing from the root list; {action}"
    elif result["daemon"] and root not in result["loaded_roots"]:
        can_register = managed and env.get("SCCACHE_CLIENT_SIDE") == "1"
        action = "run register" if can_register else "restart your daemon with the configured roots"
        note = f"the serving daemon lacks this checkout; {action}"
    else:
        note = (
            "this checkout loaded"
            if result["daemon"]
            else "this checkout is listed for the next daemon start"
        )
        return {**result, "exit_code": 0, "diagnostic": note}
    return {**result, "exit_code": 3, "diagnostic": note}


def register(repo: Path) -> None:
    wrapper, env = configured_cache()
    if not _manages_roots(env):
        return
    config = inventory_path()
    # Concurrent checkouts must observe the daemon after the preceding registration.
    with _inventory_lock(config):
        _write_inventory(repo, config)
        state = cache_status(repo, wrapper, env)
        if not state["exit_code"]:
            return
        if state["exit_code"] == 1 or state["root"] not in state["listed_roots"]:
            raise ValueError(state["diagnostic"])
        if env.get("SCCACHE_CLIENT_SIDE") != "1":
            raise ValueError(
                "root listed for the next start; only client-side registration can stop the daemon"
            )
        subprocess.run(
            [str(wrapper), "--stop-server"],
            env={**os.environ, **env},
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            check=True,
            timeout=15,
        )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    installer = commands.add_parser("install")
    installer.add_argument("--revision", default=REVISION)
    commands.add_parser("register")
    commands.add_parser("seed")
    status = commands.add_parser("status")
    status.add_argument("--json", action="store_true")
    args = parser.parse_args()
    try:
        if args.command == "install":
            install(Path.cwd(), args.revision)
        elif args.command == "register":
            register(Path.cwd())
        elif args.command == "seed":
            seed_target(Path.cwd())
        else:
            result = cache_status(Path.cwd(), *configured_cache())
            if args.json:
                print(json.dumps(result, indent=2))
            else:
                daemon, stats = result["daemon"], result["stats"]
                timeout = result["configured_idle_timeout_seconds"]
                idle = f"{timeout}s" if timeout is not None else "sccache default (600s)"
                print(
                    f"wrapper  {result['wrapper']} "
                    f"({'pinned' if result['pinned'] else 'not pinned'})"
                )
                print(
                    f"daemon   pid {daemon['pid']}, {daemon['executable']}, up {daemon['uptime']}; "
                    f"configured idle timeout {idle} (next start)"
                    if daemon
                    else f"daemon   not running; next build loads the list; idle timeout {idle}"
                )
                listed = set(result["listed_roots"])
                loaded = listed.intersection(result["loaded_roots"])
                print(f"roots    {len(loaded)} of {len(listed)} loaded; {result['diagnostic']}")
                seed = result["seed"]
                print(
                    f"seed     from {os.path.relpath(seed['source'], result['root'])}"
                    f" at {seed['commit'] or 'unknown commit'}"
                    if seed
                    else "seed     none"
                )
                if stats:
                    counts = stats["stats"]
                    hits = counts.get("cache_hits", {}).get("counts", {}).get("Rust", 0)
                    misses = counts.get("cache_misses", {}).get("counts", {}).get("Rust", 0)
                    print(f"Rust     {hits} hits, {misses} misses")
                else:
                    print("Rust     unmeasured")
                # A daemon reports no size until its first cache write.
                print(
                    f"cache    {stats['cache_size'] / 1024**3:.1f} GiB "
                    f"of {stats['max_cache_size'] / 1024**3:.1f} GiB"
                    if stats and stats["cache_size"] is not None
                    else "cache    unmeasured"
                )
            raise SystemExit(result["exit_code"])
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"build cache: {error}", file=sys.stderr)
        raise SystemExit(1) from error


if __name__ == "__main__":
    main()
