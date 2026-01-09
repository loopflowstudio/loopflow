"""Maestro daemon service."""

import json
import socket
from datetime import datetime
from pathlib import Path
from typing import Optional

from loopflow.maestro.session import Session, SessionStatus
from loopflow.maestro.notification import NotificationEvent, NotificationType
from loopflow.maestro.adapters import NotificationAdapter, MacOSAdapter


class Maestro:
    """Background daemon that tracks sessions and dispatches notifications."""

    def __init__(self, socket_path: Path, state_path: Path):
        self.socket_path = socket_path
        self.state_path = state_path
        self.sessions: dict[str, Session] = {}
        self.adapters: list[NotificationAdapter] = [MacOSAdapter()]
        self._load_state()

    def _load_state(self) -> None:
        """Load persisted sessions from disk."""
        if not self.state_path.exists():
            return
        try:
            data = json.loads(self.state_path.read_text())
            self.sessions = {
                sid: Session.from_dict(s) for sid, s in data.items()
            }
        except (json.JSONDecodeError, KeyError):
            # Corrupt state, start fresh
            self.sessions = {}

    def _save_state(self) -> None:
        """Persist sessions to disk."""
        data = {sid: s.to_dict() for sid, s in self.sessions.items()}
        self.state_path.parent.mkdir(parents=True, exist_ok=True)
        self.state_path.write_text(json.dumps(data, indent=2))

    def register_session(self, session: Session) -> None:
        """Register a new session."""
        self.sessions[session.id] = session
        self._save_state()
        self.emit(
            NotificationEvent(
                type=NotificationType.STARTED,
                session_id=session.id,
                message=f"Started {session.task}",
                timestamp=datetime.now(),
                repo=session.repo,
            )
        )

    def update_status(self, session_id: str, status: SessionStatus) -> None:
        """Update session status."""
        if session_id not in self.sessions:
            return
        self.sessions[session_id].status = status
        self._save_state()

        # Emit notification for completion/error
        if status in (SessionStatus.COMPLETED, SessionStatus.ERROR):
            session = self.sessions[session_id]
            msg = f"{session.task} completed" if status == SessionStatus.COMPLETED else f"{session.task} failed"
            self.emit(
                NotificationEvent(
                    type=NotificationType(status.value),
                    session_id=session_id,
                    message=msg,
                    timestamp=datetime.now(),
                    repo=session.repo,
                )
            )

    def unregister_session(self, session_id: str) -> None:
        """Remove a session from tracking."""
        if session_id in self.sessions:
            del self.sessions[session_id]
            self._save_state()

    def list_sessions(self, repo: Optional[Path] = None) -> list[Session]:
        """List active sessions, optionally filtered by repo."""
        sessions = list(self.sessions.values())
        if repo:
            sessions = [s for s in sessions if s.repo == repo]
        return sessions

    def emit(self, event: NotificationEvent) -> None:
        """Dispatch notification to all adapters."""
        for adapter in self.adapters:
            try:
                adapter.send(event)
            except Exception:
                # Don't let adapter failures break the daemon
                pass

    def run(self) -> None:
        """Run the maestro daemon, listening on Unix socket."""
        # Remove stale socket
        if self.socket_path.exists():
            self.socket_path.unlink()

        self.socket_path.parent.mkdir(parents=True, exist_ok=True)

        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
            sock.bind(str(self.socket_path))
            sock.listen()

            while True:
                conn, _ = sock.accept()
                with conn:
                    try:
                        data = conn.recv(65536)
                        if not data:
                            continue

                        request = json.loads(data.decode())
                        response = self._handle_request(request)
                        conn.sendall(json.dumps(response).encode())
                    except Exception as e:
                        conn.sendall(
                            json.dumps({"ok": False, "error": str(e)}).encode()
                        )

    def _handle_request(self, request: dict) -> dict:
        """Handle a client request."""
        action = request.get("action")

        if action == "register":
            session = Session.from_dict(request["session"])
            self.register_session(session)
            return {"ok": True}

        elif action == "update":
            session_id = request["session_id"]
            status = SessionStatus(request["status"])
            self.update_status(session_id, status)
            return {"ok": True}

        elif action == "unregister":
            session_id = request["session_id"]
            self.unregister_session(session_id)
            return {"ok": True}

        elif action == "list":
            repo = Path(request["repo"]) if request.get("repo") else None
            sessions = self.list_sessions(repo)
            return {
                "ok": True,
                "sessions": [s.to_dict() for s in sessions],
            }

        else:
            return {"ok": False, "error": f"Unknown action: {action}"}
