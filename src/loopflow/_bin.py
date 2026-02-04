"""Entry points that optionally dispatch to bundled Rust binaries."""

from __future__ import annotations

import os
import platform
import sys
from pathlib import Path


def _target_triple() -> str | None:
    system = sys.platform
    machine = platform.machine().lower()

    if system == "darwin":
        if machine in ("x86_64", "amd64"):
            return "x86_64-apple-darwin"
        if machine in ("arm64", "aarch64"):
            return "aarch64-apple-darwin"
    elif system == "linux":
        if machine in ("x86_64", "amd64"):
            return "x86_64-unknown-linux-gnu"
        if machine in ("arm64", "aarch64"):
            return "aarch64-unknown-linux-gnu"

    return None


def _find_bundled_binary(name: str) -> Path | None:
    pkg_dir = Path(__file__).resolve().parent
    direct = pkg_dir / "bin" / name
    if direct.exists():
        return direct

    target = _target_triple()
    if not target:
        return None

    candidate = pkg_dir / "bin" / f"{name}-{target}"
    if candidate.exists():
        return candidate

    return None


def _should_use_rust() -> bool:
    return os.environ.get("LF_RUST", "1") != "0"


def main() -> None:
    if _should_use_rust():
        binary = _find_bundled_binary("lf")
        if binary:
            os.execv(str(binary), [str(binary)] + sys.argv[1:])

    from loopflow.lf.cli import main as lf_main

    lf_main()


def daemon() -> None:
    from loopflow.lfd.cli import main as lfd_main

    lfd_main()
