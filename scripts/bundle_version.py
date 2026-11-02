"""Read and stamp the version used by macOS app bundles."""

from __future__ import annotations

import os
import plistlib
import re
import subprocess
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


def read_source_revision(repo_root: Path) -> str:
    """Use GITHUB_SHA on CI, otherwise the checkout's HEAD; `unknown` outside git."""
    sha = os.environ.get("GITHUB_SHA")
    if sha:
        return sha
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=repo_root, capture_output=True, text=True, check=False
    )
    return result.stdout.strip() if result.returncode == 0 else "unknown"


def stamp_bundle_version(info_plist: Path, version: str, revision: str) -> None:
    """Desktop names its version and source revision in every launch and diagnostic."""
    data = plistlib.loads(info_plist.read_bytes())
    data["CFBundleShortVersionString"] = version
    data["CFBundleVersion"] = version
    data["LoopflowSourceRevision"] = revision
    info_plist.write_bytes(plistlib.dumps(data))
