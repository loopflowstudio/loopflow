"""Data structures and logging for lf step execution."""

import json
import socket
from pathlib import Path
from typing import Any

from loopflow.lfd.models import StepRun, StepRunStatus

SOCKET_PATH = Path.home() / ".lf" / "lfd.sock"

# StepRun logging (fire-and-forget communication with lfd daemon)


def _send_fire_and_forget(method: str, params: dict[str, Any]) -> None:
    """Send a request to lfd without waiting for response. Fails silently."""
    try:
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.settimeout(0.5)
        sock.connect(str(SOCKET_PATH))
        request = json.dumps({"method": method, "params": params}) + "\n"
        sock.sendall(request.encode())
        sock.close()
    except Exception:
        pass  # Fire-and-forget: don't block on errors


def log_step_run_start(step_run: StepRun) -> None:
    """Tell lfd a step run started. Fire-and-forget."""
    _send_fire_and_forget("sessions.start", {"session": step_run.to_dict()})


def log_step_run_end(step_run_id: str, status: StepRunStatus) -> None:
    """Tell lfd a step run ended. Fire-and-forget."""
    _send_fire_and_forget("sessions.end", {"session_id": step_run_id, "status": status.value})
