#!/usr/bin/env python3
"""Register the redesign waves with lfd."""

from __future__ import annotations

import loopflow.api as loopflow

REPO = "."
WAVE_NAMES = [
    "chord-model",
    "clear-the-deck",
    "agent-embedding",
    "signals",
    "redesign",
]


def ensure_wave(name: str) -> None:
    existing = loopflow.wave(name)
    if existing is not None:
        print(f"{name}: exists ({existing.id})")
        return

    created = loopflow.create_wave(name, REPO)
    print(f"{name}: created ({created.id})")


def print_summary() -> None:
    redesign = loopflow.wave("redesign")
    if redesign is None:
        raise RuntimeError("bootstrap created no redesign wave")

    print("\nredesign")
    print(f"  id: {redesign.id}")
    print(f"  flow: {redesign.primary_flow}")
    print(f"  area: {', '.join(redesign.area) if redesign.area else '-'}")
    print(f"  status: {redesign.status}")


def main() -> int:
    for name in WAVE_NAMES:
        ensure_wave(name)
    print_summary()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
