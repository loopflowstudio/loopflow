#!/usr/bin/env python3
"""Register the redesign waves with lfd."""

from __future__ import annotations

import subprocess
from pathlib import Path

import loopflow.api as loopflow
from loopflow.models import Wave

SCRIPT_ROOT = Path(__file__).resolve().parents[1]
MEMBER_WAVE_NAMES = [
    "chord-model",
    "agent-embedding",
]
REDESIGN_FLOW = "garden-or-silent"
REDESIGN_AREA = [
    "wave/chord-model/",
    "wave/agent-embedding/",
]
WAVE_NAMES = [
    *MEMBER_WAVE_NAMES,
    "redesign",
]


def _resolve_repo_root() -> Path:
    try:
        result = subprocess.run(
            ["git", "rev-parse", "--git-common-dir"],
            cwd=SCRIPT_ROOT,
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return SCRIPT_ROOT

    common_dir = Path(result.stdout.strip())
    if not common_dir.is_absolute():
        common_dir = (SCRIPT_ROOT / common_dir).resolve()
    if common_dir.name == ".git":
        return common_dir.parent
    return SCRIPT_ROOT


REPO_ROOT = _resolve_repo_root()


def _ensure_wave(name: str) -> Wave:
    wave = loopflow.wave(name)
    if wave is None:
        wave = loopflow.create_wave(name, str(REPO_ROOT))
        print(f"{name}: created ({wave.id})")
        return wave

    print(f"{name}: exists ({wave.id})")
    return wave


def _print_summary(redesign: Wave) -> None:
    print("\nredesign")
    print(f"  id: {redesign.id}")
    print(f"  flow: {redesign.primary_flow}")
    print(f"  area: {', '.join(redesign.area) if redesign.area else '-'}")
    print(f"  status: {redesign.status}")


def main() -> int:
    for name in WAVE_NAMES:
        _ensure_wave(name)

    redesign = loopflow.update_wave(
        "redesign",
        flow=REDESIGN_FLOW,
        area=REDESIGN_AREA,
    )
    print(f"redesign: configured flow={REDESIGN_FLOW}, area={', '.join(REDESIGN_AREA)}")

    _print_summary(redesign)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
