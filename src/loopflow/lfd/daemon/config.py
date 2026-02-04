from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any

import yaml


DEFAULT_AUTH_BASE_URL = "https://loopflow.studio"


@dataclass
class AuthConfig:
    provider: str | None = None
    base_url: str = DEFAULT_AUTH_BASE_URL


@dataclass
class LfdConfig:
    auth: AuthConfig = AuthConfig()


def load_lfd_config() -> LfdConfig:
    """Load lfd config from ~/.lf/lfd.yaml."""
    config_path = Path.home() / ".lf" / "lfd.yaml"
    if not config_path.exists():
        return LfdConfig()

    try:
        data: dict[str, Any] = yaml.safe_load(config_path.read_text()) or {}
    except Exception:
        return LfdConfig()

    auth_data = data.get("auth") or {}
    if not isinstance(auth_data, dict):
        auth_data = {}

    provider = auth_data.get("provider")
    base_url = auth_data.get("base_url") or DEFAULT_AUTH_BASE_URL

    return LfdConfig(
        auth=AuthConfig(
            provider=provider if isinstance(provider, str) else None,
            base_url=base_url if isinstance(base_url, str) else DEFAULT_AUTH_BASE_URL,
        )
    )
