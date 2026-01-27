"""launchd plist management for lfd daemon.

The daemon is managed by launchd so it:
- Starts automatically at login
- Restarts if it crashes
- Survives app quit and computer restart

Uses modern launchctl APIs (bootstrap/bootout/kickstart) for proper
service lifecycle management.
"""

import os
import subprocess
import sys
import time
from pathlib import Path

LABEL = "com.loopflow.lfd"
PLIST_PATH = Path.home() / "Library" / "LaunchAgents" / f"{LABEL}.plist"
LOG_PATH = Path.home() / ".lf" / "logs" / "lfd.log"
PID_PATH = Path.home() / ".lf" / "lfd.pid"


def _get_gui_domain() -> str:
    """Get the launchd GUI domain for current user."""
    return f"gui/{os.getuid()}"


def _get_service_target() -> str:
    """Get the full service target for launchctl commands."""
    return f"{_get_gui_domain()}/{LABEL}"


def _find_lfd_executable() -> str:
    """Find the lfd executable path.

    Always prefer the global installation (~/.local/bin/lfd) over any
    virtualenv or dev installation. This ensures the daemon survives
    worktree switches and dev environment changes.
    """
    global_path = Path.home() / ".local" / "bin" / "lfd"
    if global_path.exists():
        return str(global_path)

    # Fall back to whatever is in PATH (may be a venv)
    result = subprocess.run(["which", "lfd"], capture_output=True, text=True)
    if result.returncode == 0:
        return result.stdout.strip()

    return sys.executable


def _generate_plist() -> str:
    """Generate the launchd plist XML."""
    lfd_path = _find_lfd_executable()

    if lfd_path == sys.executable:
        program_args = f"""    <key>ProgramArguments</key>
    <array>
        <string>{sys.executable}</string>
        <string>-m</string>
        <string>loopflow.lfd</string>
        <string>serve</string>
    </array>"""
    else:
        program_args = f"""    <key>ProgramArguments</key>
    <array>
        <string>{lfd_path}</string>
        <string>serve</string>
    </array>"""

    log_path = str(LOG_PATH)

    return f"""<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
{program_args}
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>ThrottleInterval</key>
    <integer>10</integer>
    <key>ExitTimeOut</key>
    <integer>30</integer>
    <key>StandardOutPath</key>
    <string>{log_path}</string>
    <key>StandardErrorPath</key>
    <string>{log_path}</string>
</dict>
</plist>
"""


def is_installed() -> bool:
    """Check if the launchd plist is installed."""
    return PLIST_PATH.exists()


def is_running() -> bool:
    """Check if the daemon is currently running via launchctl."""
    result = subprocess.run(
        ["launchctl", "print", _get_service_target()],
        capture_output=True,
    )
    return result.returncode == 0


def get_pid() -> int | None:
    """Get daemon PID from PID file, verifying process is alive."""
    if not PID_PATH.exists():
        return None

    try:
        pid = int(PID_PATH.read_text().strip())
        # Check if process is actually running
        os.kill(pid, 0)
        return pid
    except (ValueError, ProcessLookupError, PermissionError):
        # Stale PID file - clean it up
        PID_PATH.unlink(missing_ok=True)
        return None


def write_pid() -> None:
    """Write current process PID to file. Called by daemon on startup."""
    PID_PATH.parent.mkdir(parents=True, exist_ok=True)
    PID_PATH.write_text(str(os.getpid()))


def remove_pid() -> None:
    """Remove PID file. Called by daemon on shutdown."""
    PID_PATH.unlink(missing_ok=True)


def _kill_orphan_lfd(port: int = 8765) -> None:
    """Kill orphan lfd processes on the HTTP port.

    Only kills processes that are actually lfd (checks command line).
    This handles cases where a dev `uv run lfd serve` or crashed daemon
    left an orphan process holding the port.
    """
    # Find PIDs on the port
    result = subprocess.run(
        ["lsof", "-ti", f":{port}"],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0 or not result.stdout.strip():
        return

    for pid_str in result.stdout.strip().split("\n"):
        try:
            pid = int(pid_str)
            # Check if it's actually lfd by reading /proc or ps
            ps_result = subprocess.run(
                ["ps", "-p", str(pid), "-o", "command="],
                capture_output=True,
                text=True,
            )
            if ps_result.returncode == 0:
                cmd = ps_result.stdout.strip()
                # Only kill if it looks like lfd
                if "lfd" in cmd or "loopflow" in cmd:
                    os.kill(pid, 9)
        except (ValueError, ProcessLookupError):
            pass

    time.sleep(0.3)


def install() -> bool:
    """Install the launchd plist and start the daemon.

    Uses bootout/bootstrap for proper service lifecycle.
    If already installed, unloads first to pick up any plist changes.
    Kills any orphan processes on the HTTP port before starting.
    Ensures wt (worktrunk) is available, installing via homebrew if needed.
    """
    from loopflow.lf.worktrees import ensure_wt_available

    # Ensure wt is available (install via homebrew if needed)
    ensure_wt_available()

    PLIST_PATH.parent.mkdir(parents=True, exist_ok=True)
    LOG_PATH.parent.mkdir(parents=True, exist_ok=True)

    # Write new plist first
    plist_content = _generate_plist()
    PLIST_PATH.write_text(plist_content)

    # Bootout to stop the managed service (if running)
    subprocess.run(
        ["launchctl", "bootout", _get_service_target()],
        capture_output=True,
    )
    time.sleep(0.3)

    # Kill any orphan lfd processes on the port (from dev runs, crashed daemons, etc.)
    _kill_orphan_lfd()

    # Bootstrap the service
    result = subprocess.run(
        ["launchctl", "bootstrap", _get_gui_domain(), str(PLIST_PATH)],
        capture_output=True,
    )

    if result.returncode != 0:
        # Bootstrap can fail if KeepAlive already restarted the service
        if is_running():
            return True
        return False

    # Wait for service to start
    for _ in range(20):
        time.sleep(0.2)
        if is_running():
            return True

    return False


def uninstall() -> bool:
    """Stop the daemon and remove the launchd plist."""
    # Get PID before bootout (for fallback kill)
    pid = get_pid()
    if not pid:
        # Try to get PID from launchctl
        result = subprocess.run(["launchctl", "list"], capture_output=True, text=True)
        for line in result.stdout.split("\n"):
            if LABEL in line:
                try:
                    pid = int(line.split()[0])
                except (ValueError, IndexError):
                    pass
                break

    # Bootout from launchd (sends SIGTERM)
    subprocess.run(
        ["launchctl", "bootout", _get_service_target()],
        capture_output=True,
    )

    # Wait for graceful shutdown
    time.sleep(1.0)

    # If process still running, send SIGKILL as fallback
    if pid:
        try:
            os.kill(pid, 0)  # Check if still running
            os.kill(pid, 9)  # SIGKILL
            time.sleep(0.2)
        except ProcessLookupError:
            pass  # Already dead

    PLIST_PATH.unlink(missing_ok=True)
    PID_PATH.unlink(missing_ok=True)
    return True


def restart() -> bool:
    """Restart the daemon with a clean shutdown.

    Uses kickstart -k for a proper restart that sends SIGTERM,
    waits for graceful shutdown, then starts fresh.
    """
    if not is_installed():
        return install()

    # kickstart -k kills the existing process and starts a new one
    result = subprocess.run(
        ["launchctl", "kickstart", "-k", _get_service_target()],
        capture_output=True,
    )
    return result.returncode == 0


def get_log_path() -> Path:
    """Get the daemon log file path."""
    return LOG_PATH
