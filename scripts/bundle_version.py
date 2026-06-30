"""Read and stamp the version used by macOS app bundles."""

from __future__ import annotations

import os
import plistlib
import re
from pathlib import Path


def read_release_version(repo_root: Path) -> str:
    """Use RELEASE_TAG on CI, otherwise the Cargo workspace version."""
    tag = os.environ.get("RELEASE_TAG")
    if tag:
        return tag.lstrip("v")

    cargo_toml = repo_root / "Cargo.toml"
    match = re.search(r'^version = "([^"]+)"', cargo_toml.read_text(), re.MULTILINE)
    if not match:
        raise RuntimeError(f"workspace version not found in {cargo_toml}")
    return match.group(1)


def stamp_bundle_version(info_plist: Path, version: str) -> None:
    data = plistlib.loads(info_plist.read_bytes())
    data["CFBundleShortVersionString"] = version
    data["CFBundleVersion"] = version
    info_plist.write_bytes(plistlib.dumps(data))
