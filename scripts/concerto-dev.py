#!/usr/bin/env python3
"""Development commands for Concerto (Swift app) and GhosttyKit.

Usage:
    uv run python scripts/concerto-dev.py <command>

Commands:
    setup           Install/check repo dev environment tools
    build           Build the app
    test            Build and run tests
    run             Build and launch the app
    run-debug       Build and run with stdout visible (default: bundled lfd only)
    run-debug --with-lfd
                    Also run one-shot local lfd from this branch (for daemon debugging)
    run-ios         Build and launch in iOS Simulator
    run-ios --device "iPad Pro 13-inch (M4)"
                    Target a specific simulator device
    release         Build release .app and .dmg (delegates to release-concerto.py)
    clean           Remove dev app and reset permissions
    xcode           Open in Xcode
    logs            Tail the app logs
    lfd             Stop installed lfd and run from this branch (native/sqlite)
    lfd -k          Aggressive preflight kill before starting lfd
    lfd --docker    Run lfd with Docker executor (postgres in container)
    agent-image     Build the Docker agent image
    sandbox-dind    Validate Docker Sandbox lifecycle from inside bundled lfd container

    screenshots     Generate app screenshots
    ghostty-build   Build GhosttyKit xcframework locally
    ghostty-update  Build, upload to R2, and update Package.swift

Streaming logs (long-running commands):
    ~/.lf/logs/dev/<repo>.lfd.log
    ~/.lf/logs/dev/<repo>.concerto-run-debug.log
"""

import argparse
import json
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import TextIO

REPO_ROOT = Path(__file__).parent.parent
SWIFT_DIR = REPO_ROOT / "swift"
GHOSTTY_DIR = REPO_ROOT / "vendor" / "ghostty"
DEV_APP = Path.home() / "Applications" / "Concerto Dev.app"
R2_URL = "https://bin.loopflow.studio"
ENV_SETUP = REPO_ROOT / ".lf" / "env-setup.sh"
DEV_LOG_DIR = Path.home() / ".lf" / "logs" / "dev"
LFD_STREAM_LOG = DEV_LOG_DIR / f"{REPO_ROOT.name}.lfd.log"
CONCERTO_STREAM_LOG = DEV_LOG_DIR / f"{REPO_ROOT.name}.concerto-run-debug.log"


def run(cmd: list[str], cwd: Path | None = None, check: bool = True) -> subprocess.CompletedProcess:
    """Run a command, optionally checking return code."""
    return subprocess.run(cmd, cwd=cwd, check=check)


def run_capture(cmd: list[str], cwd: Path | None = None) -> subprocess.CompletedProcess:
    """Run a command and capture output."""
    return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)


def stream_with_log(
    cmd: list[str],
    *,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
    log_path: Path,
) -> int:
    """Run a long-lived command, streaming stdout and teeing to a known log file."""
    DEV_LOG_DIR.mkdir(parents=True, exist_ok=True)
    with log_path.open("a", encoding="utf-8") as log_file:
        log_file.write(f"\n\n=== {time.strftime('%Y-%m-%d %H:%M:%S')} ===\n")
        log_file.write(f"$ {' '.join(cmd)}\n\n")
        log_file.flush()

        process = subprocess.Popen(
            cmd,
            cwd=cwd,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
        )

        try:
            if process.stdout is None:
                return process.wait()
            for line in process.stdout:
                print(line, end="")
                log_file.write(line)
                log_file.flush()
        except KeyboardInterrupt:
            print("\nStopping process...")
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
        return process.wait()


def run_app_bundle_with_log(app_path: Path, log_path: Path) -> int:
    """Launch app bundle through LaunchServices and stream redirected stdout/stderr."""
    DEV_LOG_DIR.mkdir(parents=True, exist_ok=True)

    open_cmd = [
        "open",
        "-n",
        "-W",
        "--stdout",
        str(log_path),
        "--stderr",
        str(log_path),
        str(app_path),
    ]
    with log_path.open("a", encoding="utf-8") as log_file:
        log_file.write(f"\n\n=== {time.strftime('%Y-%m-%d %H:%M:%S')} ===\n")
        log_file.write(f"$ {' '.join(open_cmd)}\n\n")
        log_file.flush()

    tail_process = subprocess.Popen(["tail", "-n", "0", "-F", str(log_path)])
    try:
        return run(open_cmd, check=False).returncode
    except KeyboardInterrupt:
        print("\nStopping app...")
        _stop_concerto_app(app_path)
        return 130
    finally:
        if tail_process.poll() is None:
            tail_process.terminate()
            try:
                tail_process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                tail_process.kill()


def _stop_concerto_app(app_path: Path) -> None:
    """Quit Concerto Dev, falling back to killing the app if a modal blocks quit."""
    executable = app_path / "Contents" / "MacOS" / "Concerto"
    bundled_lfd = app_path / "Contents" / "MacOS" / "lfd"

    run(["osascript", "-e", 'tell application id "com.loopflow.concerto" to quit'], check=False)
    if _wait_for_process_exit(str(executable), timeout_seconds=3):
        return

    print("App did not quit cleanly; forcing shutdown...")
    run(["pkill", "-f", str(executable)], check=False)
    run(["pkill", "-f", f"{bundled_lfd} serve"], check=False)
    _wait_for_process_exit(str(executable), timeout_seconds=2)


def _wait_for_process_exit(pattern: str, timeout_seconds: float) -> bool:
    deadline = time.time() + timeout_seconds
    while time.time() < deadline:
        result = run_capture(["pgrep", "-f", pattern])
        if result.returncode != 0:
            return True
        time.sleep(0.1)
    return False


def _read_session_token() -> str | None:
    token_path = Path.home() / ".lf" / "session-token"
    if not token_path.exists():
        return None
    token = token_path.read_text(encoding="utf-8").strip()
    return token or None


def _status_ok(token: str | None) -> bool:
    request = urllib.request.Request("http://127.0.0.1:2486/status")
    if token:
        request.add_header("Authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(request, timeout=1.5) as response:
            return response.status == 200
    except (urllib.error.URLError, TimeoutError):
        return False


def _wait_for_lfd_ready(timeout_seconds: float = 20.0) -> bool:
    deadline = time.time() + timeout_seconds
    while time.time() < deadline:
        if _status_ok(_read_session_token()):
            return True
        time.sleep(0.25)
    return False


def _resolve_lfd_sqlite_path(db_path: str) -> Path:
    candidate = Path(db_path)
    if candidate.is_absolute():
        return candidate
    return Path.home() / ".lf" / candidate


def _bundled_lfd_sqlite_path() -> Path:
    return Path.home() / "Library" / "Application Support" / "Concerto" / "lfd" / "concerto.db"


def _remove_sqlite_database(db_path: Path) -> None:
    removed_any = False
    for suffix in ("", "-wal", "-shm"):
        candidate = Path(f"{db_path}{suffix}")
        if candidate.exists():
            candidate.unlink()
            removed_any = True
    if removed_any:
        print(f"Reset sqlite DB: {db_path}")


def _reset_run_debug_databases(with_lfd: bool) -> None:
    _remove_sqlite_database(_bundled_lfd_sqlite_path())
    if with_lfd:
        db_path = os.environ.get("LFD_DB_PATH", f"lfd-{REPO_ROOT.name}.db")
        _remove_sqlite_database(_resolve_lfd_sqlite_path(db_path))


def _start_lfd_background(docker: bool = False) -> tuple[subprocess.Popen[str], TextIO]:
    if docker:
        raise RuntimeError("run-debug --with-lfd currently supports native lfd only")

    env = os.environ.copy()
    env["GRPC_ENABLE_FORK_SUPPORT"] = "0"
    env["GRPC_VERBOSITY"] = "ERROR"
    env.setdefault("LFD_DB_PATH", f"lfd-{REPO_ROOT.name}.db")
    env["RUST_LOG"] = "loopflow=debug,tower_http=debug"

    print("Building lfd...")
    result = run(["cargo", "build", "--bin", "lfd"], cwd=REPO_ROOT, check=False)
    if result.returncode != 0:
        raise RuntimeError("Failed to build lfd")

    lfd_bin = str(REPO_ROOT / "target" / "debug" / "lfd")
    DEV_LOG_DIR.mkdir(parents=True, exist_ok=True)
    log_file = LFD_STREAM_LOG.open("a", encoding="utf-8")
    log_file.write(f"\n\n=== {time.strftime('%Y-%m-%d %H:%M:%S')} ===\n")
    log_file.write(f"$ {lfd_bin} serve\n\n")
    log_file.flush()

    print(f"Using sqlite DB: ~/.lf/{env['LFD_DB_PATH']}")
    print(f"Stream log: {LFD_STREAM_LOG}")
    print("Starting lfd from this branch (debug logging enabled)...")
    process = subprocess.Popen(
        [lfd_bin, "serve"],
        cwd=REPO_ROOT,
        env=env,
        stdout=log_file,
        stderr=subprocess.STDOUT,
        text=True,
    )
    return process, log_file


def _stop_process(process: subprocess.Popen[str], name: str) -> None:
    if process.poll() is not None:
        return
    print(f"Stopping {name}...")
    process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        print(f"Force-killing {name}...")
        process.kill()


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


def _list_worktree_paths(repo_root: Path) -> list[Path]:
    """List worktree paths from `git worktree list --porcelain`."""
    result = run_capture(["git", "-C", str(repo_root), "worktree", "list", "--porcelain"])
    if result.returncode != 0:
        return []

    worktrees: list[Path] = []
    for line in result.stdout.splitlines():
        if line.startswith("worktree "):
            worktrees.append(Path(line.split(" ", 1)[1]))
    return worktrees


def _seed_waves_from_wave_dir() -> None:
    """Create idle waves in lfd for each subdirectory in wave/."""
    wave_dir = REPO_ROOT / "wave"
    if not wave_dir.is_dir():
        return

    wave_names = sorted(
        d.name for d in wave_dir.iterdir() if d.is_dir() and not d.name.startswith(".")
    )
    if not wave_names:
        return

    import loopflow.api as loopflow

    try:
        existing = {w.name for w in loopflow.waves(repo=str(REPO_ROOT))}
    except Exception:
        print("Warning: could not list waves from lfd, skipping wave seed")
        return

    seeded = 0
    for wave_name in wave_names:
        if wave_name in existing:
            continue
        try:
            loopflow.create_wave(wave_name, repo=str(REPO_ROOT))
            print(f"  Seeded wave: {wave_name}")
            seeded += 1
        except Exception as exc:
            print(f"  Wave {wave_name}: {exc}")

    if seeded:
        print(f"Seeded {seeded} wave(s) from wave/")


def _seed_worktrees_background() -> None:
    """Wait for bundled lfd to start, then seed worktree waves."""
    import threading

    def _seed():
        if not _wait_for_lfd_ready(timeout_seconds=30.0):
            return
        try:
            _seed_waves_from_wave_dir()
        except Exception as exc:
            print(f"Warning: worktree seeding failed: {exc}")

    thread = threading.Thread(target=_seed, daemon=True)
    thread.start()


def cmd_run_debug(with_lfd: bool = False, docker_lfd: bool = False) -> int:
    """Build and run with stdout visible."""
    lfd_process: subprocess.Popen[str] | None = None
    lfd_log: TextIO | None = None

    if with_lfd:
        _stop_installed_lfd()
        if docker_lfd:
            print("run-debug --docker-lfd is not supported yet")
            return 1
    _reset_run_debug_databases(with_lfd=with_lfd)

    if with_lfd:
        try:
            lfd_process, lfd_log = _start_lfd_background(docker=False)
        except RuntimeError as error:
            print(str(error))
            return 1

        if not _wait_for_lfd_ready():
            print("lfd did not become ready in time. Check:")
            print(f"  {LFD_STREAM_LOG}")
            if lfd_process is not None:
                _stop_process(lfd_process, "lfd")
            if lfd_log is not None:
                lfd_log.close()
            return 1

    if with_lfd:
        _seed_waves_from_wave_dir()

    print("Building and running Concerto (debug mode with logs)...")
    result = run(["swift", "build"], cwd=SWIFT_DIR, check=False)
    if result.returncode != 0:
        if lfd_process is not None:
            _stop_process(lfd_process, "lfd")
        if lfd_log is not None:
            lfd_log.close()
        return result.returncode

    _install_dev_app()
    # Seed worktree waves after bundled lfd starts (background thread waits for ready)
    if not with_lfd:
        _seed_worktrees_background()
    print("Logs: ~/Library/Logs/Concerto/")
    print(f"Stream log: {CONCERTO_STREAM_LOG}")
    print("Press Ctrl+C to quit")
    print("---")
    app_exit = run_app_bundle_with_log(DEV_APP, CONCERTO_STREAM_LOG)

    if lfd_process is not None:
        _stop_process(lfd_process, "lfd")
    if lfd_log is not None:
        lfd_log.close()
    return app_exit


def _find_default_iphone_simulator() -> str:
    """Find the first available iPhone simulator, preferring one already booted."""
    result = run_capture(["xcrun", "simctl", "list", "devices", "available", "-j"])
    if result.returncode != 0:
        return "iPhone 16"

    import json

    try:
        data = json.loads(result.stdout)
    except json.JSONDecodeError:
        return "iPhone 16"

    for runtime, devices in data.get("devices", {}).items():
        if "iOS" not in runtime:
            continue
        for device in devices:
            if device.get("state") == "Booted" and "iPhone" in device.get("name", ""):
                return device["name"]

    for runtime, devices in data.get("devices", {}).items():
        if "iOS" not in runtime:
            continue
        for device in devices:
            if "iPhone" in device.get("name", ""):
                return device["name"]

    return "iPhone 16"


def cmd_run_ios(device: str | None = None) -> int:
    """Build and launch Concerto in the iOS Simulator."""
    device = device or _find_default_iphone_simulator()

    # Regenerate xcode project to pick up current file layout
    print("Generating Xcode project...")
    result = run(["xcodegen", "generate"], cwd=SWIFT_DIR, check=False)
    if result.returncode != 0:
        print("xcodegen failed. Install with: brew install xcodegen")
        return result.returncode

    project = SWIFT_DIR / "LoopflowSwift.xcodeproj"
    destination = f"platform=iOS Simulator,name={device}"

    print(f"Building Concerto for {device}...")
    result = run(
        [
            "xcodebuild",
            "build",
            "-scheme",
            "Concerto",
            "-project",
            str(project),
            "-destination",
            destination,
            "-configuration",
            "Debug",
        ],
        cwd=SWIFT_DIR,
        check=False,
    )
    if result.returncode != 0:
        return result.returncode

    # Find the built .app
    settings = run_capture(
        [
            "xcodebuild",
            "-showBuildSettings",
            "-scheme",
            "Concerto",
            "-project",
            str(project),
            "-destination",
            destination,
            "-configuration",
            "Debug",
        ],
        cwd=SWIFT_DIR,
    )
    app_path = None
    for line in settings.stdout.splitlines():
        line = line.strip()
        if line.startswith("BUILT_PRODUCTS_DIR = "):
            products_dir = line.split(" = ", 1)[1].strip()
            app_path = Path(products_dir) / "Concerto.app"
            break

    if not app_path or not app_path.exists():
        print("Could not locate built Concerto.app in derived data")
        return 1

    # Boot simulator and install
    print(f"Booting {device}...")
    run(["xcrun", "simctl", "boot", device], check=False)  # already booted is fine
    run(["open", "-a", "Simulator"], check=False)

    print("Installing Concerto...")
    result = run(["xcrun", "simctl", "install", "booted", str(app_path)], check=False)
    if result.returncode != 0:
        return result.returncode

    print(f"Launching Concerto on {device}...")
    print("Press Ctrl+C to quit")
    print("---")
    return run(
        ["xcrun", "simctl", "launch", "--console-pty", "booted", "com.loopflow.concerto"],
        check=False,
    ).returncode


def cmd_release() -> int:
    """Build release .app and .dmg. Delegates to scripts/release-concerto.py."""
    script = REPO_ROOT / "scripts" / "release-concerto.py"
    result = run([sys.executable, str(script)], check=False)
    return result.returncode


def cmd_clean() -> int:
    """Remove dev app and reset permissions."""
    print("Removing dev app...")
    if DEV_APP.exists():
        shutil.rmtree(DEV_APP)

    print("Resetting Accessibility permissions...")
    run(["tccutil", "reset", "Accessibility", "com.loopflow.concerto"], check=False)
    print("Resetting Automation permissions...")
    run(["tccutil", "reset", "AppleEvents", "com.loopflow.concerto"], check=False)
    print("Resetting Microphone permissions...")
    run(["tccutil", "reset", "Microphone", "com.loopflow.concerto"], check=False)
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


def cmd_lfd(docker: bool = False, kill: bool = False) -> int:
    """Stop installed lfd and run from this branch."""
    _stop_installed_lfd()

    if kill:
        print("Preflight kill complete; starting lfd from this branch...")

    if docker:
        return _lfd_docker()
    return _lfd_native()


def _stop_installed_lfd() -> None:
    """Stop any running lfd (launchd, pid file, compose, port)."""
    label = "com.loopflow.lfd"
    plist = Path.home() / "Library" / "LaunchAgents" / f"{label}.plist"
    domain_label = f"gui/{os.getuid()}/{label}"

    # bootout fully detaches the job from launchd ("spawn scheduled" can keep
    # clobbering ~/.lf/session-token even when the daemon itself fails).
    launchd_status = run_capture(["launchctl", "print", domain_label])
    if launchd_status.returncode == 0:
        print("Stopping lfd launchd service...")
        run(["launchctl", "bootout", domain_label], check=False)

    # Some environments can keep the label registered after bootout.
    if run_capture(["launchctl", "print", domain_label]).returncode == 0:
        print("Forcing launchd label removal for lfd...")
        run(["launchctl", "remove", label], check=False)

    if plist.exists():
        disabled = plist.with_suffix(".plist.disabled")
        if disabled.exists():
            print(f"Removing active launchd plist at {plist} (already disabled once)...")
            plist.unlink(missing_ok=True)
        else:
            print(f"Disabling launchd plist at {plist}...")
            plist.rename(disabled)

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
    _stop_lfd_on_port(2486)


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
        result = run(
            [
                "docker",
                "run",
                "-d",
                "--name",
                pg_container,
                "-p",
                "5432:5432",
                "-e",
                "POSTGRES_USER=lfd",
                "-e",
                "POSTGRES_PASSWORD=lfd",
                "-e",
                "POSTGRES_DB=lfd",
                "--health-cmd",
                "pg_isready -U lfd",
                "--health-interval",
                "2s",
                "--health-retries",
                "10",
                "postgres:16-alpine",
            ],
            check=False,
        )
        if result.returncode != 0:
            return result.returncode

        # Wait for healthy
        print("Waiting for postgres...")
        for _ in range(30):
            time.sleep(1)
            check = run_capture(
                [
                    "docker",
                    "inspect",
                    pg_container,
                    "--format",
                    "{{.State.Health.Status}}",
                ]
            )
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
    env = os.environ.copy()
    env["LFD_MODE"] = "container"
    env["LFD_DATABASE_URL"] = "postgres://lfd:lfd@127.0.0.1:5432/lfd"
    env["LFD_EXECUTOR_CREDENTIALS_MOUNTS"] = "claude,ssh,gitconfig"
    env["RUST_LOG"] = "loopflow=debug,tower_http=debug"

    lfd_bin = str(REPO_ROOT / "target" / "debug" / "lfd")
    print(f"Stream log: {LFD_STREAM_LOG}")
    print("Starting lfd (container mode, debug logging)...")
    return stream_with_log([lfd_bin, "serve"], env=env, log_path=LFD_STREAM_LOG)


def _lfd_native() -> int:
    """Build and run lfd natively (sqlite, local executor)."""
    env = os.environ.copy()
    env["GRPC_ENABLE_FORK_SUPPORT"] = "0"
    env["GRPC_VERBOSITY"] = "ERROR"
    # Isolate sqlite state per repo to avoid schema drift across sibling repos.
    env.setdefault("LFD_DB_PATH", f"lfd-{REPO_ROOT.name}.db")

    print("Building lfd...")
    result = run(["cargo", "build", "--bin", "lfd"], cwd=REPO_ROOT, check=False)
    if result.returncode != 0:
        return result.returncode

    env["RUST_LOG"] = "loopflow=debug,tower_http=debug"

    lfd_bin = str(REPO_ROOT / "target" / "debug" / "lfd")
    print(f"Using sqlite DB: ~/.lf/{env['LFD_DB_PATH']}")
    print(f"Stream log: {LFD_STREAM_LOG}")
    print("Starting lfd from this branch (debug logging enabled)...")
    return stream_with_log([lfd_bin, "serve"], env=env, log_path=LFD_STREAM_LOG)


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


def cmd_sandbox_dind(container: str = "lfd-container", workspace: str | None = None) -> int:
    """Validate sandbox CLI lifecycle from inside the bundled lfd container."""

    def run_probe(cmd: list[str]) -> subprocess.CompletedProcess:
        print(f"$ {' '.join(cmd)}")
        result = run_capture(cmd)
        if result.stdout:
            print(result.stdout, end="")
        if result.stderr:
            print(result.stderr, end="")
        return result

    sandbox_name = f"lf-dind-test-{int(time.time())}"
    created = False

    print(f"Validating sandbox lifecycle inside container: {container}")
    version = run_probe(["docker", "exec", container, "docker", "sandbox", "version"])
    if version.returncode != 0:
        return version.returncode

    cli_help = run_probe(["docker", "exec", container, "docker", "sandbox", "--help"])
    if cli_help.returncode != 0:
        return cli_help.returncode
    help_text = f"{cli_help.stdout}\n{cli_help.stderr}"
    missing = [
        command for command in ("create", "exec", "rm", "ls") if command not in help_text.split()
    ]
    if missing:
        print(f"Sandbox CLI missing required commands: {', '.join(missing)}")
        return 1

    workspace_path = workspace
    if not workspace_path:
        pwd_result = run_probe(["docker", "exec", container, "pwd"])
        if pwd_result.returncode != 0:
            return pwd_result.returncode
        workspace_path = pwd_result.stdout.strip() or "/workspace"
    print(f"Using workspace path inside container: {workspace_path}")

    create = run_probe(
        [
            "docker",
            "exec",
            container,
            "docker",
            "sandbox",
            "create",
            "--name",
            sandbox_name,
            "claude",
            workspace_path,
        ]
    )
    if create.returncode != 0:
        return create.returncode
    created = True

    try:
        execute = run_probe(
            [
                "docker",
                "exec",
                container,
                "docker",
                "sandbox",
                "exec",
                sandbox_name,
                "echo",
                "dind works",
            ]
        )
        if execute.returncode == 0:
            print("Detected direct sandbox exec syntax inside container.")
        else:
            legacy_execute = run_probe(
                [
                    "docker",
                    "exec",
                    container,
                    "docker",
                    "sandbox",
                    "exec",
                    sandbox_name,
                    "--",
                    "echo",
                    "dind works",
                ]
            )
            if legacy_execute.returncode == 0:
                print("Detected legacy sandbox exec syntax inside container (-- separator).")
            else:
                return execute.returncode
    finally:
        if created:
            run_probe(["docker", "exec", container, "docker", "sandbox", "rm", sandbox_name])

    print("DinD sandbox validation passed.")
    return 0


def _stop_docker_on_port(port: int) -> None:
    """Stop any Docker containers bound to the given port."""
    result = run_capture(["docker", "ps", "--filter", f"publish={port}", "-q"])
    if result.returncode != 0 or not result.stdout.strip():
        return
    for container_id in result.stdout.strip().splitlines():
        print(f"Stopping Docker container {container_id} on port {port}...")
        run(["docker", "stop", container_id], check=False)


def _stop_lfd_on_port(port: int) -> None:
    """Stop any local process listening on the given port and print wave context."""
    pids = _list_pids_on_port(port)
    if not pids:
        return

    for pid in pids:
        print(_describe_lfd_process(pid, port))

    for pid in pids:
        try:
            print(f"Stopping local process {pid} on port {port}...")
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass

    time.sleep(1.0)

    for pid in _list_pids_on_port(port):
        try:
            print(f"Force-killing local process {pid} on port {port}...")
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass


def _list_pids_on_port(port: int) -> list[int]:
    result = run_capture(["lsof", "-nP", "-iTCP:" + str(port), "-sTCP:LISTEN", "-t"])
    if result.returncode != 0 or not result.stdout.strip():
        return []

    pids: list[int] = []
    for raw in result.stdout.strip().splitlines():
        try:
            pids.append(int(raw.strip()))
        except ValueError:
            continue
    return sorted(set(pids))


def _describe_lfd_process(pid: int, port: int) -> str:
    cmd = run_capture(["ps", "-p", str(pid), "-o", "command="]).stdout.strip()
    repo = _infer_repo_for_pid(pid, cmd)
    branch = _git_branch(repo) if repo else None
    wave = _wave_name_from_branch(branch) if branch else None

    parts = [f"Found local process on :{port}", f"pid={pid}"]
    if wave:
        parts.append(f"wave={wave}")
    if branch:
        parts.append(f"branch={branch}")
    if repo:
        parts.append(f"repo={repo}")
    if cmd:
        parts.append(f"cmd={cmd}")
    return " | ".join(parts)


def _infer_repo_for_pid(pid: int, cmd: str) -> str | None:
    cwd = _cwd_for_pid(pid)
    if cwd and (Path(cwd) / ".git").exists():
        return cwd

    if not cmd:
        return None
    executable = cmd.split()[0]
    try:
        path = Path(executable).resolve()
    except OSError:
        return None

    if path.name != "lfd":
        return None
    # .../<repo>/target/{debug,release}/lfd
    if len(path.parents) >= 3 and path.parents[1].name == "target":
        repo = path.parents[2]
        if (repo / ".git").exists():
            return str(repo)
    return None


def _cwd_for_pid(pid: int) -> str | None:
    result = run_capture(["lsof", "-a", "-p", str(pid), "-d", "cwd", "-Fn"])
    if result.returncode != 0 or not result.stdout.strip():
        return None
    for line in result.stdout.splitlines():
        if line.startswith("n"):
            return line[1:]
    return None


def _git_branch(repo: str) -> str | None:
    result = run_capture(["git", "-C", repo, "branch", "--show-current"])
    branch = result.stdout.strip()
    if result.returncode != 0 or not branch:
        return None
    return branch


def _wave_name_from_branch(branch: str) -> str | None:
    if branch.startswith("jack-heart."):
        parts = branch.split(".")
        if len(parts) > 1 and parts[1]:
            return parts[1]
    if branch.startswith("wave/"):
        wave = branch.removeprefix("wave/").split(".")[0]
        return wave or None
    return None


def _install_dev_app() -> None:
    """Install debug build to stable location for permissions persistence."""
    app_dir = DEV_APP / "Contents"
    (app_dir / "MacOS").mkdir(parents=True, exist_ok=True)
    (app_dir / "Resources").mkdir(parents=True, exist_ok=True)

    shutil.copy(SWIFT_DIR / ".build" / "debug" / "Concerto", app_dir / "MacOS")
    shutil.copy(SWIFT_DIR / "Concerto" / "Info.plist", app_dir)
    shutil.copy(SWIFT_DIR / "Concerto" / "Concerto.sdef", app_dir / "Resources")
    shutil.copy(SWIFT_DIR / "Concerto" / "AppIcon.icns", app_dir / "Resources")
    _copy_bundled_tools(app_dir / "MacOS", profile="debug")

    entitlements = SWIFT_DIR / "Concerto" / "Concerto.entitlements"
    codesign_cmd = ["codesign", "--force", "--deep", "--sign", "-"]
    if entitlements.exists():
        codesign_cmd += ["--entitlements", str(entitlements)]
    codesign_cmd.append(str(DEV_APP))
    run(codesign_cmd)


def _copy_bundled_tools(app_macos_dir: Path, profile: str) -> None:
    if profile == "release":
        cargo_cmd = ["cargo", "build", "--release", "--bin", "lf", "--bin", "lfd"]
        bin_dir = REPO_ROOT / "target" / "release"
    else:
        cargo_cmd = ["cargo", "build", "--bin", "lf", "--bin", "lfd"]
        bin_dir = REPO_ROOT / "target" / "debug"

    result = run(cargo_cmd, cwd=REPO_ROOT, check=False)
    if result.returncode != 0:
        raise RuntimeError("Failed to build bundled lf/lfd binaries")

    for binary in ("lf", "lfd"):
        source = bin_dir / binary
        if not source.exists():
            raise RuntimeError(f"Missing built binary: {source}")
        shutil.copy(source, app_macos_dir / binary)


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
                "git",
                "clone",
                "--depth",
                "1",
                "https://github.com/ghostty-org/ghostty.git",
                str(GHOSTTY_DIR),
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


def _load_r2_credentials() -> tuple[str, str, str] | None:
    """Load R2 credentials from Doppler, falling back to env."""
    # Prefer Doppler — env vars may be stale from old shell sessions
    try:
        result = run_capture(
            [
                "doppler",
                "secrets",
                "download",
                "--no-file",
                "--project",
                "loopflow",
                "--config",
                "prd",
            ]
        )
        if result.returncode == 0:
            secrets = json.loads(result.stdout)
            account_id = secrets.get("R2_ACCOUNT_ID", "").strip()
            access_key = secrets.get("R2_ACCESS_KEY_ID", "").strip()
            secret_key = secrets.get("R2_SECRET_ACCESS_KEY", "").strip()
            if all([account_id, access_key, secret_key]):
                return account_id, access_key, secret_key
    except FileNotFoundError:
        pass

    # Fall back to env
    account_id = os.environ.get("R2_ACCOUNT_ID", "").strip()
    access_key = os.environ.get("R2_ACCESS_KEY_ID", "").strip()
    secret_key = os.environ.get("R2_SECRET_ACCESS_KEY", "").strip()
    if all([account_id, access_key, secret_key]):
        return account_id, access_key, secret_key

    print(
        "R2 credentials not found in Doppler or env "
        "(R2_ACCOUNT_ID, R2_ACCESS_KEY_ID, R2_SECRET_ACCESS_KEY)"
    )
    return None


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
    try:
        import boto3
    except ImportError:
        print("boto3 required: uv pip install boto3")
        return 1

    r2_env = _load_r2_credentials()
    if not r2_env:
        return 1
    account_id, access_key, secret_key = r2_env

    client = boto3.client(
        "s3",
        endpoint_url=f"https://{account_id}.r2.cloudflarestorage.com",
        aws_access_key_id=access_key,
        aws_secret_access_key=secret_key,
        region_name="auto",
    )
    client.upload_file(
        str(zip_path),
        "bin",
        zip_name,
        ExtraArgs={
            "ContentType": "application/zip",
            "CacheControl": "public, max-age=31536000, immutable",
        },
    )
    print(f"Uploaded to {R2_URL}/{zip_name}")

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


def cmd_screenshots() -> int:
    """Generate app screenshots."""
    script = REPO_ROOT / "scripts" / "generate_screenshots.py"
    result = subprocess.run(
        [sys.executable, str(script)],
        cwd=REPO_ROOT,
    )
    return result.returncode


def _update_package_swift(url: str, checksum: str) -> None:
    """Update Package.swift with new URL and checksum."""
    import re

    package_swift = SWIFT_DIR / "Package.swift"
    print(f"Updating {package_swift}...")
    content = package_swift.read_text()

    content = re.sub(
        r'(\.binaryTarget\([^)]*url:\s*")[^"]*(")',
        rf"\g<1>{url}\2",
        content,
    )
    content = re.sub(
        r'(\.binaryTarget\([^)]*checksum:\s*")[^"]*(")',
        rf"\g<1>{checksum}\2",
        content,
    )
    package_swift.write_text(content)


# --- Main ---


COMMANDS = {
    "setup": (cmd_setup, "Install/check repo dev environment tools"),
    "build": (cmd_build, "Build the app"),
    "test": (cmd_test, "Build and run tests"),
    "run": (cmd_run, "Build and launch the app"),
    "run-debug": (cmd_run_debug, "Build and run with stdout visible (bundled lfd default)"),
    "run-ios": (cmd_run_ios, "Build and launch in iOS Simulator"),
    "release": (cmd_release, "Build release .app and .dmg"),
    "clean": (cmd_clean, "Remove dev app and reset permissions"),
    "xcode": (cmd_xcode, "Open in Xcode"),
    "logs": (cmd_logs, "Tail the app logs"),
    "lfd": (cmd_lfd, "Stop local lfd and run from this branch (--docker or -k)"),
    "agent-image": (cmd_agent_image, "Build the Docker agent image"),
    "sandbox-dind": (cmd_sandbox_dind, "Validate Docker Sandbox lifecycle inside lfd-container"),
    "screenshots": (cmd_screenshots, "Generate app screenshots"),
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
            sub.add_argument(
                "-k",
                "--kill",
                action="store_true",
                help="Aggressive preflight kill (default also kills port 2486 listeners)",
            )
        if name == "run-debug":
            mode_group = sub.add_mutually_exclusive_group()
            mode_group.add_argument(
                "--with-lfd",
                action="store_true",
                help="Also run one-shot lfd lifecycle from this branch",
            )
            mode_group.add_argument(
                "--no-lfd",
                action="store_true",
                help="Run UI only (default; bundled lfd managed by Concerto)",
            )
            sub.add_argument(
                "--docker-lfd",
                action="store_true",
                help="Reserved for future container lfd support with --with-lfd",
            )
        if name == "run-ios":
            sub.add_argument(
                "--device",
                type=str,
                default=None,
                help='Simulator device name (default: first iPhone, e.g. "iPad Pro 13-inch (M4)")',
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
        if name == "sandbox-dind":
            sub.add_argument(
                "--container",
                default="lfd-container",
                help="Container name running bundled lfd (default: lfd-container)",
            )
            sub.add_argument(
                "--workspace",
                help="Workspace path to mount in sandbox (defaults to container working directory)",
            )

    args = parser.parse_args()

    if not args.command:
        parser.print_help()
        return 0

    func, _ = COMMANDS[args.command]
    if args.command == "lfd":
        return func(docker=args.docker, kill=args.kill)
    if args.command == "run-debug":
        with_lfd = args.with_lfd
        return func(with_lfd=with_lfd, docker_lfd=args.docker_lfd)
    if args.command == "run-ios":
        return func(device=args.device)
    if args.command == "setup":
        return func(install=args.install, dry_run=args.dry_run)
    if args.command == "sandbox-dind":
        return func(container=args.container, workspace=args.workspace)
    return func()


if __name__ == "__main__":
    sys.exit(main())
