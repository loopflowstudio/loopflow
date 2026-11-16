"""Install the pinned Machine sccache, refresh its roots, report its status."""

import argparse
import fcntl
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

import tomlkit

REVISION = "ca81ec2a2566efc623692741440d99c710268841"
CACHE_ENV = {
    "SCCACHE_CLIENT_SIDE": "1",
    "SCCACHE_ALLOW_INCREMENTAL": "1",
    "SCCACHE_MAX_FRAME_LENGTH": str(1024 * 1024 * 1024),
    "SCCACHE_IGNORE_SERVER_IO_ERROR": "1",
}
VERBATIM_ENV_DEPS = "CARGO_MANIFEST_DIR,CARGO_MANIFEST_PATH,LOOPFLOW_BUILD_SOURCE_ROOT"


def worktree_roots(repo: Path) -> set[str]:
    result = subprocess.run(
        ["git", "worktree", "list", "--porcelain", "-z"],
        cwd=repo,
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


def refresh(repo: Path, config: Path, settings: dict | None = None) -> None:
    """Write the next daemon's configuration, leaving the running daemon alone."""
    roots = worktree_roots(repo)
    config.parent.mkdir(parents=True, exist_ok=True)
    with config.with_suffix(".lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        saved = json.loads(config.read_text()) if config.exists() else {}
        if not isinstance(saved, dict) or not isinstance(saved.get("basedirs", []), list):
            raise ValueError("cache configuration must be an object with a basedirs list")
        # Keep other repositories using the same Machine cache. Refresh each
        # repository's complete inventory so removed worktrees do not accumulate.
        for old in saved.get("basedirs", []):
            path = Path(old)
            if path.exists() and old not in roots:
                try:
                    roots.update(worktree_roots(path))
                except subprocess.CalledProcessError:
                    continue
        selected = dict(saved if settings is None else settings)
        selected["basedirs"] = sorted(roots)
        encoded = json.dumps(selected, indent=2) + "\n"
        if not config.exists() or config.read_text() != encoded:
            with tempfile.NamedTemporaryFile(mode="w", dir=config.parent, delete=False) as output:
                temporary = Path(output.name)
                try:
                    output.write(encoded)
                    output.close()
                    temporary.replace(config)
                finally:
                    temporary.unlink(missing_ok=True)


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
            return tomlkit.loads(source.read_text()).unwrap()
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
        "SCCACHE_BASEDIRS_VERBATIM_ENV_DEPS": VERBATIM_ENV_DEPS,
    }


def cargo_configuration() -> Path:
    return Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo"))) / "config.toml"


def configure_cargo(path: Path, wrapper: Path, env: dict[str, str]) -> None:
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
        with tempfile.NamedTemporaryFile(mode="w", dir=path.parent, delete=False) as output:
            temporary = Path(output.name)
            try:
                output.write(encoded)
                output.close()
                temporary.replace(path)
            finally:
                temporary.unlink(missing_ok=True)


def install_hook(repo: Path) -> None:
    hook = Path(
        subprocess.check_output(
            ["git", "rev-parse", "--git-path", "hooks/post-checkout"], cwd=repo, text=True
        ).removesuffix("\n")
    )
    if not hook.is_absolute():
        hook = repo / hook
    original = hook.read_text() if hook.exists() else "#!/bin/sh\n"
    begin, end = "# loopflow build cache begin", "# loopflow build cache end"
    block = (
        begin
        + "\n"
        + (
            "(if [ -f scripts/build_cache.py ]; then\n"
            "    uv run python scripts/build_cache.py refresh </dev/null >/dev/null 2>&1 || :\n"
            "fi) >/dev/null 2>&1 &\n"
            ": # cache registration cannot fail checkout\n"
        )
        + end
        + "\n"
    )
    if begin in original:
        start = original.index(begin)
        finish = original.index(end, start) + len(end)
        if original[finish : finish + 1] == "\n":
            finish += 1
        updated = original[:start] + block + original[finish:]
    else:
        updated = original + ("" if original.endswith("\n") else "\n") + block
    if updated != original or not hook.exists():
        hook.parent.mkdir(parents=True, exist_ok=True)
        hook.write_text(updated)
    hook.chmod(hook.stat().st_mode | 0o111)


def install(repo: Path, revision: str) -> None:
    if len(revision) != 40 or any(c not in "0123456789abcdef" for c in revision):
        raise ValueError("revision must be a full lowercase Git SHA")
    prefix = Path.home() / ".local/share/loopflow/sccache" / revision
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
    refresh(repo, inventory_path(), _default_configuration())
    install_hook(repo)
    configure_cargo(cargo_configuration(), wrapper, cache_environment(revision))


def configured_cache() -> tuple[Path | None, dict[str, str]]:
    path = cargo_configuration()
    config = tomlkit.loads(path.read_text()).unwrap() if path.exists() else {}
    wrapper = config.get("build", {}).get("rustc-wrapper")
    env = {}
    for key, value in config.get("env", {}).items():
        if key.startswith("SCCACHE_"):
            env[key] = str(value["value"] if isinstance(value, dict) else value)
    return Path(wrapper) if wrapper else None, env


def cache_status(repo: Path) -> dict:
    wrapper, env = configured_cache()
    expected = Path.home() / ".local/share/loopflow/sccache" / REVISION / "bin/sccache"
    # A revision override is still a pin when its full SHA is carried in the stable path.
    pinned = bool(
        wrapper
        and wrapper.is_file()
        and os.access(wrapper, os.X_OK)
        and wrapper.parent.name == "bin"
        and wrapper.parent.parent.parent == expected.parent.parent.parent
        and len(wrapper.parent.parent.name) == 40
        and all(c in "0123456789abcdef" for c in wrapper.parent.parent.name)
    )
    config = Path(env.get("SCCACHE_CONF", str(inventory_path())))
    refresh(repo, config)
    listed = json.loads(config.read_text())["basedirs"]
    root = subprocess.check_output(
        ["git", "rev-parse", "--show-toplevel"], cwd=repo, text=True
    ).removesuffix("\n")
    root = str(Path(root).resolve())
    result = {
        "wrapper": str(wrapper) if wrapper else None,
        "pinned": pinned,
        "listed_roots": listed,
        "root": root,
        "daemon": None,
        "loaded_roots": [],
        "stats": None,
        "configured_idle_timeout_seconds": int(env["SCCACHE_IDLE_TIMEOUT"])
        if "SCCACHE_IDLE_TIMEOUT" in env
        else None,
    }
    if not pinned:
        return {**result, "exit_code": 1}
    socket = env.get("SCCACHE_SERVER_UDS")
    if socket and Path(socket).exists():
        owners = subprocess.run(
            ["lsof", "-nP", "-a", "-U", "-c", "sccache", "-t", socket],
            capture_output=True,
            text=True,
            timeout=10,
        )
        for pid in set(owners.stdout.split()):
            command = subprocess.run(
                ["ps", "-p", pid, "-o", "comm="], capture_output=True, text=True
            ).stdout.strip()
            if command == str(wrapper):
                uptime = subprocess.run(
                    ["ps", "-p", pid, "-o", "etime="], capture_output=True, text=True
                ).stdout.strip()
                result["daemon"] = {"pid": int(pid), "executable": command, "uptime": uptime}
                break
    if result["daemon"]:
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
    loaded = result["loaded_roots"] if result["daemon"] else listed
    return {**result, "exit_code": 0 if root in loaded else 3}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    installer = commands.add_parser("install")
    installer.add_argument("--revision", default=REVISION)
    commands.add_parser("refresh")
    status = commands.add_parser("status")
    status.add_argument("--json", action="store_true")
    args = parser.parse_args()
    try:
        if args.command == "install":
            install(Path.cwd(), args.revision)
        elif args.command == "refresh":
            refresh(Path.cwd(), inventory_path(), _default_configuration())
        else:
            result = cache_status(Path.cwd())
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
                    f"daemon   pid {daemon['pid']}, same executable, up {daemon['uptime']}; "
                    f"configured idle timeout {idle} (next start)"
                    if daemon
                    else f"daemon   not running; next build loads the list; idle timeout {idle}"
                )
                print(
                    f"roots    {len(result['loaded_roots'])} "
                    f"of {len(result['listed_roots'])} loaded; "
                    + (
                        "this checkout loaded"
                        if daemon and result["exit_code"] == 0
                        else "this checkout loads at the daemon's next idle exit"
                        if daemon
                        else "this checkout listed"
                        if result["exit_code"] == 0
                        else "checkout not listed"
                    )
                )
                counts = stats["stats"] if stats else {}
                print(
                    f"Rust     "
                    f"{counts.get('cache_hits', {}).get('counts', {}).get('Rust', 0)} hits, "
                    f"{counts.get('cache_misses', {}).get('counts', {}).get('Rust', 0)} misses"
                )
                print(
                    f"cache    {stats['cache_size'] / 1024**3:.1f} GiB "
                    f"of {stats['max_cache_size'] / 1024**3:.1f} GiB"
                    if stats
                    else "cache    unmeasured (daemon not running)"
                )
            raise SystemExit(result["exit_code"])
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"build cache: {error}", file=sys.stderr)
        raise SystemExit(1) from error


if __name__ == "__main__":
    main()
