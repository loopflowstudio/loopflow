#!/usr/bin/env python3
"""Fail the deploy when a published website capture is stale or unproven."""

from __future__ import annotations

from website_screens import REPO_ROOT, load_captures, validate_capture


def main() -> None:
    shots = load_captures()
    errors = [error for shot in shots for error in validate_capture(REPO_ROOT / shot.output, shot)]
    if errors:
        raise SystemExit("Website capture gate failed:\n- " + "\n- ".join(errors))
    published = sum(1 for shot in shots if (REPO_ROOT / shot.output).is_file())
    print(f"Website capture gate: {published} published capture(s) current")


if __name__ == "__main__":
    main()
