#!/usr/bin/env python3
"""Register the redesign waves with lfd."""

from __future__ import annotations

from typing import Any

import loopflow.api as loopflow

REPO = "."
WAVE_NAMES = [
    "chord-model",
    "clear-the-deck",
    "agent-embedding",
    "signals",
    "redesign",
]


def _ensure_wave(name: str) -> Any:
    wave = loopflow.wave(name)
    if wave is None:
        wave = loopflow.create_wave(name, REPO)
        print(f"{name}: created ({wave.id})")
        return wave

    print(f"{name}: exists ({wave.id})")
    return wave


def _print_summary(redesign: Any) -> None:
    print("\nredesign")
    print(f"  id: {redesign.id}")
    print(f"  flow: {redesign.primary_flow}")
    print(f"  area: {', '.join(redesign.area) if redesign.area else '-'}")
    print(f"  status: {redesign.status}")


def main() -> int:
    redesign = None
    for name in WAVE_NAMES:
        wave = _ensure_wave(name)
        if name == "redesign":
            redesign = wave

    if redesign is None:
        raise RuntimeError("bootstrap created no redesign wave")

    _print_summary(redesign)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
