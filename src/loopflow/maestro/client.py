"""Client for communicating with maestro daemon."""

import json
import socket
from pathlib import Path
from typing import Optional

from loopflow.maestro.session import Session, SessionStatus


class MaestroClient:
    """Client for sending requests to maestro daemon."""

    def __init__(self, socket_path: Path):
        self.socket_path = socket_path

    def _send_request(self, request: dict) -> dict:
        """Send request to maestro, return response."""
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
            sock.connect(str(self.socket_path))
            sock.sendall(json.dumps(request).encode())
            data = sock.recv(65536)
            return json.loads(data.decode())

    def register(self, session: Session) -> bool:
        """Register a session. Returns success."""
        response = self._send_request({
            "action": "register",
            "session": session.to_dict(),
        })
        return response.get("ok", False)

    def update(self, session_id: str, status: SessionStatus) -> bool:
        """Update session status. Returns success."""
        response = self._send_request({
            "action": "update",
            "session_id": session_id,
            "status": status.value,
        })
        return response.get("ok", False)

    def unregister(self, session_id: str) -> bool:
        """Unregister a session. Returns success."""
        response = self._send_request({
            "action": "unregister",
            "session_id": session_id,
        })
        return response.get("ok", False)

    def list_sessions(self, repo: Optional[Path] = None) -> list[Session]:
        """List active sessions."""
        request = {"action": "list"}
        if repo:
            request["repo"] = str(repo)

        response = self._send_request(request)
        if not response.get("ok"):
            return []

        return [Session.from_dict(s) for s in response.get("sessions", [])]


def connect_maestro() -> MaestroClient | None:
    """Connect to maestro daemon. Returns None if not running."""
    socket_path = Path.home() / ".lf" / "maestro.sock"
    if not socket_path.exists():
        return None

    try:
        client = MaestroClient(socket_path)
        # Test connection
        client.list_sessions()
        return client
    except (ConnectionRefusedError, FileNotFoundError):
        return None
