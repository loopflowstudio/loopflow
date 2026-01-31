"""Publishing utilities for loopflow releases."""

import json
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


def check_ci_passed(repo_root: Path | None = None) -> tuple[bool, str]:
    """Check if CI passed for the current HEAD commit. Returns (success, message)."""
    cwd = repo_root or Path.cwd()

    # Get current commit SHA
    result = _run(["git", "rev-parse", "HEAD"], cwd)
    if result.returncode != 0:
        return False, "Failed to get current commit"
    commit_sha = result.stdout.strip()

    # Get CI status for this commit
    result = _run(
        [
            "gh",
            "run",
            "list",
            "--commit",
            commit_sha,
            "--json",
            "status,conclusion,workflowName",
        ],
        cwd,
    )

    if result.returncode != 0:
        return False, f"Failed to get CI status: {result.stderr}"

    try:
        runs = json.loads(result.stdout)
    except json.JSONDecodeError:
        return False, f"Invalid JSON from gh run list: {result.stdout}"

    if not runs:
        return (
            False,
            f"No CI runs found for commit {commit_sha[:8]}. Push to main to trigger CI.",
        )

    # Check status of each workflow
    statuses = []
    all_passed = True
    any_running = False

    seen_workflows = set()
    for run in runs:
        workflow = run.get("workflowName", "unknown")
        if workflow in seen_workflows:
            continue
        seen_workflows.add(workflow)

        status = run.get("status")
        conclusion = run.get("conclusion")

        if status in ("queued", "in_progress", "pending", "waiting"):
            any_running = True
            statuses.append(f"{workflow}: {status}")
        elif conclusion == "success":
            statuses.append(f"{workflow}: ✓")
        else:
            all_passed = False
            statuses.append(f"{workflow}: ✗ ({conclusion or status})")

    status_str = ", ".join(statuses)

    if any_running:
        return False, f"CI still running: {status_str}"

    if not all_passed:
        return False, f"CI failed: {status_str}"

    return True, f"CI passed: {status_str}"


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
        message = f"Not on main branch (current: {current_branch}). "
        message += "Merge your changes to main first."
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


def build_rust_extension(repo_root: Path | None = None) -> tuple[bool, str]:
    """Build loopflow-engine Python extension with maturin. Returns (success, output)."""
    cwd = repo_root or Path.cwd()
    manifest_path = cwd / "rust" / "loopflow-engine" / "Cargo.toml"

    if not manifest_path.exists():
        return False, f"Cargo.toml not found: {manifest_path}"

    result = _run(
        [
            "uv",
            "run",
            "maturin",
            "develop",
            "--manifest-path",
            str(manifest_path),
            "--features",
            "python",
        ],
        cwd,
    )
    success = result.returncode == 0
    output = result.stdout + result.stderr
    return success, output


def build_package(repo_root: Path | None = None) -> tuple[bool, str]:
    """Build package with uv. Returns (success, output)."""
    cwd = repo_root or Path.cwd()
    dist_dir = cwd / "dist"

    # Clean old artifacts (keep .gitignore)
    if dist_dir.exists():
        for f in dist_dir.iterdir():
            if f.name != ".gitignore":
                f.unlink()

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


def restart_daemon() -> tuple[bool, str]:
    """Restart lfd daemon. Returns (success, message)."""
    from loopflow.lfd.daemon.launchd import install, is_running

    was_running = is_running()
    if install():
        if was_running:
            return True, "Daemon restarted"
        else:
            return True, "Daemon started"
    return False, "Failed to start daemon"


# DMG publishing functions


def build_dmg(repo_root: Path | None = None) -> tuple[bool, str]:
    """Build Concerto DMG. Returns (success, output)."""
    cwd = repo_root or Path.cwd()
    swift_dir = cwd / "swift"

    if not swift_dir.exists():
        return False, f"swift directory not found: {swift_dir}"

    result = _run(["./dev", "release"], cwd=swift_dir)
    success = result.returncode == 0
    output = result.stdout + result.stderr
    return success, output


def get_dmg_path(repo_root: Path | None = None) -> Path:
    """Get path to built DMG."""
    cwd = repo_root or Path.cwd()
    return cwd / "swift" / "dist" / "LoopflowConcerto.dmg"


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

    versioned_key = f"LoopflowConcerto-{version}.dmg"
    latest_key = "LoopflowConcerto-latest.dmg"

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

        return (
            True,
            f"Uploaded to {R2_PUBLIC_URL}/{versioned_key} and {R2_PUBLIC_URL}/{latest_key}",
        )
    except Exception as e:
        return False, f"Upload failed: {e}"


def upload_file(
    file_path: Path,
    key: str,
    content_type: str = "application/octet-stream",
    bucket: str | None = None,
) -> tuple[bool, str]:
    """Upload file to R2 bucket. Returns (success, output)."""
    bucket = bucket or os.environ.get("R2_BUCKET_NAME", "downloads")

    if not file_path.exists():
        return False, f"File not found: {file_path}"

    try:
        client = _get_r2_client()
    except PublishError as e:
        return False, str(e)

    try:
        client.upload_file(
            str(file_path),
            bucket,
            key,
            ExtraArgs={
                "ContentType": content_type,
                "CacheControl": "public, max-age=31536000, immutable",
            },
        )
        url = f"https://bin.loopflow.studio/{key}" if bucket == "bin" else f"{R2_PUBLIC_URL}/{key}"
        return True, f"Uploaded to {url}"
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
