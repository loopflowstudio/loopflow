#!/usr/bin/env python3
"""Capture the website screenshots from the installed app.

    uv run python scripts/capture_screenshots.py
    uv run python scripts/capture_screenshots.py --executable path/to/Loopflow

Reads the `website:` set from scripts/screenshots.yaml, launches the installed
app once per view, and writes each PNG plus its provenance sidecar
({captured_at, wave, app_version}) into website/static/ in this worktree.
Wave liveness is reported but never blocks: look at the images, then commit
what looks right like any other change.
"""

from __future__ import annotations

import argparse
import json
import subprocess
from dataclasses import asdict
from datetime import datetime, timezone
from pathlib import Path

from website_screens import (
    REPO_ROOT,
    CaptureProvenance,
    CaptureUnavailable,
    capture,
    captured_wave,
    load_captures,
    read_app_version,
    sidecar_path,
    write_json,
)

DEFAULT_EXECUTABLE = Path("/Applications/Loopflow.app/Contents/MacOS/Loopflow")
STATUS_TIMEOUT = 10


def _report_wave_status(lf_binary: Path, wave: str) -> None:
    """Informational only: print what live state we know, then capture anyway."""
    try:
        result = subprocess.run(
            [str(lf_binary), "status", wave, "--json"],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            timeout=STATUS_TIMEOUT,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        print(f"capture: lf status unavailable ({exc}); capturing anyway")
        return
    if result.returncode != 0:
        detail = result.stderr.strip() or "no detail"
        print(f"capture: lf status failed ({detail}); capturing anyway")
        return
    try:
        payload = json.loads(result.stdout)
    except json.JSONDecodeError:
        print("capture: lf status returned invalid JSON; capturing anyway")
        return
    # `lf status` prints `null` for a wave with no registry state.
    if not isinstance(payload, dict):
        print(f"capture: {wave} has no registry state; capturing anyway")
        return
    served = bool((payload.get("wave") or {}).get("live"))
    print(f"capture: {wave} is {'served' if served else 'not served'}")


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        "--executable",
        type=Path,
        default=DEFAULT_EXECUTABLE,
        help=f"App binary to launch (default: {DEFAULT_EXECUTABLE})",
    )
    parser.add_argument(
        "--lf-binary",
        type=Path,
        default=Path("lf"),
        help="lf used only to report wave liveness (default: lf on PATH)",
    )
    args = parser.parse_args()

    shots = load_captures()
    wave = captured_wave(shots)
    _report_wave_status(args.lf_binary, wave)
    app_version = read_app_version(args.executable)
    provenance = CaptureProvenance(
        captured_at=datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        wave=wave,
        app_version=app_version,
    )

    for shot in shots:
        target = REPO_ROOT / shot.output
        staging = target.with_name(f".{target.name}.tmp")
        try:
            capture(shot, executable=args.executable, repo_path=REPO_ROOT, output=staging)
            staging.replace(target)
        finally:
            staging.unlink(missing_ok=True)
        write_json(sidecar_path(target), asdict(provenance))
        print(f"capture: wrote {target.relative_to(REPO_ROOT)}")

    print(
        f"capture: {len(shots)} image(s) from Loopflow {app_version}; "
        "review with `git status`, then commit"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except CaptureUnavailable as exc:
        raise SystemExit(f"capture: {exc}") from exc
