"""Website captures: manifest, capture environment, provenance sidecars, gate.

Every published image is the installed app photographed against this repo's
own Wave state. `scripts/capture_screenshots.py` produces the images and
sidecars; `scripts/check_website_screens.py` gates the website deploy on them.
"""

from __future__ import annotations

import json
import os
import plistlib
import subprocess
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any

import yaml

REPO_ROOT = Path(__file__).resolve().parent.parent
MANIFEST_PATH = REPO_ROOT / "scripts/screenshots.yaml"

MAX_CAPTURE_AGE = timedelta(days=14)
CAPTURE_TIMEOUT = 30


class CaptureUnavailable(RuntimeError):
    """The installed app cannot produce a capture right now."""


@dataclass(frozen=True)
class LiveCapture:
    """One published image: which view of which Wave, at which size."""

    name: str
    view: str
    wave: str
    width: int
    height: int
    delay: float
    output: Path


@dataclass(frozen=True)
class CaptureProvenance:
    """What the caption claims and the gate checks: when, which Wave, which build."""

    captured_at: str
    wave: str
    app_version: str


# --- Manifest ---


def load_captures(path: Path = MANIFEST_PATH) -> list[LiveCapture]:
    """Every entry is complete; a malformed manifest raises rather than degrades."""
    data = yaml.safe_load(path.read_text())
    return [
        LiveCapture(
            name=raw["name"],
            view=raw["view"],
            wave=raw["wave"],
            width=raw["width"],
            height=raw["height"],
            delay=raw["delay"],
            output=Path(raw["output"]),
        )
        for raw in data["website"]
    ]


def captured_wave(captures: list[LiveCapture]) -> str:
    waves = {capture.wave for capture in captures}
    if len(waves) != 1:
        raise ValueError(f"the website set must show one live Wave; found {sorted(waves)}")
    return waves.pop()


def sidecar_path(image: Path) -> Path:
    """The provenance file that ships beside `image`."""
    return image.with_suffix(".json")


# --- The installed app ---


def read_app_version(executable: Path) -> str:
    """CFBundleShortVersionString of the installed bundle."""
    info_plist = executable.parent.parent / "Info.plist"
    if not executable.is_file():
        raise CaptureUnavailable(f"installed app executable is missing: {executable}")
    if not info_plist.is_file():
        raise CaptureUnavailable(f"installed app metadata is missing: {info_plist}")
    try:
        version = plistlib.loads(info_plist.read_bytes())["CFBundleShortVersionString"]
    except (KeyError, plistlib.InvalidFileException) as exc:
        raise CaptureUnavailable(f"installed app has no version: {exc}") from exc
    if not isinstance(version, str) or not version:
        raise CaptureUnavailable(f"installed app has an invalid version: {version!r}")
    return version


# --- Capture ---


def capture_environment(shot: LiveCapture, output: Path) -> dict[str, str]:
    env = {
        **os.environ,
        "LOOPFLOW_UI_TEST_MODE": "live",
        "LOOPFLOW_UI_TEST_VIEW": shot.view,
        "LOOPFLOW_UI_TEST_WIDTH": str(shot.width),
        "LOOPFLOW_UI_TEST_HEIGHT": str(shot.height),
        "LOOPFLOW_UI_TEST_DELAY": str(shot.delay),
        "LOOPFLOW_UI_TEST_APPEARANCE": "light",
        "LOOPFLOW_UI_TEST_SNAPSHOT_PATH": str(output),
    }
    if shot.view != "roadmap":
        env["LOOPFLOW_UI_TEST_SELECT_BRANCH"] = shot.wave
    return env


def capture(shot: LiveCapture, *, executable: Path, repo_path: Path, output: Path) -> None:
    """Launch the installed app against real state; it snapshots itself and exits."""
    output.parent.mkdir(parents=True, exist_ok=True)
    output.unlink(missing_ok=True)
    args = [str(executable), "--repo", str(repo_path), "-ui-test-mode", "live"]
    try:
        result = subprocess.run(
            args,
            cwd=repo_path,
            env=capture_environment(shot, output),
            capture_output=True,
            text=True,
            timeout=CAPTURE_TIMEOUT,
        )
    except subprocess.TimeoutExpired as exc:
        stderr = exc.stderr or ""
        detail = (
            stderr.decode(errors="replace").strip() if isinstance(stderr, bytes) else stderr.strip()
        )
        raise RuntimeError(
            f"{shot.name}: capture timed out after {CAPTURE_TIMEOUT}s; {detail}"
        ) from exc
    if not output.is_file() or output.stat().st_size == 0:
        detail = result.stderr.strip() or result.stdout.strip() or "the view did not render"
        raise RuntimeError(f"{shot.name}: capture produced no image: {detail}")


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")


# --- Gate ---


def validate_capture(image: Path, *, now: datetime | None = None) -> tuple[list[str], list[str]]:
    """Why `image` is unpublishable (errors) or merely aging (warnings).

    An image without a parseable provenance sidecar is an error. A stale
    `captured_at` only warns, so shipping docs or website changes is never
    coupled to recapturing. An absent image is allowed.
    """
    if not image.is_file():
        return [], []
    sidecar = sidecar_path(image)
    if not sidecar.is_file():
        return [f"{image}: capture exists without {sidecar.name}"], []
    try:
        provenance = CaptureProvenance(**json.loads(sidecar.read_text()))
        captured_at = datetime.fromisoformat(provenance.captured_at.replace("Z", "+00:00"))
    except (TypeError, ValueError, json.JSONDecodeError) as exc:
        return [f"{sidecar}: invalid provenance: {exc}"], []

    errors = []
    warnings = []
    if captured_at.tzinfo is None:
        errors.append(f"{sidecar}: captured_at must include a timezone")
    else:
        age = (now or datetime.now(timezone.utc)) - captured_at
        if age > MAX_CAPTURE_AGE:
            warnings.append(
                f"{image}: capture is {age.days} days old (stale after {MAX_CAPTURE_AGE.days})"
            )
    if not provenance.wave:
        errors.append(f"{sidecar}: wave is empty")
    if not provenance.app_version:
        errors.append(f"{sidecar}: app_version is empty")
    return errors, warnings
