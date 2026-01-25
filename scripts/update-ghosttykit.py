#!/usr/bin/env python3
"""Update GhosttyKit dependency to latest Ghostty version.

Builds from source, uploads to R2, updates Package.swift.
"""

import re
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).parent.parent
GHOSTTY_DIR = REPO_ROOT / "vendor" / "ghostty"
SWIFT_DIR = REPO_ROOT / "swift"
PACKAGE_SWIFT = SWIFT_DIR / "Package.swift"
R2_URL = "https://bin.loopflow.studio"


def run(cmd: list[str], cwd: Path | None = None) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)


def clone_or_update_ghostty() -> str:
    """Clone or update Ghostty, return commit hash."""
    if GHOSTTY_DIR.exists():
        print("Updating Ghostty...")
        run(["git", "fetch", "origin"], cwd=GHOSTTY_DIR)
        run(["git", "checkout", "main"], cwd=GHOSTTY_DIR)
        run(["git", "pull"], cwd=GHOSTTY_DIR)
    else:
        print("Cloning Ghostty...")
        GHOSTTY_DIR.parent.mkdir(parents=True, exist_ok=True)
        result = run(["git", "clone", "--depth", "1", "https://github.com/ghostty-org/ghostty.git", str(GHOSTTY_DIR)])
        if result.returncode != 0:
            print(f"Clone failed: {result.stderr}", file=sys.stderr)
            sys.exit(1)

    result = run(["git", "rev-parse", "--short", "HEAD"], cwd=GHOSTTY_DIR)
    return result.stdout.strip()


def build_xcframework() -> Path:
    """Build GhosttyKit.xcframework, return path."""
    print("Building GhosttyKit...")
    result = subprocess.run(["zig", "build", "-Doptimize=ReleaseFast"], cwd=GHOSTTY_DIR)
    if result.returncode != 0:
        print("Build failed", file=sys.stderr)
        sys.exit(1)

    # Find xcframework
    for candidate in [
        GHOSTTY_DIR / "zig-out" / "frameworks" / "GhosttyKit.xcframework",
        GHOSTTY_DIR / "macos" / "GhosttyKit.xcframework",
    ]:
        if candidate.exists():
            return candidate

    print("GhosttyKit.xcframework not found after build", file=sys.stderr)
    sys.exit(1)


def create_zip(xcfw_path: Path, commit: str) -> Path:
    """Zip the xcframework, return zip path."""
    zip_name = f"GhosttyKit-{commit}.xcframework.zip"
    zip_path = Path(tempfile.gettempdir()) / zip_name

    print(f"Creating {zip_path}...")
    if zip_path.exists():
        zip_path.unlink()

    result = run(
        ["zip", "-r", str(zip_path), "GhosttyKit.xcframework"],
        cwd=xcfw_path.parent,
    )
    if result.returncode != 0:
        print(f"Zip failed: {result.stderr}", file=sys.stderr)
        sys.exit(1)

    return zip_path


def compute_checksum(zip_path: Path) -> str:
    """Compute Swift package checksum."""
    result = run(["swift", "package", "compute-checksum", str(zip_path)])
    if result.returncode != 0:
        print(f"Checksum failed: {result.stderr}", file=sys.stderr)
        sys.exit(1)
    return result.stdout.strip()


def upload_to_r2(zip_path: Path, key: str) -> None:
    """Upload to R2 bin bucket."""
    print("Uploading to R2...")
    from loopflow.publish import upload_file

    success, msg = upload_file(zip_path, key, "application/zip", bucket="bin")
    print(msg)
    if not success:
        sys.exit(1)


def update_package_swift(url: str, checksum: str) -> None:
    """Update Package.swift with new URL and checksum."""
    print(f"Updating {PACKAGE_SWIFT}...")
    content = PACKAGE_SWIFT.read_text()

    # Update URL
    content = re.sub(
        r'(\.binaryTarget\([^)]*url:\s*")[^"]*(")',
        rf'\g<1>{url}\2',
        content,
    )

    # Update checksum
    content = re.sub(
        r'(\.binaryTarget\([^)]*checksum:\s*")[^"]*(")',
        rf'\g<1>{checksum}\2',
        content,
    )

    PACKAGE_SWIFT.write_text(content)


def main() -> int:
    commit = clone_or_update_ghostty()
    print(f"Ghostty commit: {commit}")

    xcfw_path = build_xcframework()
    zip_path = create_zip(xcfw_path, commit)
    checksum = compute_checksum(zip_path)
    print(f"Checksum: {checksum}")

    key = f"GhosttyKit-{commit}.xcframework.zip"
    upload_to_r2(zip_path, key)

    url = f"{R2_URL}/{key}"
    update_package_swift(url, checksum)

    print()
    print("Updated Package.swift:")
    for line in PACKAGE_SWIFT.read_text().split("\n"):
        if "binaryTarget" in line or "url:" in line or "checksum:" in line:
            print(f"  {line.strip()}")

    print()
    print(f"Done! GhosttyKit updated to {commit}")
    print("Run 'swift package resolve' to download the new version")
    print()
    print("Optionally remove vendor/ghostty to save disk space:")
    print(f"  rm -rf {GHOSTTY_DIR}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
