#!/usr/bin/env python3
"""Development commands for the Loopflow Swift app.

Usage:
    uv run python scripts/loopflow-dev.py <command>

Commands:
    setup           Install/check repo dev environment tools
    build           Build the app
    install         Build and install without launching
    test            Build and run tests
    ghostty-build   Build the pinned patched GhosttyKit
    run             Build and launch the app
    run-debug       Build and run with stdout visible (--world <dir> opens an lf-world)
    release         Build release .app and .dmg (delegates to release-loopflow.py)
    clean           Remove dev app and reset permissions
    xcode           Open in Xcode
    logs            Tail the app logs

    screenshots     Generate app screenshots

Streaming logs (long-running commands):
    ~/.lf/logs/dev/<repo>.loopflow-run-debug.log
"""

from __future__ import annotations

import argparse
import json
import os
import plistlib
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.request
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
SWIFT_DIR = REPO_ROOT / "swift"
DEV_APP = Path.home() / "Applications" / "Loopflow Dev.app"
# Dev build runs under its own bundle id so it keeps preferences separate from
# the installed Loopflow app.
DEV_BUNDLE_ID = "com.loopflow.mac.dev"
# Stable self-signed identity used to sign dev builds. Ad-hoc signing (`--sign -`)
# yields a fresh cdhash every build, so the keychain ACL that "Always Allow"
# stays stable across builds and macOS keeps previously granted permissions.
DEV_SIGNING_IDENTITY = "Loopflow Dev"
LOGIN_KEYCHAIN = Path.home() / "Library" / "Keychains" / "login.keychain-db"
ENV_SETUP = REPO_ROOT / "scripts" / "env-setup.sh"
DEV_LOG_DIR = Path.home() / ".lf" / "logs" / "dev"
LOOPFLOW_STREAM_LOG = DEV_LOG_DIR / f"{REPO_ROOT.name}.loopflow-run-debug.log"
DEV_CONTROL_CONFIG = "LoopflowDevControl.json"
GHOSTTY_REVISION = "a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0"
GHOSTTY_ARTIFACT = "GhosttyKit-a60e9e2-lf7-theme-readback1.xcframework.zip"


def _app_environment(repo: Path, world: Path | None = None) -> dict[str, str]:
    env = {
        "LOOPFLOW_DEV_WAVE_REPO": str(repo),
        "LF_BIN": str(DEV_APP / "Contents/MacOS/lf"),
    }
    if value := os.environ.get("LF_HOME"):
        env["LF_HOME"] = value
    if world:
        env.update(json.loads((world / "env.json").read_text()))
    return env


def run(cmd: list[str], cwd: Path | None = None, check: bool = True) -> subprocess.CompletedProcess:
    """Run a command, optionally checking return code."""
    return subprocess.run(cmd, cwd=cwd, check=check)


def run_capture(cmd: list[str], cwd: Path | None = None) -> subprocess.CompletedProcess:
    """Run a command and capture output."""
    return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)


def run_app_bundle_with_log(
    app_path: Path,
    log_path: Path,
    args: list[str] | None = None,
    env: dict[str, str] | None = None,
) -> int:
    """Launch app bundle through LaunchServices and stream redirected stdout/stderr.

    `env` entries are passed to the launched app via `open --env` (LaunchServices
    does not inherit this process's environment).
    """
    DEV_LOG_DIR.mkdir(parents=True, exist_ok=True)

    open_cmd = [
        "open",
        "-n",
        "-W",
        "--stdout",
        str(log_path),
        "--stderr",
        str(log_path),
    ]
    for key, value in (env or {}).items():
        open_cmd.extend(["--env", f"{key}={value}"])
    open_cmd.append(str(app_path))
    if args:
        open_cmd.extend(["--args", *args])
    with log_path.open("a", encoding="utf-8") as log_file:
        log_file.write(f"\n\n=== {time.strftime('%Y-%m-%d %H:%M:%S')} ===\n")
        log_file.write(f"$ {' '.join(open_cmd)}\n\n")
        log_file.flush()

    tail_process = subprocess.Popen(["tail", "-n", "0", "-F", str(log_path)])
    try:
        return run(open_cmd, check=False).returncode
    except KeyboardInterrupt:
        print("\nStopping app...")
        _stop_loopflow_app(app_path)
        return 130
    finally:
        if tail_process.poll() is None:
            tail_process.terminate()
            try:
                tail_process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                tail_process.kill()


def _stop_loopflow_app(app_path: Path) -> None:
    """Quit Loopflow Dev, falling back to killing the app if a modal blocks quit."""
    executable = app_path / "Contents" / "MacOS" / "Loopflow"

    run(["osascript", "-e", f'tell application id "{DEV_BUNDLE_ID}" to quit'], check=False)
    if _wait_for_process_exit(str(executable), timeout_seconds=3):
        return

    print("App did not quit cleanly; forcing shutdown...")
    run(["pkill", "-f", str(executable)], check=False)
    _wait_for_process_exit(str(executable), timeout_seconds=2)


def _wait_for_process_exit(pattern: str, timeout_seconds: float) -> bool:
    deadline = time.time() + timeout_seconds
    while time.time() < deadline:
        result = run_capture(["pgrep", "-f", pattern])
        if result.returncode != 0:
            return True
        time.sleep(0.1)
    return False


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


def cmd_ffi(release: bool = False) -> int:
    """Build the Rust library the app links, where Package.swift finds it."""
    cargo = ["cargo", "build", "--locked", "-p", "loopflow-ffi", "--lib"]
    return run(cargo + (["--release"] if release else []), cwd=REPO_ROOT, check=False).returncode


def cmd_build() -> int:
    """Build the app."""
    print("Building Loopflow...")
    return cmd_ffi() or run(["swift", "build"], cwd=SWIFT_DIR, check=False).returncode


def cmd_install() -> int:
    """Build and install the app without opening a live operator surface."""
    result = cmd_build()
    if result != 0:
        return result
    _stop_loopflow_app(DEV_APP)
    _install_dev_app()
    return 0


def cmd_test() -> int:
    """Build and run tests."""
    print("Building and testing...")
    if built := cmd_ffi():
        return built
    result = run(["swift", "build"], cwd=SWIFT_DIR, check=False)
    if result.returncode != 0:
        return result.returncode
    return run(["swift", "test"], cwd=SWIFT_DIR, check=False).returncode


def _normalize_ghostty_library(framework: Path) -> None:
    """Give SwiftPM the static-library basename it requires."""
    manifest = framework / "Info.plist"
    info = plistlib.loads(manifest.read_bytes())
    for library in info["AvailableLibraries"]:
        directory = framework / library["LibraryIdentifier"]
        original = library["LibraryPath"]
        name = "libghostty.a"
        if original != name:
            (directory / original).rename(directory / name)
        library["LibraryPath"] = name
        library["BinaryPath"] = name
    manifest.write_bytes(plistlib.dumps(info))


def cmd_ghostty_build() -> int:
    """Build the pinned GhosttyKit with Loopflow's small embedder patch."""
    build_root = SWIFT_DIR / ".build" / "ghostty-kit"
    source = build_root / f"ghostty-{GHOSTTY_REVISION}"
    archive = build_root / "source.tar.gz"
    local_framework = SWIFT_DIR / ".build" / "local" / "GhosttyKit.xcframework"
    artifact = SWIFT_DIR / ".build" / "artifacts" / GHOSTTY_ARTIFACT

    if build_root.exists():
        shutil.rmtree(build_root)
    build_root.mkdir(parents=True)

    url = f"https://github.com/ghostty-org/ghostty/archive/{GHOSTTY_REVISION}.tar.gz"
    print(f"Downloading Ghostty {GHOSTTY_REVISION}...")
    with urllib.request.urlopen(url) as response, archive.open("wb") as output:
        shutil.copyfileobj(response, output)
    with tarfile.open(archive) as source_archive:
        source_archive.extractall(build_root, filter="data")

    for patch in sorted((SWIFT_DIR / "GhosttyKitPatches").glob("*.patch")):
        print(f"Applying {patch.name}...")
        with patch.open("rb") as patch_input:
            result = subprocess.run(
                ["patch", "--batch", "--forward", "-p1"],
                cwd=source,
                stdin=patch_input,
                check=False,
            )
        if result.returncode != 0:
            return result.returncode

    # Patch behavior, the protocol parser it consumes, and the exported action ABI.
    result = run(
        [
            "zig",
            "build",
            "test",
            "-Dtest-filter=command",
            "-Dtest-filter=bounded text",
            "-Dtest-filter=semantic prompt",
            "-Dtest-filter=execCommand",
            "-Dtest-filter=program status",
            "-Dtest-filter=OSC 7501",
            "-Dtest-filter=ghostty.h Action.Key",
            "-Dtest-filter=c_get:",
            "-Dtest-filter=terminal contrast",
            "-Dtest-filter=terminal feedback",
            "-Dtest-filter=renderer.cell",
        ],
        cwd=source,
        check=False,
    )
    if result.returncode != 0:
        return result.returncode

    result = run(
        [sys.executable, str(REPO_ROOT / "scripts" / "check_ghostty_shaders.py"), str(source)],
        check=False,
    )
    if result.returncode != 0:
        return result.returncode

    result = run(
        ["zig", "build", "-Doptimize=ReleaseFast", "-Demit-macos-app=false"],
        cwd=source,
        check=False,
    )
    if result.returncode != 0:
        return result.returncode

    if local_framework.exists():
        shutil.rmtree(local_framework)
    local_framework.parent.mkdir(parents=True, exist_ok=True)
    result = run(
        ["ditto", str(source / "macos" / "GhosttyKit.xcframework"), str(local_framework)],
        check=False,
    )
    if result.returncode != 0:
        return result.returncode

    _normalize_ghostty_library(local_framework)
    run(
        [
            sys.executable,
            str(REPO_ROOT / "scripts/third_party.py"),
            "package",
            str(local_framework),
            "--framework",
            str(local_framework),
        ]
    )

    artifact.parent.mkdir(parents=True, exist_ok=True)
    artifact.unlink(missing_ok=True)
    result = run(
        [
            "ditto",
            "-c",
            "-k",
            "--sequesterRsrc",
            "--keepParent",
            str(local_framework),
            str(artifact),
        ],
        check=False,
    )
    if result.returncode != 0:
        return result.returncode

    run(
        [sys.executable, str(REPO_ROOT / "scripts/third_party.py"), "verify-archive", str(artifact)]
    )
    checksum = run_capture(["swift", "package", "compute-checksum", str(artifact)])
    if checksum.returncode != 0:
        print(checksum.stderr, file=sys.stderr)
        return checksum.returncode
    print(f"Artifact: {artifact}")
    print(f"SwiftPM checksum: {checksum.stdout.strip()}")
    return 0


def cmd_run() -> int:
    """Build and launch the app."""
    print("Building and running Loopflow...")
    result = cmd_install()
    if result != 0:
        return result
    if not _check_repository_opening(REPO_ROOT):
        return 1
    # Tell the app which checkout initiated the launch. Repository discovery
    # collapses linked worktrees to their authoritative main checkout.
    command = ["open", "-n"]
    for key, value in _app_environment(REPO_ROOT).items():
        command.extend(["--env", f"{key}={value}"])
    command.extend([str(DEV_APP), "--args", "--repo", str(REPO_ROOT)])
    run(command)
    return 0


def _check_repository_opening(repo: Path, world: Path | None = None) -> bool:
    cli = DEV_APP / "Contents" / "MacOS" / "lf"
    # Match an app launch, not the enclosing agent's Task or process context.
    env = {key: value for key, value in os.environ.items() if not key.startswith("LF_")}
    env.update(_app_environment(repo, world))
    print(f"Checking repository opening through {cli}...", flush=True)
    try:
        result = subprocess.run(
            [str(cli), "config", "repository", "--json"],
            cwd=repo,
            env=env,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        detail = str(error)
    else:
        if result.returncode == 0:
            return True
        detail = result.stderr.strip() or f"lf exited with status {result.returncode}"

    print(f"Loopflow Dev cannot open this repository:\n{detail}", file=sys.stderr)
    print(
        "The app uses its bundled CLI. Check the reported repository or schema error; "
        "ordinary app commands never migrate the shared database.",
        file=sys.stderr,
    )
    return False


def _print_run_debug_checklist() -> None:
    """Print the manual review path for Work and Session surfaces."""
    print("Review checklist:")
    print("  1. Select a Wave: inspect its plan, Tasks and Runs.")
    print("  2. Open a Session under a Task and verify its terminal is retained across navigation.")
    print("  3. Verify Task conditions and PR delivery refresh from lf wave status.")


def cmd_run_debug(repo: Path = REPO_ROOT, world: Path | None = None) -> int:
    """Build and run with stdout visible. `repo` is the repo the app opens;
    `world` is an `lf-world build` directory whose data and repo it opens instead."""
    if world:
        repo = world / "repo"
    print("Building and running Loopflow (debug mode with logs)...")
    result = cmd_install()
    if result != 0:
        return result
    if not _check_repository_opening(repo, world):
        return 1
    print("Logs: ~/.lf/logs/lf.log (exits: lf doctor)")
    print(f"Stream log: {LOOPFLOW_STREAM_LOG}")
    print(f"Bundled lf: {DEV_APP}/Contents/MacOS/lf")
    print(
        "The app and its children use the bundled CLI; "
        "LF_HOME selects data, not a different binary."
    )
    print("Telemetry dashboard: Go → Telemetry (⌘1)")
    _print_run_debug_checklist()
    print("Press Ctrl+C to quit")
    print("---")
    return run_app_bundle_with_log(
        DEV_APP,
        LOOPFLOW_STREAM_LOG,
        args=["--repo", str(repo)],
        # Repository discovery collapses this checkout to its authoritative
        # main checkout before displaying or reading it.
        env=_app_environment(repo, world),
    )


def cmd_release() -> int:
    """Build release .app and .dmg. Delegates to scripts/release-loopflow.py."""
    script = REPO_ROOT / "scripts" / "release-loopflow.py"
    result = run([sys.executable, str(script)], check=False)
    return result.returncode


def cmd_clean() -> int:
    """Remove dev app and reset permissions."""
    print("Removing dev app...")
    if DEV_APP.exists():
        shutil.rmtree(DEV_APP)

    print("Resetting Accessibility permissions...")
    run(["tccutil", "reset", "Accessibility", DEV_BUNDLE_ID], check=False)
    print("Resetting Automation permissions...")
    run(["tccutil", "reset", "AppleEvents", DEV_BUNDLE_ID], check=False)
    print("Resetting Microphone permissions...")
    run(["tccutil", "reset", "Microphone", DEV_BUNDLE_ID], check=False)
    print("Done. Next run will require re-granting permissions.")
    return 0


def cmd_xcode() -> int:
    """Open in Xcode."""
    print("Opening in Xcode...")
    return run(["open", str(SWIFT_DIR / "LoopflowSwift.xcodeproj")], check=False).returncode


def cmd_logs() -> int:
    """Tail the app logs."""
    log_dir = Path.home() / "Library" / "Logs" / "Loopflow"
    if log_dir.exists():
        logs = list(log_dir.glob("*.log"))
        if logs:
            return run(["tail", "-f"] + [str(log) for log in logs], check=False).returncode
    print("No logs yet. Run the app first.")
    return 1


def _find_stable_signing_identity() -> str | None:
    """Pick a stable codesigning identity already in the keychain.

    Prefers our dedicated dev cert, then a real Apple identity (what Xcode uses
    for local debug builds). Any of these keeps the cdhash constant across
    builds so the connection-token ACL persists. Returns None if none exist.
    """
    result = run_capture(["security", "find-identity", "-v", "-p", "codesigning"])
    names = []
    for line in result.stdout.splitlines():
        start = line.find('"')
        end = line.rfind('"')
        if start != -1 and end > start:
            names.append(line[start + 1 : end])

    for preferred in (DEV_SIGNING_IDENTITY, "Apple Development", "Developer ID Application"):
        for name in names:
            if name.startswith(preferred):
                return name
    return names[0] if names else None


def _create_dev_signing_identity() -> None:
    """Create a stable, trusted self-signed code-signing identity.

    Fallback for machines with no Apple developer identity. The cert must be
    trusted for code signing or `codesign` and `find-identity` won't accept it;
    the trust skill may raise a one-time authorization dialog.
    """
    print(f'Creating stable dev signing identity "{DEV_SIGNING_IDENTITY}"...')
    p12_password = "loopflow-dev"  # transient; only unlocks the export archive
    with tempfile.TemporaryDirectory() as tmp:
        tmp_dir = Path(tmp)
        config = tmp_dir / "cert.conf"
        key = tmp_dir / "key.pem"
        cert = tmp_dir / "cert.pem"
        p12 = tmp_dir / "identity.p12"
        config.write_text(
            "[req]\n"
            "distinguished_name = dn\n"
            "x509_extensions = v3\n"
            "prompt = no\n"
            "[dn]\n"
            f"CN = {DEV_SIGNING_IDENTITY}\n"
            "[v3]\n"
            "basicConstraints = critical,CA:false\n"
            "keyUsage = critical,digitalSignature\n"
            "extendedKeyUsage = critical,codeSigning\n"
        )
        run(
            [
                "openssl",
                "req",
                "-x509",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-keyout",
                str(key),
                "-out",
                str(cert),
                "-days",
                "3650",
                "-config",
                str(config),
                "-extensions",
                "v3",
            ]
        )
        # An empty p12 password trips a MAC-verification failure in macOS'
        # `security import`, so use a transient one.
        run(
            [
                "openssl",
                "pkcs12",
                "-export",
                "-inkey",
                str(key),
                "-in",
                str(cert),
                "-out",
                str(p12),
                "-name",
                DEV_SIGNING_IDENTITY,
                "-passout",
                f"pass:{p12_password}",
            ]
        )
        # -A lets any app (incl. codesign) use the private key without an access
        # prompt. Importing into the already-unlocked login keychain is
        # non-interactive.
        run(
            [
                "security",
                "import",
                str(p12),
                "-k",
                str(LOGIN_KEYCHAIN),
                "-P",
                p12_password,
                "-A",
            ]
        )
        # Trust the cert for code signing so codesign/find-identity accept it.
        run(
            [
                "security",
                "add-trusted-cert",
                "-r",
                "trustRoot",
                "-p",
                "codeSign",
                "-k",
                str(LOGIN_KEYCHAIN),
                str(cert),
            ]
        )


def _ensure_dev_signing_identity() -> str:
    identity = _find_stable_signing_identity()
    if identity is None:
        _create_dev_signing_identity()
        identity = _find_stable_signing_identity()
    if identity is None:
        raise RuntimeError("Failed to create a dev signing identity")
    return identity


def _bundle_executable_name(info_plist: Path) -> str:
    """The binary macOS actually launches, per CFBundleExecutable."""
    result = run_capture(
        ["/usr/libexec/PlistBuddy", "-c", "Print :CFBundleExecutable", str(info_plist)]
    )
    name = (result.stdout or "").strip()
    if result.returncode != 0 or not name:
        raise RuntimeError(f"Could not read CFBundleExecutable from {info_plist}")
    return name


def _install_dev_app() -> None:
    """Install debug build to stable location for permissions persistence."""
    app_dir = DEV_APP / "Contents"
    macos_dir = app_dir / "MacOS"
    macos_dir.mkdir(parents=True, exist_ok=True)
    (app_dir / "Resources").mkdir(parents=True, exist_ok=True)

    # `swift build` names the product LoopflowMac, but Info.plist declares
    # CFBundleExecutable = Loopflow, so that is the name macOS launches. Copying
    # the build under its own name leaves whatever `Loopflow` was already there
    # to run forever: the app keeps starting a stale binary while the fresh one
    # sits beside it, and codesigning bumps the stale mtime so it even looks new.
    executable = _bundle_executable_name(SWIFT_DIR / "LoopflowMac" / "Info.plist")
    shutil.copy(SWIFT_DIR / ".build" / "debug" / "LoopflowMac", macos_dir / executable)
    stale = macos_dir / "LoopflowMac"
    if stale.name != executable and stale.exists():
        stale.unlink()
    shutil.copy(SWIFT_DIR / "LoopflowMac" / "Info.plist", app_dir)
    _apply_dev_identity(app_dir / "Info.plist")
    shutil.copy(SWIFT_DIR / "LoopflowMac" / "AppIcon.icns", app_dir / "Resources")
    _copy_bundled_tools(app_dir / "MacOS")
    (app_dir / "Resources" / DEV_CONTROL_CONFIG).unlink(missing_ok=True)

    run(
        [
            sys.executable,
            str(REPO_ROOT / "scripts/third_party.py"),
            "package",
            str(app_dir / "Resources"),
        ]
    )

    identity = _ensure_dev_signing_identity()
    entitlements = SWIFT_DIR / "LoopflowMac" / "Loopflow.entitlements"
    codesign_cmd = ["codesign", "--force", "--deep", "--sign", identity]
    if entitlements.exists():
        codesign_cmd += ["--entitlements", str(entitlements)]
    codesign_cmd.append(str(DEV_APP))
    run(codesign_cmd)


def _apply_dev_identity(plist: Path) -> None:
    """Rewrite the dev app's bundle id and name so it gets its own settings domain."""
    run(["plutil", "-replace", "CFBundleIdentifier", "-string", DEV_BUNDLE_ID, str(plist)])
    run(["plutil", "-replace", "CFBundleName", "-string", "Loopflow Dev", str(plist)])
    run(["plutil", "-replace", "CFBundleDisplayName", "-string", "Loopflow Dev", str(plist)])


def _build_control_tools() -> Path:
    # The app is a live operator surface even when its Swift shell is a dev
    # build. The app selects its bundled CLI for every child; an ordinary lf
    # never migrates the main Machine.
    target_dir = REPO_ROOT / "target" / "dev-app-control"
    cargo_cmd = [
        "/usr/bin/env",
        "LOOPFLOW_BUILD_PROVENANCE=development",
        "cargo",
        "build",
        "--locked",
        "--bin",
        "lf",
        "--target-dir",
        str(target_dir),
    ]
    run(cargo_cmd, cwd=REPO_ROOT)
    source = target_dir / "debug" / "lf"
    if not source.exists():
        raise RuntimeError(f"Missing built binary: {source}")
    return source


def _copy_bundled_tools(app_macos_dir: Path) -> None:
    shutil.copy(_build_control_tools(), app_macos_dir / "lf")


def cmd_prepare_control_tools() -> int:
    """Prepare the same control binary the app verifies and copies."""
    _build_control_tools()
    return 0


def cmd_bundle_control_tools(output: Path) -> int:
    """Build control tools into an Xcode app bundle."""
    output.mkdir(parents=True, exist_ok=True)
    _copy_bundled_tools(output)
    return 0


# --- Screenshots ---


def cmd_screenshots() -> int:
    """Generate app screenshots."""
    script = REPO_ROOT / "scripts" / "generate_screenshots.py"
    result = subprocess.run([sys.executable, str(script)], cwd=REPO_ROOT)
    return result.returncode


# --- Main ---


COMMANDS = {
    "setup": (cmd_setup, "Install/check repo dev environment tools"),
    "ffi": (cmd_ffi, "Build the Rust library the app links"),
    "build": (cmd_build, "Build the app"),
    "install": (cmd_install, "Build and install without launching"),
    "test": (cmd_test, "Build and run tests"),
    "ghostty-build": (cmd_ghostty_build, "Build the pinned patched GhosttyKit"),
    "run": (cmd_run, "Build and launch the app"),
    "run-debug": (cmd_run_debug, "Build and run with stdout visible"),
    "release": (cmd_release, "Build release .app and .dmg"),
    "clean": (cmd_clean, "Remove dev app and reset permissions"),
    "xcode": (cmd_xcode, "Open in Xcode"),
    "logs": (cmd_logs, "Tail the app logs"),
    "bundle-control-tools": (
        cmd_bundle_control_tools,
        "Build control tools into an app bundle",
    ),
    "prepare-control-tools": (cmd_prepare_control_tools, "Prepare control tools"),
    "screenshots": (cmd_screenshots, "Generate app screenshots"),
}


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Development commands for Loopflow",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    subparsers = parser.add_subparsers(dest="command", metavar="command")

    for name, (func, help_text) in COMMANDS.items():
        sub = subparsers.add_parser(name, help=help_text)
        if name == "run-debug":
            sub.add_argument(
                "--repo",
                type=lambda p: Path(p).expanduser().resolve(),
                default=REPO_ROOT,
                help="Repo the app opens on launch (default: this checkout)",
            )
            sub.add_argument(
                "--world",
                type=lambda p: Path(p).expanduser().resolve(),
                help="lf-world directory: open its repo with its LF_HOME and provider homes",
            )
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
        if name == "ffi":
            sub.add_argument(
                "--release", action="store_true", help="build for `swift build -c release`"
            )
        if name == "bundle-control-tools":
            sub.add_argument(
                "--output",
                type=lambda p: Path(p).expanduser().resolve(),
                required=True,
                help="Contents/MacOS directory that receives lf",
            )
    args = parser.parse_args()

    if not args.command:
        parser.print_help()
        return 0

    func, _ = COMMANDS[args.command]
    if args.command == "run-debug":
        return func(repo=args.repo, world=args.world)
    if args.command == "setup":
        return func(install=args.install, dry_run=args.dry_run)
    if args.command == "bundle-control-tools":
        return func(output=args.output)
    if args.command == "ffi":
        return func(release=args.release)
    return func()


if __name__ == "__main__":
    sys.exit(main())
