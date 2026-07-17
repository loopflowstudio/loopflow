#!/usr/bin/env python3
"""Capture deterministic UI-test evidence and live website product images.

    uv run python scripts/refresh_website_screens.py
    uv run python scripts/generate_screenshots.py --set wave-proof

Live captures launch the real app with ``-ui-test-mode live``. They never
substitute fixtures when the registry or requested view is unavailable. The
refresh wrapper adds provenance and publishes only perceptual changes.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Literal

import yaml
from website_screens import CaptureUnavailable, require_live_wave

XCODE_LOCAL_SIGNING = [
    "CODE_SIGNING_ALLOWED=YES",
    "CODE_SIGNING_REQUIRED=YES",
    "CODE_SIGN_STYLE=Manual",
    "CODE_SIGN_IDENTITY=-",
    "DEVELOPMENT_TEAM=",
]
XCODE_DERIVED_DATA = ".build/xcode-derived-data"


@dataclass(frozen=True)
class Screenshot:
    name: str
    type: Literal["live", "uitest"]
    sets: tuple[str, ...] = ()
    output: Path | None = None
    view: str | None = None
    wave: str | None = None
    width: int | None = None
    height: int | None = None
    delay: float | None = None
    ui_test_mode: str | None = None
    ui_test_detail_state: str | None = None


@dataclass(frozen=True)
class Manifest:
    screenshots: tuple[Screenshot, ...] = field(default_factory=tuple)


def load_manifest(path: Path) -> Manifest:
    data = yaml.safe_load(path.read_text()) or {}
    screenshots = []
    for raw in data.get("screenshots", []):
        shot_type = raw["type"]
        if shot_type not in {"live", "uitest"}:
            raise ValueError(f"{raw['name']}: unsupported screenshot type {shot_type!r}")
        screenshots.append(
            Screenshot(
                name=raw["name"],
                type=shot_type,
                sets=tuple(raw.get("sets", [])),
                output=Path(raw["output"]) if raw.get("output") else None,
                view=raw.get("view"),
                wave=raw.get("wave"),
                width=raw.get("width") or raw.get("ui_test_width"),
                height=raw.get("height"),
                delay=raw.get("delay") or raw.get("ui_test_delay"),
                ui_test_mode=raw.get("ui_test_mode"),
                ui_test_detail_state=raw.get("ui_test_detail_state"),
            )
        )
    return Manifest(tuple(screenshots))


def select_screenshots(manifest: Manifest, set_name: str | None) -> list[Screenshot]:
    if set_name is None:
        return list(manifest.screenshots)
    return [shot for shot in manifest.screenshots if set_name in shot.sets]


def resolve_output_path(
    shot: Screenshot,
    repo_root: Path,
    output_override: Path | None,
) -> Path:
    if output_override is not None:
        base = output_override if output_override.is_absolute() else repo_root / output_override
        return base / f"{shot.name}.png"
    if shot.output is not None:
        return shot.output if shot.output.is_absolute() else repo_root / shot.output
    return repo_root / "docs" / "screenshots" / f"{shot.name}.png"


def find_loopflow_executable(explicit: Path | None, repo_root: Path) -> Path:
    candidates = [
        explicit,
        Path("/Applications/Loopflow.app/Contents/MacOS/Loopflow"),
        repo_root / "local-bin/Loopflow.app/Contents/MacOS/Loopflow",
        repo_root / "swift/.build/release/LoopflowMac",
        repo_root / "swift/.build/debug/LoopflowMac",
    ]
    for candidate in candidates:
        if candidate is not None and candidate.is_file():
            return candidate.resolve()
    raise FileNotFoundError(
        "Loopflow executable not found; promote the app with "
        "`uv run python scripts/install.py local --use` or pass --executable"
    )


def validate_live_state(lf_binary: Path, repo_path: Path, wave: str) -> None:
    result = subprocess.run(
        [str(lf_binary), "status", wave, "--json"],
        cwd=repo_path,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        reason = result.stderr.strip() or "lf status failed"
        raise CaptureUnavailable(reason)
    require_live_wave(json.loads(result.stdout), wave)


def _write_log(
    path: Path | None,
    args: list[str],
    stdout: str,
    stderr: str,
    returncode: int,
    duration: float,
) -> None:
    if path is None:
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "\n".join(
            [
                f"time: {datetime.now().isoformat()}",
                f"args: {' '.join(args)}",
                f"returncode: {returncode}",
                f"duration_seconds: {duration:.2f}",
                "stdout:",
                stdout,
                "stderr:",
                stderr,
            ]
        )
        + "\n"
    )


def live_capture_environment(shot: Screenshot, output_path: Path) -> dict[str, str]:
    if not shot.view or not shot.wave or not shot.width or not shot.height:
        raise ValueError(f"{shot.name}: live captures require view, wave, width, and height")
    env = {
        **os.environ,
        "LOOPFLOW_UI_TEST_MODE": "live",
        "LOOPFLOW_UI_TEST_VIEW": shot.view,
        "LOOPFLOW_UI_TEST_WIDTH": str(shot.width),
        "LOOPFLOW_UI_TEST_HEIGHT": str(shot.height),
        "LOOPFLOW_UI_TEST_DELAY": str(shot.delay or 6),
        "LOOPFLOW_UI_TEST_APPEARANCE": "light",
        "LOOPFLOW_UI_TEST_SNAPSHOT_PATH": str(output_path),
    }
    if shot.view != "roadmap":
        env["LOOPFLOW_UI_TEST_SELECT_BRANCH"] = shot.wave
    return env


def capture_live_screenshot(
    shot: Screenshot,
    *,
    repo_path: Path,
    output_path: Path,
    executable: Path,
    timeout: int,
    log_path: Path | None,
) -> Path:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.unlink(missing_ok=True)
    env = live_capture_environment(shot, output_path)
    args = [str(executable), "--repo", str(repo_path), "-ui-test-mode", "live"]
    started = time.monotonic()
    process = subprocess.Popen(
        args,
        cwd=repo_path,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    try:
        stdout, stderr = process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired as exc:
        process.terminate()
        stdout, stderr = process.communicate(timeout=5)
        _write_log(
            log_path,
            args,
            stdout,
            stderr,
            process.returncode or -1,
            time.monotonic() - started,
        )
        raise TimeoutError(f"{shot.name}: live capture timed out after {timeout}s") from exc
    _write_log(log_path, args, stdout, stderr, process.returncode, time.monotonic() - started)
    if not output_path.is_file() or output_path.stat().st_size == 0:
        detail = stderr.strip() or stdout.strip() or "the requested view did not render"
        raise RuntimeError(f"{shot.name}: live capture missing: {detail}")
    return output_path


def capture_ui_test_screenshot(
    shot: Screenshot,
    *,
    repo_root: Path,
    output_path: Path,
    timeout: int,
    log_path: Path | None,
) -> Path:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    env = {**os.environ, "LOOPFLOW_UI_TEST_NAME": shot.name}
    if shot.ui_test_mode:
        env["LOOPFLOW_UI_TEST_MODE"] = shot.ui_test_mode
    if shot.wave:
        env["LOOPFLOW_UI_TEST_SELECT_BRANCH"] = shot.wave
    if shot.ui_test_detail_state:
        env["LOOPFLOW_UI_TEST_DETAIL_STATE"] = shot.ui_test_detail_state
    if shot.width:
        env["LOOPFLOW_UI_TEST_WIDTH"] = str(shot.width)
    if shot.delay:
        env["LOOPFLOW_UI_TEST_DELAY"] = str(shot.delay)

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
        cwd=repo_root / "swift",
        env=env,
        capture_output=True,
        text=True,
        timeout=timeout,
    )
    _write_log(
        log_path,
        args,
        result.stdout,
        result.stderr,
        result.returncode,
        time.monotonic() - started,
    )
    if result.returncode != 0:
        raise RuntimeError(f"{shot.name}: UI test failed; see {log_path}")
    marker = "UI_TEST_SCREENSHOT_PATH="
    source = next(
        (
            Path(line.removeprefix(marker))
            for line in result.stdout.splitlines()
            if line.startswith(marker)
        ),
        None,
    )
    if source is None or not source.is_file():
        raise RuntimeError(f"{shot.name}: UI test screenshot was not created")
    shutil.copy2(source, output_path)
    return output_path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=Path("scripts/screenshots.yaml"))
    parser.add_argument("--set", dest="set_name", help="Capture one named manifest set")
    parser.add_argument("--output", type=Path, help="Override every per-shot output directory")
    parser.add_argument("--repo-path", type=Path, default=Path.cwd())
    parser.add_argument("--executable", type=Path)
    parser.add_argument("--lf-binary", type=Path, default=Path("lf"))
    parser.add_argument("--live-timeout", type=int, default=30)
    parser.add_argument("--ui-test-timeout", type=int, default=120)
    parser.add_argument("--log-dir", type=Path, default=Path("scratch/screenshot-logs"))
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parent.parent
    manifest_path = args.manifest if args.manifest.is_absolute() else repo_root / args.manifest
    shots = select_screenshots(load_manifest(manifest_path), args.set_name)
    if not shots:
        raise SystemExit(f"no screenshots found for set {args.set_name!r}")

    if any(shot.type == "uitest" for shot in shots):
        subprocess.run(["xcodegen", "generate"], cwd=repo_root / "swift", check=True)
    executable = None
    if any(shot.type == "live" for shot in shots):
        executable = find_loopflow_executable(args.executable, repo_root)
        for wave in {shot.wave for shot in shots if shot.wave}:
            validate_live_state(args.lf_binary, args.repo_path.resolve(), wave)

    failures = []
    for shot in shots:
        output_path = resolve_output_path(shot, repo_root, args.output)
        log_path = repo_root / args.log_dir / f"{shot.name}.log"
        print(f"Capturing {shot.name} ({shot.type})...")
        try:
            if shot.type == "live":
                assert executable is not None
                capture_live_screenshot(
                    shot,
                    repo_path=args.repo_path.resolve(),
                    output_path=output_path,
                    executable=executable,
                    timeout=args.live_timeout,
                    log_path=log_path,
                )
            else:
                capture_ui_test_screenshot(
                    shot,
                    repo_root=repo_root,
                    output_path=output_path,
                    timeout=args.ui_test_timeout,
                    log_path=log_path,
                )
            print(f"  -> {output_path}")
        except Exception as exc:
            failures.append(str(exc))
            print(f"  FAIL: {exc}", file=sys.stderr)

    if failures:
        raise SystemExit(f"{len(failures)}/{len(shots)} captures failed")
    print(f"Captured {len(shots)} screenshots.")


if __name__ == "__main__":
    main()
