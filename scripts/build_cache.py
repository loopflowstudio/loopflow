"""Refresh derived worktree roots before a developer build; never restart sccache."""

import fcntl
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

import tomllib

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


def refresh(repo: Path, config: Path, settings: dict | None = None) -> dict[str, str]:
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
    return {
        "SCCACHE_CONF": str(config),
        "SCCACHE_BASEDIRS_VERBATIM_ENV_DEPS": VERBATIM_ENV_DEPS,
        "SCCACHE_MAX_FRAME_LENGTH": str(1024 * 1024 * 1024),
        "SCCACHE_IGNORE_SERVER_IO_ERROR": "1",
    }


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


def main() -> None:
    if len(sys.argv) < 2:
        raise SystemExit("usage: build_cache.py <build command> [args...]")
    env = os.environ.copy()
    config = Path(env.get("XDG_CACHE_HOME", str(Path.home() / ".cache")))
    config = config / "loopflow" / "sccache.json"
    # An explicitly selected configuration belongs to its caller. Ordinary
    # Cargo also remains usable with the Machine's own configuration.
    if "SCCACHE_CONF" not in env and env.get("SCCACHE_DISABLE") != "1":
        try:
            configured = env.get("SCCACHE_BASEDIRS_VERBATIM_ENV_DEPS", "")
            env.update(refresh(Path.cwd(), config, _default_configuration()))
            if configured:
                env["SCCACHE_BASEDIRS_VERBATIM_ENV_DEPS"] += "," + configured
        except (OSError, ValueError, subprocess.CalledProcessError) as error:
            print(f"build cache roots: {error}; using the existing configuration", file=sys.stderr)
    os.execvpe(sys.argv[1], sys.argv[1:], env)


if __name__ == "__main__":
    main()
