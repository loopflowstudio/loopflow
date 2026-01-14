"""launchd plist management for lfd daemon.

The daemon is managed by launchd so it:
- Starts automatically at login
- Restarts if it crashes
- Survives app quit and computer restart
"""

import subprocess
import sys
from pathlib import Path

LABEL = "com.loopflow.lfd"
PLIST_PATH = Path.home() / "Library" / "LaunchAgents" / f"{LABEL}.plist"
LOG_PATH = Path.home() / ".lf" / "logs" / "lfd.log"


def _find_lfd_executable() -> str:
    """Find the lfd executable path."""
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
    """Check if the daemon is currently running."""
    result = subprocess.run(
        ["launchctl", "list", LABEL],
        capture_output=True,
    )
    return result.returncode == 0


def install() -> bool:
    """Install the launchd plist and start the daemon."""
    PLIST_PATH.parent.mkdir(parents=True, exist_ok=True)
    LOG_PATH.parent.mkdir(parents=True, exist_ok=True)

    plist_content = _generate_plist()
    PLIST_PATH.write_text(plist_content)

    result = subprocess.run(
        ["launchctl", "load", str(PLIST_PATH)],
        capture_output=True,
    )
    return result.returncode == 0


def uninstall() -> bool:
    """Stop the daemon and remove the launchd plist."""
    if not PLIST_PATH.exists():
        return True

    subprocess.run(
        ["launchctl", "unload", str(PLIST_PATH)],
        capture_output=True,
    )

    PLIST_PATH.unlink(missing_ok=True)
    return True


def restart() -> bool:
    """Restart the daemon."""
    if not is_installed():
        return install()

    subprocess.run(
        ["launchctl", "unload", str(PLIST_PATH)],
        capture_output=True,
    )

    result = subprocess.run(
        ["launchctl", "load", str(PLIST_PATH)],
        capture_output=True,
    )
    return result.returncode == 0


def get_log_path() -> Path:
    """Get the daemon log file path."""
    return LOG_PATH
