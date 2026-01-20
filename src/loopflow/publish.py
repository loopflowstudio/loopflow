"""Publishing utilities for loopflow releases."""
import os
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path


class PublishError(Exception):
    """Publishing operation failed."""


# R2 configuration
R2_PUBLIC_URL = "https://downloads.loopflow.studio"


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
    try:
        major, minor, patch = map(int, parts)
    except ValueError:
        raise PublishError(f"Invalid version format: {version}")

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


def install_locally(repo_root: Path | None = None) -> tuple[bool, str]:
    """Install loopflow locally from the built wheel. Returns (success, output)."""
    cwd = repo_root or Path.cwd()
    dist_dir = cwd / "dist"

    # Find the wheel file (most recent)
    wheels = sorted(dist_dir.glob("loopflow-*.whl"))
    if not wheels:
        return False, "No wheel found in dist/"

    wheel_path = wheels[-1]
    result = _run(["uv", "tool", "install", "--force", str(wheel_path)])
    success = result.returncode == 0
    output = result.stdout + result.stderr
    return success, output


# DMG publishing functions


def build_dmg(repo_root: Path | None = None) -> tuple[bool, str]:
    """Build Maestro DMG. Returns (success, output)."""
    cwd = repo_root or Path.cwd()
    maestro_dir = cwd / "Maestro"

    if not maestro_dir.exists():
        return False, f"Maestro directory not found: {maestro_dir}"

    result = _run(["./dev", "release"], cwd=maestro_dir)
    success = result.returncode == 0
    output = result.stdout + result.stderr
    return success, output


def get_dmg_path(repo_root: Path | None = None) -> Path:
    """Get path to built DMG."""
    cwd = repo_root or Path.cwd()
    return cwd / "Maestro" / "dist" / "LoopflowMaestro.dmg"


def _get_r2_client():
    """Create boto3 S3 client configured for Cloudflare R2."""
    try:
        import boto3
    except ImportError:
        raise PublishError("boto3 required for DMG upload: pip install boto3")

    account_id = os.environ.get("R2_ACCOUNT_ID")
    access_key = os.environ.get("R2_ACCESS_KEY_ID")
    secret_key = os.environ.get("R2_SECRET_ACCESS_KEY")

    if not all([account_id, access_key, secret_key]):
        raise PublishError(
            "R2 credentials not set. Required environment variables:\n"
            "  R2_ACCOUNT_ID\n"
            "  R2_ACCESS_KEY_ID\n"
            "  R2_SECRET_ACCESS_KEY"
        )

    return boto3.client(
        "s3",
        endpoint_url=f"https://{account_id}.r2.cloudflarestorage.com",
        aws_access_key_id=access_key,
        aws_secret_access_key=secret_key,
    )


def upload_dmg(dmg_path: Path, version: str) -> tuple[bool, str]:
    """Upload DMG to Cloudflare R2. Returns (success, output)."""
    bucket = os.environ.get("R2_BUCKET_NAME", "loopflow-downloads")

    if not dmg_path.exists():
        return False, f"DMG not found: {dmg_path}"

    try:
        client = _get_r2_client()
    except PublishError as e:
        return False, str(e)

    versioned_key = f"LoopflowMaestro-{version}.dmg"
    latest_key = "LoopflowMaestro-latest.dmg"

    try:
        # Upload versioned file (cache forever)
        client.upload_file(
            str(dmg_path),
            bucket,
            versioned_key,
            ExtraArgs={
                "ContentType": "application/x-apple-diskimage",
                "CacheControl": "public, max-age=31536000, immutable",
            },
        )

        # Upload as latest (short cache)
        client.upload_file(
            str(dmg_path),
            bucket,
            latest_key,
            ExtraArgs={
                "ContentType": "application/x-apple-diskimage",
                "CacheControl": "public, max-age=60",
            },
        )

        return True, f"Uploaded to {R2_PUBLIC_URL}/{versioned_key} and {R2_PUBLIC_URL}/{latest_key}"
    except Exception as e:
        return False, f"Upload failed: {e}"


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
