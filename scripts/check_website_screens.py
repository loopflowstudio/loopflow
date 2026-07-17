#!/usr/bin/env python3
"""Fail when a published website capture is stale or lacks provenance."""

from __future__ import annotations

from pathlib import Path

from generate_screenshots import load_manifest, select_screenshots
from website_screens import validate_capture


def main() -> None:
    repo_root = Path(__file__).resolve().parent.parent
    shots = select_screenshots(load_manifest(repo_root / "scripts/screenshots.yaml"), "website")
    errors = []
    for shot in shots:
        if shot.output is None or shot.wave is None or shot.view is None or shot.width is None:
            errors.append(f"{shot.name}: incomplete website capture manifest entry")
            continue
        errors.extend(
            validate_capture(
                repo_root / shot.output,
                expected_wave=shot.wave,
                expected_view=shot.view,
                logical_width=shot.width,
            )
        )
    if errors:
        raise SystemExit("Website capture gate failed:\n- " + "\n- ".join(errors))
    published = sum(1 for shot in shots if shot.output and (repo_root / shot.output).is_file())
    print(f"Website capture gate: {published} published capture(s) current")


if __name__ == "__main__":
    main()
