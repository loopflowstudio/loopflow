#!/usr/bin/env python3
"""Capture deterministic control-room evidence from built worlds and fixtures.

    uv run python scripts/generate_screenshots.py

Renders the `wave-proof` manifest set so the same states are comparable across
commits. Each shot launches the Xcode-built app, which renders its own window to
a PNG (`LOOPFLOW_UI_TEST_SNAPSHOT_PATH`) and exits: no Screen Recording or
Automation permission, and other windows on the display never appear. A shot
naming a `world` reads an `lf-world` recipe through the app's bundled `lf`; the
rest use fixture modes.
The website's live product images are a different pipeline: see
scripts/capture_screenshots.py.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
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
APP_EXECUTABLE = (
    REPO_ROOT
    / "swift"
    / XCODE_DERIVED_DATA
    / "Build/Products/Debug/Loopflow.app/Contents/MacOS/Loopflow"
)
TIMEOUT = 120
HEIGHT = 900
# Fixtures render immediately; this only lets the window settle. World shots
# read through the bundled lf, like the website's live captures.
DELAY = 2
WORLD_DELAY = 8


@dataclass(frozen=True)
class WaveProofShot:
    """One fixture-rendered control-room state at one width."""

    name: str
    mode: str
    width: int
    view: str | None
    wave: str | None
    detail_state: str | None
    world: str | None


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
            world=raw.get("world"),
        )
        for raw in data["wave-proof"]
    ]


def shot_environment(shot: WaveProofShot, worlds: dict[str, Path], output: Path) -> dict[str, str]:
    env = {
        **os.environ,
        "LOOPFLOW_UI_TEST_MODE": shot.mode,
        "LOOPFLOW_UI_TEST_WIDTH": str(shot.width),
        "LOOPFLOW_UI_TEST_HEIGHT": str(HEIGHT),
        "LOOPFLOW_UI_TEST_DELAY": str(WORLD_DELAY if shot.world else DELAY),
        "LOOPFLOW_UI_TEST_SNAPSHOT_PATH": str(output),
    }
    if shot.wave:
        env["LOOPFLOW_UI_TEST_SELECT_BRANCH"] = shot.wave
    if shot.view:
        env["LOOPFLOW_UI_TEST_VIEW"] = shot.view
    if shot.detail_state:
        env["LOOPFLOW_UI_TEST_DETAIL_STATE"] = shot.detail_state
    if shot.world:
        env.update(json.loads((worlds[shot.world] / "env.json").read_text()))
    return env


def build_worlds(recipes: set[str], root: Path) -> dict[str, Path]:
    """Build each named `lf-world` recipe once into `root`."""
    if not recipes:
        return {}
    subprocess.run(
        ["cargo", "build", "-p", "loopflow", "--bin", "lf-world"], cwd=REPO_ROOT, check=True
    )
    binary = REPO_ROOT / "target/debug/lf-world"
    worlds = {}
    for recipe in sorted(recipes):
        worlds[recipe] = root / recipe
        subprocess.run(
            [str(binary), "build", recipe, str(worlds[recipe])],
            check=True,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
        )
    return worlds


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


def build_app() -> None:
    """Build the app; its post-build step bundles this checkout's lf."""
    log_path = LOG_DIR / "build.log"
    args = [
        "xcodebuild",
        "-project",
        "LoopflowSwift.xcodeproj",
        "-scheme",
        "LoopflowMac",
        "-destination",
        "platform=macOS",
        "-derivedDataPath",
        XCODE_DERIVED_DATA,
        "-disableAutomaticPackageResolution",
        "build",
        *XCODE_LOCAL_SIGNING,
    ]
    started = time.monotonic()
    result = subprocess.run(args, cwd=REPO_ROOT / "swift", capture_output=True, text=True)
    _write_log(log_path, args, result, time.monotonic() - started)
    if result.returncode != 0:
        raise SystemExit(f"app build failed; see {log_path}")


def capture(shot: WaveProofShot, worlds: dict[str, Path]) -> Path:
    output = OUTPUT_DIR / f"{shot.name}.png"
    output.parent.mkdir(parents=True, exist_ok=True)
    output.unlink(missing_ok=True)
    repo = worlds[shot.world] / "repo" if shot.world else REPO_ROOT
    args = [str(APP_EXECUTABLE), "--repo", str(repo), "-ui-test-mode", shot.mode]
    started = time.monotonic()
    result = subprocess.run(
        args,
        cwd=repo,
        env=shot_environment(shot, worlds, output),
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
        timeout=TIMEOUT,
    )
    _write_log(LOG_DIR / f"{shot.name}.log", args, result, time.monotonic() - started)
    if not output.is_file() or output.stat().st_size == 0:
        raise RuntimeError(f"{shot.name}: the app rendered no image; see {LOG_DIR / shot.name}.log")
    return output


def main() -> None:
    shots = load_shots()
    subprocess.run(["xcodegen", "generate"], cwd=REPO_ROOT / "swift", check=True)
    build_app()

    failures = []
    with tempfile.TemporaryDirectory(prefix="lf-worlds-") as root:
        worlds = build_worlds({shot.world for shot in shots if shot.world}, Path(root))
        for shot in shots:
            print(f"Capturing {shot.name}...")
            try:
                print(f"  -> {capture(shot, worlds)}")
            except (RuntimeError, subprocess.SubprocessError) as exc:
                failures.append(str(exc))
                print(f"  FAIL: {exc}", file=sys.stderr)

    if failures:
        raise SystemExit(f"{len(failures)}/{len(shots)} captures failed")
    print(f"Captured {len(shots)} screenshots.")


if __name__ == "__main__":
    main()
