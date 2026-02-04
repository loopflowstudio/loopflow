from __future__ import annotations

import time

from loopflow.lfd.daemon.registration import _post_json


class ConnectionValidator:
    def __init__(self, base_url: str = "https://loopflow.studio"):
        self.base_url = base_url.rstrip("/")
        self._cache: dict[str, tuple[bool, float]] = {}

    async def validate_connection_token(self, token: str) -> bool:
        """Validate a connection token from a mobile client."""
        if not token:
            return False

        if token in self._cache:
            valid, expires = self._cache[token]
            if time.time() < expires:
                return valid
            del self._cache[token]

        status, data = await _post_json(
            f"{self.base_url}/api/v1/daemons/validate-connection",
            {"connection_token": token},
        )

        if status == 200 and isinstance(data, dict):
            valid = bool(data.get("valid", False))
            expires = data.get("expires_at")
            expiry = (
                float(expires)
                if isinstance(expires, (int, float))
                else time.time() + 60
            )
            self._cache[token] = (valid, expiry)
            return valid

        return False
