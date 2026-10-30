"""Loopflow.app contents every build shares: release, dev and `install.py local`."""

from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

# SwiftPM links Sparkle beside the build products; the bundle keeps it in Frameworks.
SPARKLE_FRAMEWORK = "Sparkle.framework"


def embed_frameworks(build_dir: Path, contents_dir: Path, executable: str) -> None:
    """Copy Sparkle into the bundle and point the app executable's rpath at it."""
    source = build_dir / SPARKLE_FRAMEWORK
    if not source.is_dir():
        raise RuntimeError(f"Missing built {SPARKLE_FRAMEWORK}: {source}")
    frameworks = contents_dir / "Frameworks"
    frameworks.mkdir(exist_ok=True)
    # Framework bundles are symlink trees; copying their targets breaks the signature.
    shutil.copytree(source, frameworks / SPARKLE_FRAMEWORK, symlinks=True)
    subprocess.run(
        [
            "install_name_tool",
            "-add_rpath",
            "@executable_path/../Frameworks",
            str(contents_dir / "MacOS" / executable),
        ],
        check=True,
    )
