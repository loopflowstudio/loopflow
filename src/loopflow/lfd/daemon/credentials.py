from __future__ import annotations

import json
from pathlib import Path
from typing import Any


def load_jwt() -> str | None:
    """Load JWT from ~/.lf/credentials.json."""
    credentials_path = Path.home() / ".lf" / "credentials.json"
    if not credentials_path.exists():
        return None

    try:
        data: dict[str, Any] = json.loads(credentials_path.read_text())
    except Exception:
        return None

    token = data.get("jwt") or data.get("token")
    if isinstance(token, str) and token.strip():
        return token.strip()
    return None
