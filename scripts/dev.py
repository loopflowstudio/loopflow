#!/usr/bin/env python3
"""Development commands for Concerto (Swift app) and GhosttyKit.

Usage:
    uv run python scripts/dev.py <command>

Commands:
    setup           Install/check repo dev environment tools
    build           Build the app
    test            Build and run tests
    run             Build and launch the app
    run-debug       Build and run with stdout visible
    release         Build release .app and .dmg
    clean           Remove dev app and reset permissions
    xcode           Open in Xcode
    logs            Tail the app logs
    lfd             Stop installed lfd and run from this branch (native/sqlite)
    lfd --docker    Run lfd with Docker executor (postgres in container)
    agent-image     Build the Docker agent image

    ghostty-build   Build GhosttyKit xcframework locally
    ghostty-update  Build, upload to R2, and update Package.swift
"""

import argparse
import os
import signal
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

REPO_ROOT = Path(__file__).parent.parent
SWIFT_DIR = REPO_ROOT / "swift"
GHOSTTY_DIR = REPO_ROOT / "vendor" / "ghostty"
DEV_APP = Path.home() / "Applications" / "Concerto Dev.app"
R2_URL = "https://bin.loopflow.studio"
ENV_SETUP = REPO_ROOT / ".lf" / "env-setup.sh"


def run(cmd: list[str], cwd: Path | None = None, check: bool = True) -> subprocess.CompletedProcess:
    """Run a command, optionally checking return code."""
    return subprocess.run(cmd, cwd=cwd, check=check)


def run_capture(cmd: list[str], cwd: Path | None = None) -> subprocess.CompletedProcess:
    """Run a command and capture output."""
    return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)


# --- Swift commands ---


def cmd_setup(install: bool = False, dry_run: bool = False) -> int:
    """Install or check idempotent repo dev environment tooling."""
    if not ENV_SETUP.exists():
        print(f"Error: missing {ENV_SETUP}")
        return 1

    mode = "--install" if install else "--check"
    cmd = [str(ENV_SETUP), mode]
    if dry_run:
        cmd.append("--dry-run")
    return run(cmd, cwd=REPO_ROOT, check=False).returncode


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
    print("Logs: ~/Library/Logs/Concerto/")
    print("Press Ctrl+C to quit")
    print("---")
    # Use exec so Ctrl+C goes directly to Concerto
    executable = str(DEV_APP / "Contents" / "MacOS" / "Concerto")
    os.execv(executable, [executable])


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


def cmd_lfd(docker: bool = False) -> int:
    """Stop installed lfd and run from this branch."""
    _stop_installed_lfd()

    if docker:
        return _lfd_docker()
    return _lfd_native()


def _stop_installed_lfd() -> None:
    """Stop any running lfd (launchd, pid file, compose, port)."""
    plist = Path.home() / "Library" / "LaunchAgents" / "com.loopflow.lfd.plist"
    if plist.exists():
        print("Unloading lfd launchd service...")
        run(["launchctl", "unload", str(plist)], check=False)

    pid_file = Path.home() / ".lf" / "lfd.pid"
    if pid_file.exists():
        try:
            pid = int(pid_file.read_text().strip())
            print(f"Stopping lfd (pid {pid})...")
            os.kill(pid, signal.SIGTERM)
            time.sleep(1.0)
        except (ValueError, ProcessLookupError):
            pass
        pid_file.unlink(missing_ok=True)

    _stop_docker_on_port(2486)


def _lfd_docker() -> int:
    """Start postgres in a container, run lfd natively with Docker executor.

    lfd needs host filesystem access to resolve repo paths and build agent
    images, so it runs on the host. Only postgres is containerized.
    """
    # Start postgres
    pg_container = "lfd-dev-postgres"
    result = run_capture(["docker", "inspect", pg_container, "--format", "{{.State.Running}}"])
    if result.returncode == 0 and result.stdout.strip() == "true":
        print(f"Postgres already running ({pg_container})")
    else:
        print("Starting postgres...")
        # Remove stopped container if it exists
        run(["docker", "rm", "-f", pg_container], check=False)
        result = run([
            "docker", "run", "-d",
            "--name", pg_container,
            "-p", "5432:5432",
            "-e", "POSTGRES_USER=lfd",
            "-e", "POSTGRES_PASSWORD=lfd",
            "-e", "POSTGRES_DB=lfd",
            "--health-cmd", "pg_isready -U lfd",
            "--health-interval", "2s",
            "--health-retries", "10",
            "postgres:16-alpine",
        ], check=False)
        if result.returncode != 0:
            return result.returncode

        # Wait for healthy
        print("Waiting for postgres...")
        for _ in range(30):
            time.sleep(1)
            check = run_capture([
                "docker", "inspect", pg_container,
                "--format", "{{.State.Health.Status}}",
            ])
            if check.stdout.strip() == "healthy":
                break
        else:
            print("Postgres did not become healthy in time")
            return 1
        print("Postgres ready")

    # Build agent image if needed (cached rebuilds are instant)
    result = _ensure_agent_image()
    if result != 0:
        return result

    # Build lfd from source
    os.environ["GRPC_ENABLE_FORK_SUPPORT"] = "0"
    os.environ["GRPC_VERBOSITY"] = "ERROR"

    print("Building lfd...")
    result = run(["cargo", "build", "--bin", "lfd"], cwd=REPO_ROOT, check=False)
    if result.returncode != 0:
        return result.returncode

    # Run lfd natively in container mode
    os.environ["LFD_MODE"] = "container"
    os.environ["LFD_DATABASE_URL"] = "postgres://lfd:lfd@127.0.0.1:5432/lfd"
    os.environ["LFD_EXECUTOR_CREDENTIALS_MOUNTS"] = "claude,ssh,gitconfig"
    os.environ["RUST_LOG"] = "loopflow=debug,tower_http=debug"

    lfd_bin = str(REPO_ROOT / "target" / "debug" / "lfd")
    print("Starting lfd (container mode, debug logging)...")
    os.execv(lfd_bin, [lfd_bin, "serve"])


def _lfd_native() -> int:
    """Build and run lfd natively (sqlite, local executor)."""
    os.environ["GRPC_ENABLE_FORK_SUPPORT"] = "0"
    os.environ["GRPC_VERBOSITY"] = "ERROR"

    print("Building lfd...")
    result = run(["cargo", "build", "--bin", "lfd"], cwd=REPO_ROOT, check=False)
    if result.returncode != 0:
        return result.returncode

    os.environ["RUST_LOG"] = "loopflow=debug,tower_http=debug"

    lfd_bin = str(REPO_ROOT / "target" / "debug" / "lfd")
    print("Starting lfd from this branch (debug logging enabled)...")
    os.execv(lfd_bin, [lfd_bin, "serve"])


def _ensure_agent_image() -> int:
    """Build the agent image if it doesn't exist."""
    result = run_capture(["docker", "image", "inspect", "loopflow/agent:latest"])
    if result.returncode == 0:
        print("Agent image exists (loopflow/agent:latest)")
        return 0
    return cmd_agent_image()


def cmd_agent_image() -> int:
    """Build the Docker agent image."""
    print("Building loopflow/agent:latest...")
    return run(
        ["docker", "build", "-t", "loopflow/agent:latest", "docker/agent"],
        cwd=REPO_ROOT,
        check=False,
    ).returncode


def _stop_docker_on_port(port: int) -> None:
    """Stop any Docker containers bound to the given port."""
    result = run_capture(["docker", "ps", "--filter", f"publish={port}", "-q"])
    if result.returncode != 0 or not result.stdout.strip():
        return
    for container_id in result.stdout.strip().splitlines():
        print(f"Stopping Docker container {container_id} on port {port}...")
        run(["docker", "stop", container_id], check=False)


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
            [
                "git", "clone", "--depth", "1",
                "https://github.com/ghostty-org/ghostty.git", str(GHOSTTY_DIR)
            ],
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
    "setup": (cmd_setup, "Install/check repo dev environment tools"),
    "build": (cmd_build, "Build the app"),
    "test": (cmd_test, "Build and run tests"),
    "run": (cmd_run, "Build and launch the app"),
    "run-debug": (cmd_run_debug, "Build and run with stdout visible"),
    "release": (cmd_release, "Build release .app and .dmg"),
    "clean": (cmd_clean, "Remove dev app and reset permissions"),
    "xcode": (cmd_xcode, "Open in Xcode"),
    "logs": (cmd_logs, "Tail the app logs"),
    "lfd": (cmd_lfd, "Stop installed lfd and run from this branch (--docker for compose)"),
    "agent-image": (cmd_agent_image, "Build the Docker agent image"),
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
        sub = subparsers.add_parser(name, help=help_text)
        if name == "lfd":
            sub.add_argument("--docker", action="store_true", help="Use Docker executor")
        if name == "setup":
            sub.add_argument(
                "--install",
                action="store_true",
                help="install missing tools (default is check-only)",
            )
            sub.add_argument(
                "--dry-run",
                action="store_true",
                help="print what would be installed",
            )

    args = parser.parse_args()

    if not args.command:
        parser.print_help()
        return 0

    func, _ = COMMANDS[args.command]
    if args.command == "lfd":
        return func(docker=args.docker)
    if args.command == "setup":
        return func(install=args.install, dry_run=args.dry_run)
    return func()


if __name__ == "__main__":
    sys.exit(main())
