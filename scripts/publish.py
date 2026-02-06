#!/usr/bin/env python3
"""Publish loopflow to PyPI and Concerto DMG to R2.

Local development:
  python scripts/publish.py --local       # Build and install locally

Release flow (full):
  python scripts/publish.py patch         # Version bump, screenshots, DMG, tag, push

Single-shot modes:
  python scripts/publish.py --dmg-only        # Just upload existing DMG
  python scripts/publish.py --screenshots-only # Just generate screenshots
  python scripts/publish.py --tag-only        # Just bump, tag, push (skip screenshots/DMG)
"""

import argparse
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).parent.parent
VERSION_FILE = ROOT / "VERSION"
R2_PUBLIC_URL = "https://downloads.loopflow.studio"


# --- Version management ---


def get_version() -> str:
    """Read version from VERSION file."""
    if VERSION_FILE.exists():
        return VERSION_FILE.read_text().strip()
    # Fallback to pyproject.toml
    pyproject = ROOT / "pyproject.toml"
    content = pyproject.read_text()
    match = re.search(r'^version = "([^"]+)"', content, re.MULTILINE)
    if match:
        return match.group(1)
    raise RuntimeError("Could not find version")


def check_versions() -> tuple[bool, str]:
    """Verify VERSION, Cargo.toml, and pyproject.toml all agree."""
    versions: dict[str, str] = {}

    if VERSION_FILE.exists():
        versions["VERSION"] = VERSION_FILE.read_text().strip()

    cargo = ROOT / "Cargo.toml"
    if cargo.exists():
        m = re.search(r'^version = "([^"]+)"', cargo.read_text(), re.MULTILINE)
        if m:
            versions["Cargo.toml"] = m.group(1)

    pyproject = ROOT / "pyproject.toml"
    if pyproject.exists():
        m = re.search(r'^version = "([^"]+)"', pyproject.read_text(), re.MULTILINE)
        if m:
            versions["pyproject.toml"] = m.group(1)

    unique = set(versions.values())
    if len(unique) != 1:
        lines = [f"  {f}: {v}" for f, v in versions.items()]
        return False, "Version mismatch:\n" + "\n".join(lines)
    return True, list(unique)[0]


def bump_version(version: str, bump_type: str) -> str:
    """Calculate new version given bump type (patch/minor/major)."""
    parts = version.split(".")
    if len(parts) != 3:
        raise ValueError(f"Invalid version format: {version}")
    major, minor, patch = map(int, parts)

    if bump_type == "major":
        return f"{major + 1}.0.0"
    elif bump_type == "minor":
        return f"{major}.{minor + 1}.0"
    else:  # patch
        return f"{major}.{minor}.{patch + 1}"


def write_version(version: str) -> None:
    """Write version to all version files."""
    # VERSION file
    VERSION_FILE.write_text(version + "\n")

    # pyproject.toml
    pyproject = ROOT / "pyproject.toml"
    content = pyproject.read_text()
    content = re.sub(r'^version = "[^"]+"', f'version = "{version}"', content, flags=re.MULTILINE)
    pyproject.write_text(content)

    # Cargo.toml (workspace)
    cargo_toml = ROOT / "Cargo.toml"
    content = cargo_toml.read_text()
    content = re.sub(r'^version = "[^"]+"', f'version = "{version}"', content, flags=re.MULTILINE)
    cargo_toml.write_text(content)


# --- Build and install ---


def build_local() -> tuple[bool, str]:
    """Build wheel with maturin. Returns (success, output)."""
    result = subprocess.run(
        [
            "uv",
            "run",
            "maturin",
            "build",
            "--release",
            "--manifest-path",
            str(ROOT / "rust" / "loopflow-py" / "Cargo.toml"),
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    return result.returncode == 0, result.stdout + result.stderr


def install_local() -> tuple[bool, str]:
    """Install from built wheel. Returns (success, output)."""
    wheels = sorted((ROOT / "target" / "wheels").glob("loopflow-*.whl"))
    if not wheels:
        return False, "No wheel found in target/wheels/"

    wheel = wheels[-1]
    result = subprocess.run(
        ["uv", "tool", "install", "--force", str(wheel)],
        capture_output=True,
        text=True,
    )
    return result.returncode == 0, result.stdout + result.stderr


def install_local_binaries() -> tuple[bool, str]:
    """Build lf/lfd and install to ~/.local/bin (or LF_INSTALL_DIR)."""
    install_dir = os.environ.get("LF_INSTALL_DIR", str(Path.home() / ".local" / "bin"))
    install_path = Path(install_dir)
    install_path.mkdir(parents=True, exist_ok=True)

    result = subprocess.run(
        ["cargo", "build", "-p", "lf", "-p", "lfd", "--release"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return False, result.stdout + result.stderr

    built = ROOT / "target" / "release"
    for name in ("lf", "lfd"):
        src = built / name
        if src.exists():
            dst = install_path / name
            dst.write_bytes(src.read_bytes())
            dst.chmod(0o755)

    return True, f"Installed lf/lfd to {install_path}"


# --- Release flow ---


def check_on_main() -> tuple[bool, str]:
    """Check if on main branch and synced."""
    result = subprocess.run(
        ["git", "branch", "--show-current"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    branch = result.stdout.strip()
    if branch != "main":
        return False, f"Not on main branch (current: {branch})"

    # Check if synced with origin
    subprocess.run(["git", "fetch", "origin", "main"], cwd=ROOT, capture_output=True)
    result = subprocess.run(
        ["git", "rev-parse", "HEAD", "origin/main"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    shas = result.stdout.strip().split("\n")
    if len(shas) == 2 and shas[0] != shas[1]:
        return False, "Local main differs from origin/main. Pull or push first."

    # Check for uncommitted changes
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if result.stdout.strip():
        return False, "Uncommitted changes in working directory"

    return True, "Ready"


# --- DMG publishing ---


def build_dmg() -> tuple[bool, str]:
    """Build Concerto DMG. Returns (success, output)."""
    swift_dir = ROOT / "swift"
    if not swift_dir.exists():
        return False, f"swift directory not found: {swift_dir}"

    result = subprocess.run(
        [sys.executable, "scripts/dev.py", "release"],
        cwd=swift_dir,
        capture_output=True,
        text=True,
    )
    return result.returncode == 0, result.stdout + result.stderr


def get_dmg_path() -> Path:
    """Get path to built DMG."""
    return ROOT / "swift" / "dist" / "LoopflowConcerto.dmg"


def upload_dmg(version: str) -> tuple[bool, str]:
    """Upload DMG to Cloudflare R2. Returns (success, output)."""
    try:
        import boto3
    except ImportError:
        return False, "boto3 required for DMG upload: pip install boto3"

    account_id = os.environ.get("R2_ACCOUNT_ID")
    access_key = os.environ.get("R2_ACCESS_KEY_ID")
    secret_key = os.environ.get("R2_SECRET_ACCESS_KEY")
    bucket = os.environ.get("R2_BUCKET_NAME", "loopflow-downloads")

    if not all([account_id, access_key, secret_key]):
        return False, (
            "R2 credentials not set. Required environment variables:\n"
            "  R2_ACCOUNT_ID\n"
            "  R2_ACCESS_KEY_ID\n"
            "  R2_SECRET_ACCESS_KEY"
        )

    dmg_path = get_dmg_path()
    if not dmg_path.exists():
        return False, f"DMG not found: {dmg_path}"

    client = boto3.client(
        "s3",
        endpoint_url=f"https://{account_id}.r2.cloudflarestorage.com",
        aws_access_key_id=access_key,
        aws_secret_access_key=secret_key,
    )

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

        return True, f"Uploaded to {R2_PUBLIC_URL}/{versioned_key}"
    except Exception as e:
        return False, f"Upload failed: {e}"


def generate_screenshots() -> tuple[bool, str]:
    """Run screenshot generation. Returns (success, output)."""
    script = ROOT / "scripts" / "generate_screenshots.py"
    result = subprocess.run(
        [sys.executable, str(script)],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    return result.returncode == 0, result.stdout + result.stderr


# --- Release flow ---


def create_release(
    bump_type: str,
    dry_run: bool,
    tag_only: bool = False,
    skip_dmg: bool = False,
    skip_screenshots: bool = False,
) -> int:
    """Full release: screenshots, DMG, version bump, tag, push."""
    # Check state
    ok, msg = check_on_main()
    if not ok:
        print(f"Error: {msg}", file=sys.stderr)
        return 1

    ok, msg = check_versions()
    if not ok:
        print(f"Error: {msg}", file=sys.stderr)
        return 1

    old_version = get_version()
    new_version = bump_version(old_version, bump_type)

    do_screenshots = not tag_only and not skip_screenshots
    do_dmg = not tag_only and not skip_dmg

    if dry_run:
        if do_screenshots:
            print("Would generate screenshots")
        if do_dmg:
            print("Would build and upload DMG")
        print(f"Would bump version: {old_version} → {new_version}")
        print(f"Would commit and tag v{new_version}")
        print("Would push tag to trigger CI release")
        return 0

    # Screenshots
    if do_screenshots:
        print("Generating screenshots...")
        ok, output = generate_screenshots()
        if not ok:
            print(f"Screenshot generation failed (continuing):\n{output}", file=sys.stderr)
        else:
            print("Screenshots generated.")

    # DMG
    if do_dmg:
        print("Building Concerto DMG...")
        ok, output = build_dmg()
        if not ok:
            print(f"DMG build failed (continuing):\n{output}", file=sys.stderr)
        else:
            print("DMG built.")
            print("Uploading DMG...")
            ok, output = upload_dmg(new_version)
            if not ok:
                print(f"DMG upload failed (continuing):\n{output}", file=sys.stderr)
            else:
                print(output)

    # Version bump
    print(f"Bumping version: {old_version} → {new_version}")
    write_version(new_version)

    # Commit
    subprocess.run(["git", "add", "VERSION", "pyproject.toml", "Cargo.toml", "Cargo.lock"], cwd=ROOT, check=True)
    subprocess.run(
        ["git", "commit", "-m", f"release: v{new_version}"],
        cwd=ROOT,
        check=True,
    )

    # Tag
    subprocess.run(["git", "tag", f"v{new_version}"], cwd=ROOT, check=True)

    # Push
    print("Pushing to origin...")
    subprocess.run(["git", "push", "origin", "main"], cwd=ROOT, check=True)
    subprocess.run(["git", "push", "origin", f"v{new_version}"], cwd=ROOT, check=True)

    print(f"\nReleased v{new_version}")
    print("CI will build and publish to PyPI.")
    print("Watch: https://github.com/loopflow-ai/loopflow/actions")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Build and publish loopflow")
    parser.add_argument(
        "bump",
        nargs="?",
        choices=["patch", "minor", "major"],
        help="Version bump type for release",
    )
    parser.add_argument(
        "--local",
        action="store_true",
        help="Build and install locally (no publish)",
    )
    parser.add_argument(
        "--dmg-only",
        action="store_true",
        help="Just upload existing DMG",
    )
    parser.add_argument(
        "--screenshots-only",
        action="store_true",
        help="Just generate screenshots",
    )
    parser.add_argument(
        "--tag-only",
        action="store_true",
        help="Just bump, tag, push (skip screenshots/DMG)",
    )
    parser.add_argument(
        "--skip-dmg",
        action="store_true",
        help="Skip DMG build/upload in release",
    )
    parser.add_argument(
        "--skip-screenshots",
        action="store_true",
        help="Skip screenshot generation in release",
    )
    parser.add_argument(
        "-n", "--dry-run",
        action="store_true",
        help="Show what would be done",
    )
    args = parser.parse_args()

    # Screenshots-only mode
    if args.screenshots_only:
        if args.dry_run:
            print("Would generate screenshots")
            return 0
        print("Generating screenshots...")
        ok, output = generate_screenshots()
        if not ok:
            print(f"Screenshot generation failed:\n{output}", file=sys.stderr)
            return 1
        print("Screenshots generated.")
        return 0

    # DMG-only mode
    if args.dmg_only:
        version = get_version()
        if args.dry_run:
            print(f"Would upload DMG to {R2_PUBLIC_URL}/LoopflowConcerto-{version}.dmg")
            return 0
        print("Uploading DMG...")
        ok, output = upload_dmg(version)
        if not ok:
            print(f"DMG upload failed:\n{output}", file=sys.stderr)
            return 1
        print(output)
        return 0

    # Local build mode
    if args.local:
        if args.dry_run:
            print("Would build wheel with maturin")
            print("Would install with uv tool install")
            return 0

        print("Building wheel...")
        ok, output = build_local()
        if not ok:
            print(f"Build failed:\n{output}", file=sys.stderr)
            return 1
        print("Build succeeded.")

        print("Installing locally...")
        ok, output = install_local()
        if not ok:
            print(f"Install failed:\n{output}", file=sys.stderr)
            return 1
        print("Installed.")

        print("Installing lf/lfd binaries...")
        ok, output = install_local_binaries()
        if not ok:
            print(f"Binary install failed:\n{output}", file=sys.stderr)
            return 1
        print(output)

        # Show installed binaries
        result = subprocess.run(["which", "lf"], capture_output=True, text=True)
        if result.returncode == 0:
            print(f"lf: {result.stdout.strip()}")
        result = subprocess.run(["which", "lfd"], capture_output=True, text=True)
        if result.returncode == 0:
            print(f"lfd: {result.stdout.strip()}")

        return 0

    # Release mode
    if not args.bump:
        parser.print_help()
        return 1

    return create_release(
        args.bump,
        args.dry_run,
        tag_only=args.tag_only,
        skip_dmg=args.skip_dmg,
        skip_screenshots=args.skip_screenshots,
    )


if __name__ == "__main__":
    sys.exit(main())
