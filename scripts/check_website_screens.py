#!/usr/bin/env python3
"""Gate the website deploy on capture provenance.

An image without a parseable provenance sidecar blocks the deploy. A stale
`captured_at` (older than 14 days) only warns: docs and website changes ship
even when nobody has recaptured lately.
"""

from __future__ import annotations

from website_screens import REPO_ROOT, load_captures, validate_capture


def main() -> None:
    shots = load_captures()
    errors: list[str] = []
    warnings: list[str] = []
    for shot in shots:
        shot_errors, shot_warnings = validate_capture(REPO_ROOT / shot.output)
        errors.extend(shot_errors)
        warnings.extend(shot_warnings)
    for warning in warnings:
        print(f"WARNING: stale website capture (deploy continues): {warning}")
    if errors:
        raise SystemExit("Website capture gate failed:\n- " + "\n- ".join(errors))
    published = sum(1 for shot in shots if (REPO_ROOT / shot.output).is_file())
    freshness = "stale but present" if warnings else "current"
    print(f"Website capture gate: {published} published capture(s) {freshness}")


if __name__ == "__main__":
    main()
