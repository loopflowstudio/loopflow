"""Session logging for lf task execution.

Fire-and-forget communication with lfd daemon.
"""

import json
import socket
from pathlib import Path
from typing import Any

from loopflow.lf.models import Session, SessionStatus

SOCKET_PATH = Path.home() / ".lf" / "lfd.sock"


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


def log_session_start(session: Session) -> None:
    """Tell lfd a session started. Fire-and-forget."""
    _send_fire_and_forget("sessions.start", {"session": session.to_dict()})


def log_session_end(session_id: str, status: SessionStatus) -> None:
    """Tell lfd a session ended. Fire-and-forget."""
    _send_fire_and_forget("sessions.end", {"session_id": session_id, "status": status.value})
