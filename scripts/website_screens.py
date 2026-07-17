"""Live product captures for the website: manifest, capture, provenance, gate.

Every published image is the promoted app photographed against this repo's own
served Wave. There is no fixture path: when the Wave is not real and served the
capture fails rather than inventing a subject. Red or failed task states are
honest and publishable — the bar is a served Wave, not a healthy one.
"""

from __future__ import annotations

import json
import os
import plistlib
import re
import subprocess
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any

import yaml
from PIL import Image, ImageChops

REPO_ROOT = Path(__file__).resolve().parent.parent
MANIFEST_PATH = REPO_ROOT / "scripts/screenshots.yaml"

MAX_CAPTURE_AGE = timedelta(days=14)
MAX_CLOCK_SKEW = timedelta(minutes=5)
CAPTURE_TIMEOUT = 30
STATUS_TIMEOUT = 10
# A pixel counts as changed past this per-channel delta; a capture counts as
# changed past this fraction of pixels. Below it, only clocks and spinners moved.
PIXEL_DELTA = 12
CHANGED_PIXEL_RATIO = 0.002


class CaptureUnavailable(RuntimeError):
    """Live product state cannot honestly produce a capture right now."""


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
class AppBuild:
    version: str
    commit: str


@dataclass(frozen=True)
class CaptureProvenance:
    """What the caption claims and the gate checks: when, which Wave, which build."""

    captured_at: str
    wave: str
    app_version: str
    app_commit: str


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


def sidecar_paths(image: Path) -> tuple[Path, Path]:
    """The provenance and live-status files that must ship beside `image`."""
    return image.with_suffix(".json"), image.with_suffix(".status.json")


# --- Live subjects ---


def require_live_wave(payload: dict[str, Any], expected_wave: str) -> None:
    wave = payload.get("wave") or {}
    if wave.get("name") != expected_wave:
        raise CaptureUnavailable(f"lf status returned {wave.get('name')!r}, not {expected_wave!r}")
    if not wave.get("live"):
        raise CaptureUnavailable(f"{expected_wave} is not served")


def live_status(lf_binary: Path, repo_path: Path, wave: str) -> dict[str, Any]:
    """The Wave snapshot a capture must be showing, proven live before we shoot it."""
    try:
        result = subprocess.run(
            [str(lf_binary), "status", wave, "--json"],
            cwd=repo_path,
            capture_output=True,
            text=True,
            timeout=STATUS_TIMEOUT,
        )
    except subprocess.TimeoutExpired as exc:
        raise CaptureUnavailable(f"lf status timed out after {STATUS_TIMEOUT}s") from exc
    if result.returncode != 0:
        raise CaptureUnavailable(result.stderr.strip() or "lf status failed")
    try:
        payload = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise CaptureUnavailable(f"lf status returned invalid JSON: {exc}") from exc
    # `lf status` prints `null` for a wave with no registry state.
    if not isinstance(payload, dict):
        raise CaptureUnavailable(f"lf status has no registry state for {wave!r}")
    require_live_wave(payload, wave)
    return payload


def _is_commit(value: object) -> bool:
    return isinstance(value, str) and bool(re.fullmatch(r"[0-9a-f]{40}", value))


def read_app_build(executable: Path) -> AppBuild:
    """Provenance of the promoted bundle. A dirty build can never be published."""
    info_plist = executable.parent.parent / "Info.plist"
    if not executable.is_file():
        raise CaptureUnavailable(f"promoted app executable is missing: {executable}")
    if not info_plist.is_file():
        raise CaptureUnavailable(f"promoted app metadata is missing: {info_plist}")
    try:
        data = plistlib.loads(info_plist.read_bytes())
        version = data["CFBundleShortVersionString"]
        commit = data["LoopflowSourceCommit"]
        dirty = data["LoopflowSourceDirty"]
    except (KeyError, plistlib.InvalidFileException) as exc:
        raise CaptureUnavailable(f"promoted app has no source provenance: {exc}") from exc
    if dirty:
        raise CaptureUnavailable("promoted app was built from a dirty source tree")
    if not isinstance(version, str) or not version or not _is_commit(commit):
        raise CaptureUnavailable(f"promoted app has invalid provenance: {version!r} @ {commit!r}")
    return AppBuild(version=version, commit=commit)


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
    """Launch the promoted app against real state; it snapshots itself and exits."""
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


def changed_meaningfully(current: Path, candidate: Path) -> bool:
    """False when only volatile pixels — clocks, spinners — moved."""
    if not current.is_file():
        return True
    with Image.open(current) as old_image, Image.open(candidate) as new_image:
        old = old_image.convert("RGB")
        new = new_image.convert("RGB")
        if old.size != new.size:
            return True
        difference = ImageChops.difference(old, new).convert("L")
        changed = difference.point(lambda value: 255 if value > PIXEL_DELTA else 0)
        changed_pixels = changed.histogram()[255]
        return changed_pixels / (old.width * old.height) >= CHANGED_PIXEL_RATIO


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")


# --- Gate ---


def validate_capture(
    image: Path,
    shot: LiveCapture,
    *,
    now: datetime | None = None,
) -> tuple[list[str], list[str]]:
    """Structural reasons `image` is unpublishable, plus freshness warnings.

    Structural errors (missing or invalid sidecars, wrong Wave, non-2x pixels,
    an unserved status snapshot, a future-dated capture) block the deploy.
    Staleness only warns, so shipping docs or website changes is never coupled
    to promoting a new app build. An absent capture is allowed.
    """
    if not image.is_file():
        return [], []
    sidecar, status = sidecar_paths(image)
    if not sidecar.is_file():
        return [f"{image}: capture exists without {sidecar.name}"], []
    try:
        raw = json.loads(sidecar.read_text())
        provenance = CaptureProvenance(**raw)
        captured_at = datetime.fromisoformat(provenance.captured_at.replace("Z", "+00:00"))
    except (TypeError, ValueError, json.JSONDecodeError) as exc:
        return [f"{sidecar}: invalid provenance: {exc}"], []

    errors = []
    warnings = []
    current_time = now or datetime.now(timezone.utc)
    if captured_at.tzinfo is None:
        errors.append(f"{sidecar}: captured_at must include a timezone")
    elif captured_at - current_time > MAX_CLOCK_SKEW:
        errors.append(f"{sidecar}: captured_at is in the future")
    elif current_time - captured_at > MAX_CAPTURE_AGE:
        age = (current_time - captured_at).days
        warnings.append(f"{image}: capture is {age} days old (stale after {MAX_CAPTURE_AGE.days})")
    if provenance.wave != shot.wave:
        errors.append(f"{sidecar}: wave is {provenance.wave!r}, expected {shot.wave!r}")
    if not provenance.app_version:
        errors.append(f"{sidecar}: app_version is empty")
    if not _is_commit(provenance.app_commit):
        errors.append(f"{sidecar}: app_commit is not a full Git commit")
    try:
        with Image.open(image) as opened:
            actual_size = opened.size
    except OSError as exc:
        return [*errors, f"{image}: invalid image: {exc}"], warnings
    expected_size = (shot.width * 2, shot.height * 2)
    if actual_size != expected_size:
        errors.append(
            f"{image}: {actual_size[0]}x{actual_size[1]}px is not a 2x capture of "
            f"{shot.width}x{shot.height}pt"
        )
    if not status.is_file():
        errors.append(f"{sidecar}: live status snapshot is missing: {status.name}")
    else:
        try:
            require_live_wave(json.loads(status.read_text()), shot.wave)
        except (CaptureUnavailable, json.JSONDecodeError) as exc:
            errors.append(f"{status}: invalid live status snapshot: {exc}")
    return errors, warnings
