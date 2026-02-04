from __future__ import annotations

import socket
import uuid
from pathlib import Path


def get_machine_id() -> str:
    """Get or create persistent machine identifier."""
    machine_id_path = Path.home() / ".lf" / "machine_id"

    if machine_id_path.exists():
        return machine_id_path.read_text().strip()

    machine_id = str(uuid.uuid4())
    machine_id_path.parent.mkdir(parents=True, exist_ok=True)
    machine_id_path.write_text(machine_id)
    return machine_id


def get_machine_name() -> str:
    """Get human-readable machine name."""
    return socket.gethostname()
