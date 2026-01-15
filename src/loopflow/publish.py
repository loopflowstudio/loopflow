"""Publishing utilities for loopflow releases."""
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path


class PublishError(Exception):
    """Publishing operation failed."""


@dataclass
class PublishState:
    """Current state for publishing."""
    version: str
    on_main: bool
    main_synced: bool
    has_uncommitted: bool
    ready: bool
    message: str


def get_version() -> str:
    """Read current version from __init__.py."""
    init_path = Path(__file__).parent / "__init__.py"
    content = init_path.read_text()
    match = re.search(r'__version__\s*=\s*["\']([^"\']+)["\']', content)
    if not match:
        raise PublishError("Could not find __version__ in __init__.py")
    return match.group(1)


def bump_version(version: str, bump_type: str) -> str:
    """Calculate new version given bump type (patch/minor/major)."""
    parts = version.split(".")
    if len(parts) != 3:
        raise PublishError(f"Invalid version format: {version}")
    major, minor, patch = map(int, parts)

    if bump_type == "major":
        return f"{major + 1}.0.0"
    elif bump_type == "minor":
        return f"{major}.{minor + 1}.0"
    else:  # patch
        return f"{major}.{minor}.{patch + 1}"


def write_version(version: str) -> None:
    """Write version to __init__.py."""
    init_path = Path(__file__).parent / "__init__.py"
    init_path.write_text(f'__version__ = "{version}"\n')


def _run(cmd: list[str], cwd: Path | None = None) -> subprocess.CompletedProcess:
    """Run a command and return result."""
    return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)


def check_publish_ready(repo_root: Path | None = None) -> PublishState:
    """Check if repo is ready to publish (on main, synced with origin)."""
    cwd = repo_root or Path.cwd()

    # Check current branch
    result = _run(["git", "branch", "--show-current"], cwd)
    current_branch = result.stdout.strip()
    on_main = current_branch == "main"

    # Check if main is synced with origin
    main_synced = False
    if on_main:
        # Fetch latest
        _run(["git", "fetch", "origin", "main"], cwd)
        # Compare local and remote
        result = _run(["git", "rev-parse", "HEAD"], cwd)
        local_sha = result.stdout.strip()
        result = _run(["git", "rev-parse", "origin/main"], cwd)
        remote_sha = result.stdout.strip()
        main_synced = local_sha == remote_sha

    # Check for uncommitted changes
    result = _run(["git", "status", "--porcelain"], cwd)
    has_uncommitted = bool(result.stdout.strip())

    # Determine readiness
    if not on_main:
        message = f"Not on main branch (current: {current_branch}). Merge your changes to main first."
        ready = False
    elif not main_synced:
        message = "Local main is not synced with origin/main. Push or pull first."
        ready = False
    elif has_uncommitted:
        message = "Uncommitted changes in working directory."
        ready = False
    else:
        message = "Ready to publish."
        ready = True

    return PublishState(
        version=get_version(),
        on_main=on_main,
        main_synced=main_synced,
        has_uncommitted=has_uncommitted,
        ready=ready,
        message=message,
    )


def run_tests(repo_root: Path | None = None) -> tuple[bool, str]:
    """Run pytest. Returns (success, output)."""
    cwd = repo_root or Path.cwd()
    result = _run(["uv", "run", "pytest", "tests/"], cwd)
    success = result.returncode == 0
    output = result.stdout + result.stderr
    return success, output


def build_package(repo_root: Path | None = None) -> tuple[bool, str]:
    """Build package with uv. Returns (success, output)."""
    cwd = repo_root or Path.cwd()
    result = _run(["uv", "build"], cwd)
    success = result.returncode == 0
    output = result.stdout + result.stderr
    return success, output


def publish_package(repo_root: Path | None = None) -> tuple[bool, str]:
    """Publish package with uv. Returns (success, output)."""
    cwd = repo_root or Path.cwd()
    result = _run(["uv", "publish"], cwd)
    success = result.returncode == 0
    output = result.stdout + result.stderr
    return success, output


def install_locally() -> tuple[bool, str]:
    """Install loopflow locally with uv tool. Returns (success, output)."""
    result = _run(["uv", "tool", "install", "--force", "loopflow"])
    success = result.returncode == 0
    output = result.stdout + result.stderr
    return success, output


def main() -> int:
    """CLI entrypoint: check publish readiness."""
    state = check_publish_ready()
    print(f"Version: {state.version}")
    print(f"On main: {state.on_main}")
    print(f"Main synced: {state.main_synced}")
    print(f"Has uncommitted: {state.has_uncommitted}")
    print(f"Ready: {state.ready}")
    print(f"Message: {state.message}")
    return 0 if state.ready else 1


if __name__ == "__main__":
    sys.exit(main())
