#!/usr/bin/env python3
"""Development commands for Concerto (Swift app) and GhosttyKit.

Usage:
    uv run python swift/scripts/dev.py <command>

Commands:
    build           Build the app
    test            Build and run tests
    run             Build and launch the app
    run-debug       Build and run with stdout visible
    release         Build release .app and .dmg
    clean           Remove dev app and reset permissions
    xcode           Open in Xcode
    logs            Tail the app logs

    ghostty-build   Build GhosttyKit xcframework locally
    ghostty-update  Build, upload to R2, and update Package.swift
"""

import argparse
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

SWIFT_DIR = Path(__file__).parent.parent
REPO_ROOT = SWIFT_DIR.parent
GHOSTTY_DIR = REPO_ROOT / "vendor" / "ghostty"
DEV_APP = Path.home() / "Applications" / "Concerto Dev.app"
R2_URL = "https://bin.loopflow.studio"


def run(cmd: list[str], cwd: Path | None = None, check: bool = True) -> subprocess.CompletedProcess:
    """Run a command, optionally checking return code."""
    return subprocess.run(cmd, cwd=cwd, check=check)


def run_capture(cmd: list[str], cwd: Path | None = None) -> subprocess.CompletedProcess:
    """Run a command and capture output."""
    return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)


# --- Swift commands ---


def cmd_build() -> int:
    """Build the app."""
    print("Building Concerto...")
    return run(["swift", "build"], cwd=SWIFT_DIR, check=False).returncode


def cmd_test() -> int:
    """Build and run tests."""
    print("Building and testing...")
    result = run(["swift", "build"], cwd=SWIFT_DIR, check=False)
    if result.returncode != 0:
        return result.returncode
    return run(["swift", "test"], cwd=SWIFT_DIR, check=False).returncode


def cmd_run() -> int:
    """Build and launch the app."""
    print("Building and running Concerto...")
    result = run(["swift", "build"], cwd=SWIFT_DIR, check=False)
    if result.returncode != 0:
        return result.returncode

    _install_dev_app()
    run(["open", str(DEV_APP)])
    return 0


def cmd_run_debug() -> int:
    """Build and run with stdout visible."""
    print("Building and running Concerto (debug mode with logs)...")
    result = run(["swift", "build"], cwd=SWIFT_DIR, check=False)
    if result.returncode != 0:
        return result.returncode

    _install_dev_app()
    print(f"Logs: ~/Library/Logs/Concerto/")
    print("Press Ctrl+C to quit")
    print("---")
    return run([str(DEV_APP / "Contents" / "MacOS" / "Concerto")], check=False).returncode


def cmd_release() -> int:
    """Build release .app and .dmg."""
    print("Building Concerto release...")

    result = run(["swift", "build", "-c", "release"], cwd=SWIFT_DIR, check=False)
    if result.returncode != 0:
        return result.returncode

    app_name = "Loopflow Concerto"
    dist_dir = SWIFT_DIR / "dist"
    app_dir = dist_dir / f"{app_name}.app" / "Contents"

    # Clean and create directories
    if dist_dir.exists():
        shutil.rmtree(dist_dir)
    (app_dir / "MacOS").mkdir(parents=True)
    (app_dir / "Resources").mkdir(parents=True)

    # Copy files
    shutil.copy(SWIFT_DIR / ".build" / "release" / "Concerto", app_dir / "MacOS")
    shutil.copy(SWIFT_DIR / "Concerto" / "Info.plist", app_dir)
    shutil.copy(SWIFT_DIR / "Concerto" / "Concerto.sdef", app_dir / "Resources")
    shutil.copy(SWIFT_DIR / "Concerto" / "AppIcon.icns", app_dir / "Resources")
    (app_dir / "PkgInfo").write_text("APPL????")

    print(f"Created dist/{app_name}.app")

    # Create DMG
    dmg_path = dist_dir / "LoopflowConcerto.dmg"
    dmg_tmp = dist_dir / "dmg_tmp"
    dmg_tmp.mkdir()
    shutil.copytree(dist_dir / f"{app_name}.app", dmg_tmp / f"{app_name}.app")
    (dmg_tmp / "Applications").symlink_to("/Applications")

    run([
        "hdiutil", "create",
        "-volname", app_name,
        "-srcfolder", str(dmg_tmp),
        "-ov", "-format", "UDZO",
        str(dmg_path),
    ])
    shutil.rmtree(dmg_tmp)

    print()
    print("Release built:")
    print(f"  App: dist/{app_name}.app")
    print(f"  DMG: {dmg_path}")
    return 0


def cmd_clean() -> int:
    """Remove dev app and reset permissions."""
    print("Removing dev app...")
    if DEV_APP.exists():
        shutil.rmtree(DEV_APP)

    print("Resetting Accessibility permissions...")
    run(["tccutil", "reset", "Accessibility", "com.loopflow.concerto"], check=False)
    print("Resetting Automation permissions...")
    run(["tccutil", "reset", "AppleEvents", "com.loopflow.concerto"], check=False)
    print("Done. Next run will require re-granting permissions.")
    return 0


def cmd_xcode() -> int:
    """Open in Xcode."""
    print("Opening in Xcode...")
    return run(["open", str(SWIFT_DIR / "LoopflowSwift.xcodeproj")], check=False).returncode


def cmd_logs() -> int:
    """Tail the app logs."""
    log_dir = Path.home() / "Library" / "Logs" / "Concerto"
    if log_dir.exists():
        logs = list(log_dir.glob("*.log"))
        if logs:
            return run(["tail", "-f"] + [str(log) for log in logs], check=False).returncode
    print("No logs yet. Run the app first.")
    return 1


def _install_dev_app() -> None:
    """Install debug build to stable location for permissions persistence."""
    app_dir = DEV_APP / "Contents"
    (app_dir / "MacOS").mkdir(parents=True, exist_ok=True)
    (app_dir / "Resources").mkdir(parents=True, exist_ok=True)

    shutil.copy(SWIFT_DIR / ".build" / "debug" / "Concerto", app_dir / "MacOS")
    shutil.copy(SWIFT_DIR / "Concerto" / "Info.plist", app_dir)
    shutil.copy(SWIFT_DIR / "Concerto" / "Concerto.sdef", app_dir / "Resources")
    shutil.copy(SWIFT_DIR / "Concerto" / "AppIcon.icns", app_dir / "Resources")

    run(["codesign", "--force", "--deep", "--sign", "-", str(DEV_APP)])


# --- Ghostty commands ---


def cmd_ghostty_build() -> int:
    """Build GhosttyKit xcframework locally."""
    print("=== Building Ghostty for Concerto ===")

    # Check zig
    result = run_capture(["zig", "version"])
    if result.returncode != 0:
        print("Error: Zig not found. Install with: brew install zig")
        return 1
    print(f"Using Zig {result.stdout.strip()}")

    # Check metal toolchain
    result = run_capture(["xcrun", "-sdk", "macosx", "metal", "--version"])
    if result.returncode != 0:
        print("Metal toolchain not found. Downloading...")
        run(["xcodebuild", "-downloadComponent", "MetalToolchain"])

    # Clone if needed
    if not GHOSTTY_DIR.exists():
        print("Cloning Ghostty...")
        GHOSTTY_DIR.parent.mkdir(parents=True, exist_ok=True)
        result = run(
            ["git", "clone", "--depth", "1", "https://github.com/ghostty-org/ghostty.git", str(GHOSTTY_DIR)],
            check=False,
        )
        if result.returncode != 0:
            return result.returncode

    # Build
    print("Building Ghostty...")
    result = run(["zig", "build", "-Doptimize=ReleaseFast"], cwd=GHOSTTY_DIR, check=False)
    if result.returncode != 0:
        return result.returncode

    # Verify
    xcfw = GHOSTTY_DIR / "macos" / "GhosttyKit.xcframework"
    if not xcfw.exists():
        xcfw = GHOSTTY_DIR / "zig-out" / "frameworks" / "GhosttyKit.xcframework"

    if xcfw.exists():
        print(f"Success: {xcfw}")
        return 0
    else:
        print("Error: GhosttyKit.xcframework not found")
        return 1


def cmd_ghostty_update() -> int:
    """Build GhosttyKit, upload to R2, and update Package.swift."""
    # Build first
    result = cmd_ghostty_build()
    if result != 0:
        return result

    # Get commit
    result = run_capture(["git", "rev-parse", "--short", "HEAD"], cwd=GHOSTTY_DIR)
    commit = result.stdout.strip()
    print(f"Ghostty commit: {commit}")

    # Find xcframework
    xcfw = GHOSTTY_DIR / "macos" / "GhosttyKit.xcframework"
    if not xcfw.exists():
        xcfw = GHOSTTY_DIR / "zig-out" / "frameworks" / "GhosttyKit.xcframework"

    # Create zip
    zip_name = f"GhosttyKit-{commit}.xcframework.zip"
    zip_path = Path(tempfile.gettempdir()) / zip_name
    print(f"Creating {zip_path}...")
    if zip_path.exists():
        zip_path.unlink()
    run(["zip", "-r", str(zip_path), "GhosttyKit.xcframework"], cwd=xcfw.parent)

    # Compute checksum
    result = run_capture(["swift", "package", "compute-checksum", str(zip_path)])
    checksum = result.stdout.strip()
    print(f"Checksum: {checksum}")

    # Upload to R2
    print("Uploading to R2...")
    from loopflow.publish import upload_file
    success, msg = upload_file(zip_path, zip_name, "application/zip", bucket="bin")
    print(msg)
    if not success:
        return 1

    # Update Package.swift
    url = f"{R2_URL}/{zip_name}"
    _update_package_swift(url, checksum)

    print()
    print(f"Done! GhosttyKit updated to {commit}")
    print("Run 'swift package resolve' to download the new version")
    print()
    print("Optionally remove vendor/ghostty to save disk space:")
    print(f"  rm -rf {GHOSTTY_DIR}")
    return 0


def _update_package_swift(url: str, checksum: str) -> None:
    """Update Package.swift with new URL and checksum."""
    import re

    package_swift = SWIFT_DIR / "Package.swift"
    print(f"Updating {package_swift}...")
    content = package_swift.read_text()

    content = re.sub(
        r'(\.binaryTarget\([^)]*url:\s*")[^"]*(")',
        rf'\g<1>{url}\2',
        content,
    )
    content = re.sub(
        r'(\.binaryTarget\([^)]*checksum:\s*")[^"]*(")',
        rf'\g<1>{checksum}\2',
        content,
    )
    package_swift.write_text(content)


# --- Main ---


COMMANDS = {
    "build": (cmd_build, "Build the app"),
    "test": (cmd_test, "Build and run tests"),
    "run": (cmd_run, "Build and launch the app"),
    "run-debug": (cmd_run_debug, "Build and run with stdout visible"),
    "release": (cmd_release, "Build release .app and .dmg"),
    "clean": (cmd_clean, "Remove dev app and reset permissions"),
    "xcode": (cmd_xcode, "Open in Xcode"),
    "logs": (cmd_logs, "Tail the app logs"),
    "ghostty-build": (cmd_ghostty_build, "Build GhosttyKit xcframework locally"),
    "ghostty-update": (cmd_ghostty_update, "Build, upload to R2, update Package.swift"),
}


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Development commands for Concerto and GhosttyKit",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    subparsers = parser.add_subparsers(dest="command", metavar="command")

    for name, (func, help_text) in COMMANDS.items():
        subparsers.add_parser(name, help=help_text)

    args = parser.parse_args()

    if not args.command:
        parser.print_help()
        return 0

    func, _ = COMMANDS[args.command]
    return func()


if __name__ == "__main__":
    sys.exit(main())
