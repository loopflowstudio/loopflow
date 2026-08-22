#!/usr/bin/env python3
"""Capture deterministic control-room evidence from fixtures.

    uv run python scripts/generate_screenshots.py

Renders the `wave-proof` manifest set offline through the UI test target, so the
same states are comparable across commits. The website's live product images are
a different pipeline: see scripts/capture_screenshots.py.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

import yaml

REPO_ROOT = Path(__file__).resolve().parent.parent
MANIFEST_PATH = REPO_ROOT / "scripts/screenshots.yaml"
OUTPUT_DIR = REPO_ROOT / "docs/screenshots"
LOG_DIR = REPO_ROOT / "scratch/screenshot-logs"

XCODE_LOCAL_SIGNING = [
    "CODE_SIGNING_ALLOWED=YES",
    "CODE_SIGNING_REQUIRED=YES",
    "CODE_SIGN_STYLE=Manual",
    "CODE_SIGN_IDENTITY=-",
    "DEVELOPMENT_TEAM=",
]
XCODE_DERIVED_DATA = ".build/xcode-derived-data"
TIMEOUT = 120
# Fixtures render immediately; this only lets the window settle.
DELAY = 2


@dataclass(frozen=True)
class WaveProofShot:
    """One fixture-rendered control-room state at one width."""

    name: str
    mode: str
    width: int
    view: str | None
    wave: str | None
    detail_state: str | None


def load_shots(path: Path = MANIFEST_PATH) -> list[WaveProofShot]:
    data = yaml.safe_load(path.read_text())
    return [
        WaveProofShot(
            name=raw["name"],
            mode=raw["mode"],
            width=raw["width"],
            view=raw.get("view"),
            wave=raw.get("wave"),
            detail_state=raw.get("detail_state"),
        )
        for raw in data["wave-proof"]
    ]


def shot_environment(shot: WaveProofShot) -> dict[str, str]:
    env = {
        **os.environ,
        "LOOPFLOW_UI_TEST_NAME": shot.name,
        "LOOPFLOW_UI_TEST_MODE": shot.mode,
        "LOOPFLOW_UI_TEST_WIDTH": str(shot.width),
        "LOOPFLOW_UI_TEST_DELAY": str(DELAY),
    }
    if shot.wave:
        env["LOOPFLOW_UI_TEST_SELECT_BRANCH"] = shot.wave
    if shot.view:
        env["LOOPFLOW_UI_TEST_VIEW"] = shot.view
    if shot.detail_state:
        env["LOOPFLOW_UI_TEST_DETAIL_STATE"] = shot.detail_state
    return env


def _write_log(
    path: Path,
    args: list[str],
    result: subprocess.CompletedProcess,
    duration: float,
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "\n".join(
            [
                f"time: {datetime.now().isoformat()}",
                f"args: {' '.join(args)}",
                f"returncode: {result.returncode}",
                f"duration_seconds: {duration:.2f}",
                "stdout:",
                result.stdout,
                "stderr:",
                result.stderr,
            ]
        )
        + "\n"
    )


def capture(shot: WaveProofShot) -> Path:
    output = OUTPUT_DIR / f"{shot.name}.png"
    output.parent.mkdir(parents=True, exist_ok=True)
    log_path = LOG_DIR / f"{shot.name}.log"
    args = [
        "xcodebuild",
        "-scheme",
        "LoopflowMac",
        "-destination",
        "platform=macOS",
        "-derivedDataPath",
        XCODE_DERIVED_DATA,
        "-disableAutomaticPackageResolution",
        "test",
        "-only-testing:LoopflowUITests/ScreenshotPipelineTests/testCapture",
        *XCODE_LOCAL_SIGNING,
    ]
    started = time.monotonic()
    result = subprocess.run(
        args,
        cwd=REPO_ROOT / "swift",
        env=shot_environment(shot),
        capture_output=True,
        text=True,
        timeout=TIMEOUT,
    )
    _write_log(log_path, args, result, time.monotonic() - started)
    if result.returncode != 0:
        raise RuntimeError(f"{shot.name}: UI test failed; see {log_path}")
    marker = "UI_TEST_SCREENSHOT_PATH="
    source = next(
        (
            Path(line.removeprefix(marker).strip())
            for line in result.stdout.splitlines()
            if line.startswith(marker)
        ),
        None,
    )
    if source is None or not source.is_file():
        raise RuntimeError(f"{shot.name}: UI test screenshot was not created")
    shutil.copy2(source, output)
    return output


def main() -> None:
    shots = load_shots()
    subprocess.run(["xcodegen", "generate"], cwd=REPO_ROOT / "swift", check=True)

    failures = []
    for shot in shots:
        print(f"Capturing {shot.name}...")
        try:
            print(f"  -> {capture(shot)}")
        except (RuntimeError, subprocess.SubprocessError) as exc:
            failures.append(str(exc))
            print(f"  FAIL: {exc}", file=sys.stderr)

    if failures:
        raise SystemExit(f"{len(failures)}/{len(shots)} captures failed")
    print(f"Captured {len(shots)} screenshots.")


if __name__ == "__main__":
    main()
