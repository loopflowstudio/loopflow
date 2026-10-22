#!/usr/bin/env python3
"""Include virtual-workspace profiles omitted by rust-cache's member discovery."""

import hashlib
import json
from pathlib import Path

import tomllib


def profile_cache_key(manifest: Path) -> str:
    profiles = tomllib.loads(manifest.read_text()).get("profile", {})
    content = json.dumps(profiles, sort_keys=True, separators=(",", ":"))
    return "profiles-" + hashlib.sha256(content.encode()).hexdigest()[:16]


if __name__ == "__main__":
    print(f"key={profile_cache_key(Path(__file__).resolve().parents[1] / 'Cargo.toml')}")
