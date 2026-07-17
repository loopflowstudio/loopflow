"""Shared capture provenance, perceptual diff, and freshness checks."""

from __future__ import annotations

import json
import plistlib
import re
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any

from PIL import Image, ImageChops

MAX_CAPTURE_AGE = timedelta(days=14)
PIXEL_DELTA = 12
CHANGED_PIXEL_RATIO = 0.002
MAX_CLOCK_SKEW = timedelta(minutes=5)


class CaptureUnavailable(RuntimeError):
    """Live product state cannot honestly produce a capture right now."""


@dataclass(frozen=True)
class AppBuild:
    version: str
    commit: str
    source_dirty: bool


@dataclass(frozen=True)
class CaptureProvenance:
    captured_at: str
    wave: str
    view: str
    app_version: str
    app_commit: str
    source_dirty: bool
    pixel_width: int
    pixel_height: int
    status_snapshot: str

    def as_dict(self) -> dict[str, Any]:
        return {
            "captured_at": self.captured_at,
            "wave": self.wave,
            "view": self.view,
            "app_version": self.app_version,
            "app_commit": self.app_commit,
            "source_dirty": self.source_dirty,
            "pixel_width": self.pixel_width,
            "pixel_height": self.pixel_height,
            "status_snapshot": self.status_snapshot,
        }


def read_app_build(executable: Path) -> AppBuild:
    info_plist = executable.parent.parent / "Info.plist"
    if not executable.is_file():
        raise CaptureUnavailable(f"promoted app executable is missing: {executable}")
    if not info_plist.is_file():
        raise CaptureUnavailable(f"promoted app metadata is missing: {info_plist}")
    try:
        data = plistlib.loads(info_plist.read_bytes())
        version = data["CFBundleShortVersionString"]
        commit = data["LoopflowSourceCommit"]
        source_dirty = data["LoopflowSourceDirty"]
    except (KeyError, plistlib.InvalidFileException) as exc:
        raise CaptureUnavailable(f"promoted app has no source provenance: {exc}") from exc
    if not isinstance(version, str) or not version:
        raise CaptureUnavailable("promoted app has an invalid version")
    if not isinstance(commit, str) or not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise CaptureUnavailable("promoted app has an invalid source commit")
    if not isinstance(source_dirty, bool):
        raise CaptureUnavailable("promoted app has an invalid dirty-source flag")
    return AppBuild(version=version, commit=commit, source_dirty=source_dirty)


def require_live_wave(payload: dict[str, Any], expected_wave: str) -> None:
    wave = payload.get("wave") or {}
    if wave.get("name") != expected_wave:
        raise CaptureUnavailable(f"lf status returned {wave.get('name')!r}, not {expected_wave!r}")
    if not wave.get("live"):
        raise CaptureUnavailable(f"{expected_wave} is not served")
    tasks = [task for project in payload.get("projects", []) for task in project.get("tasks", [])]
    if not any((task.get("runtime") or {}).get("process_alive") for task in tasks):
        raise CaptureUnavailable(f"{expected_wave} has no live Task to show")


def changed_meaningfully(
    current: Path,
    candidate: Path,
    *,
    threshold: float = CHANGED_PIXEL_RATIO,
) -> bool:
    if not current.is_file():
        return True
    with Image.open(current) as old_image, Image.open(candidate) as new_image:
        old = old_image.convert("RGB")
        new = new_image.convert("RGB")
        if old.size != new.size:
            return True
        difference = ImageChops.difference(old, new).convert("L")
        changed = difference.point(lambda value: 255 if value > PIXEL_DELTA else 0)
        histogram = changed.histogram()
        changed_pixels = histogram[255]
        total_pixels = old.width * old.height
        return changed_pixels / total_pixels >= threshold


def image_dimensions(path: Path) -> tuple[int, int]:
    with Image.open(path) as image:
        return image.size


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")


def load_provenance(path: Path) -> CaptureProvenance:
    raw = json.loads(path.read_text())
    return CaptureProvenance(
        captured_at=raw["captured_at"],
        wave=raw["wave"],
        view=raw["view"],
        app_version=raw["app_version"],
        app_commit=raw["app_commit"],
        source_dirty=raw["source_dirty"],
        pixel_width=raw["pixel_width"],
        pixel_height=raw["pixel_height"],
        status_snapshot=raw["status_snapshot"],
    )


def validate_capture(
    image_path: Path,
    *,
    expected_wave: str,
    expected_view: str,
    logical_width: int,
    now: datetime | None = None,
) -> list[str]:
    if not image_path.is_file():
        return []
    errors = []
    sidecar_path = image_path.with_suffix(".json")
    if not sidecar_path.is_file():
        return [f"{image_path}: capture exists without {sidecar_path.name}"]
    try:
        provenance = load_provenance(sidecar_path)
        captured_at = datetime.fromisoformat(provenance.captured_at.replace("Z", "+00:00"))
    except (KeyError, TypeError, ValueError, json.JSONDecodeError) as exc:
        return [f"{sidecar_path}: invalid provenance: {exc}"]

    current_time = now or datetime.now(timezone.utc)
    if captured_at.tzinfo is None:
        errors.append(f"{sidecar_path}: captured_at must include a timezone")
    elif captured_at - current_time > MAX_CLOCK_SKEW:
        errors.append(f"{sidecar_path}: captured_at is in the future")
    elif current_time - captured_at > MAX_CAPTURE_AGE:
        age = current_time - captured_at
        errors.append(f"{image_path}: capture is {age.days} days old (maximum is 14)")
    if provenance.wave != expected_wave:
        errors.append(f"{sidecar_path}: wave is {provenance.wave!r}, expected {expected_wave!r}")
    if provenance.view != expected_view:
        errors.append(f"{sidecar_path}: view is {provenance.view!r}, expected {expected_view!r}")
    if provenance.source_dirty:
        errors.append(f"{sidecar_path}: capture came from a dirty app source tree")
    if not provenance.app_version:
        errors.append(f"{sidecar_path}: app_version is empty")
    if not re.fullmatch(r"[0-9a-f]{40}", provenance.app_commit):
        errors.append(f"{sidecar_path}: app_commit is not a full Git commit")
    try:
        actual_width, actual_height = image_dimensions(image_path)
    except OSError as exc:
        errors.append(f"{image_path}: invalid image: {exc}")
        return errors
    if (actual_width, actual_height) != (provenance.pixel_width, provenance.pixel_height):
        errors.append(
            f"{sidecar_path}: recorded {provenance.pixel_width}x{provenance.pixel_height}, "
            f"image is {actual_width}x{actual_height}"
        )
    if actual_width < logical_width * 2:
        errors.append(f"{image_path}: {actual_width}px is not a 2x capture of {logical_width}pt")
    status_path = image_path.parent / Path(provenance.status_snapshot).name
    if not status_path.is_file():
        errors.append(f"{sidecar_path}: status snapshot is missing: {status_path.name}")
    else:
        try:
            require_live_wave(json.loads(status_path.read_text()), expected_wave)
        except (CaptureUnavailable, json.JSONDecodeError) as exc:
            errors.append(f"{status_path}: invalid live status snapshot: {exc}")
    return errors
