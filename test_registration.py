#!/usr/bin/env python3
"""Test session registration."""

import uuid
from datetime import datetime
from pathlib import Path

from loopflow.maestro import Session, SessionStatus, connect_maestro

def test_registration():
    """Test registering a session with maestro."""
    maestro = connect_maestro()
    if not maestro:
        print("✗ Could not connect to maestro")
        return

    print("✓ Connected to maestro")

    # Create a test session
    session = Session(
        id=str(uuid.uuid4()),
        task="test_task",
        repo=Path.cwd(),
        worktree=Path.cwd(),
        status=SessionStatus.RUNNING,
        started_at=datetime.now(),
    )

    # Register it
    success = maestro.register(session)
    print(f"Register: {'✓' if success else '✗'}")

    # List sessions
    sessions = maestro.list_sessions()
    print(f"Sessions: {len(sessions)}")
    for s in sessions:
        print(f"  - {s.task} ({s.status.value})")

    # Update status
    success = maestro.update(session.id, SessionStatus.COMPLETED)
    print(f"Update: {'✓' if success else '✗'}")

    # Unregister
    success = maestro.unregister(session.id)
    print(f"Unregister: {'✓' if success else '✗'}")

    # List again
    sessions = maestro.list_sessions()
    print(f"Final sessions: {len(sessions)}")

if __name__ == "__main__":
    test_registration()
