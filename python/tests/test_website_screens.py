"""Behavioral tests for the living website capture pipeline."""

from __future__ import annotations

import plistlib
import sys
from dataclasses import asdict
from datetime import datetime, timedelta, timezone
from pathlib import Path

import pytest
from PIL import Image

SCRIPTS_DIR = Path(__file__).resolve().parents[2] / "scripts"
sys.path.insert(0, str(SCRIPTS_DIR))
try:
    from website_screens import (
        CaptureProvenance,
        CaptureUnavailable,
        LiveCapture,
        capture_environment,
        captured_wave,
        changed_meaningfully,
        load_captures,
        read_app_build,
        require_live_wave,
        validate_capture,
        write_json,
    )
finally:
    sys.path.pop(0)


def _live_status(*, live: bool = True, process_alive: bool = True) -> dict:
    return {
        "wave": {"name": "product", "live": live},
        "projects": [{"tasks": [{"runtime": {"process_alive": process_alive}}]}],
    }


def _write_image(
    path: Path,
    color: tuple[int, int, int],
    size: tuple[int, int] = (100, 100),
) -> None:
    Image.new("RGB", size, color).save(path)


def _shot(tmp_path: Path, name: str = "context-lab", view: str = "context-lab") -> LiveCapture:
    return LiveCapture(
        name=name,
        view=view,
        wave="product",
        width=1440,
        height=900,
        delay=8,
        output=Path("website/static") / f"{name}.png",
    )


def _provenance(captured_at: datetime) -> CaptureProvenance:
    return CaptureProvenance(
        captured_at=captured_at.isoformat(),
        wave="product",
        app_version="0.11.3",
        app_commit="a" * 40,
    )


def test_website_set_captures_three_real_views() -> None:
    shots = load_captures()

    assert {shot.view for shot in shots} == {"context-lab", "wave", "roadmap"}
    assert captured_wave(shots) == "product"
    assert all(shot.output.parent == Path("website/static") for shot in shots)


def test_roadmap_capture_does_not_auto_select_a_wave(tmp_path: Path) -> None:
    shots = {shot.view: shot for shot in load_captures()}

    roadmap = capture_environment(shots["roadmap"], tmp_path / "roadmap.png")
    context_lab = capture_environment(shots["context-lab"], tmp_path / "context-lab.png")

    assert "LOOPFLOW_UI_TEST_SELECT_BRANCH" not in roadmap
    assert context_lab["LOOPFLOW_UI_TEST_SELECT_BRANCH"] == "product"


def test_live_capture_requires_a_served_wave_with_a_live_task() -> None:
    require_live_wave(_live_status(), "product")

    with pytest.raises(CaptureUnavailable, match="not served"):
        require_live_wave(_live_status(live=False), "product")
    with pytest.raises(CaptureUnavailable, match="no live Task"):
        require_live_wave(_live_status(process_alive=False), "product")


def _promoted_app(tmp_path: Path, *, dirty: bool = False) -> Path:
    executable = tmp_path / "Loopflow.app/Contents/MacOS/Loopflow"
    executable.parent.mkdir(parents=True)
    executable.write_bytes(b"app")
    (executable.parent.parent / "Info.plist").write_bytes(
        plistlib.dumps(
            {
                "CFBundleShortVersionString": "0.11.4-dev",
                "LoopflowSourceCommit": "a" * 40,
                "LoopflowSourceDirty": dirty,
            }
        )
    )
    return executable


def test_app_build_provenance_comes_from_the_promoted_bundle(tmp_path: Path) -> None:
    build = read_app_build(_promoted_app(tmp_path))

    assert build.version == "0.11.4-dev"
    assert build.commit == "a" * 40


def test_a_dirty_build_can_never_be_published(tmp_path: Path) -> None:
    with pytest.raises(CaptureUnavailable, match="dirty source tree"):
        read_app_build(_promoted_app(tmp_path, dirty=True))


def test_perceptual_diff_ignores_one_volatile_pixel_but_catches_a_real_change(
    tmp_path: Path,
) -> None:
    current = tmp_path / "current.png"
    volatile = tmp_path / "volatile.png"
    changed = tmp_path / "changed.png"
    _write_image(current, (255, 255, 255))
    _write_image(volatile, (255, 255, 255))
    with Image.open(volatile) as image:
        image.putpixel((0, 0), (0, 0, 0))
        image.save(volatile)
    _write_image(changed, (240, 240, 240))

    assert not changed_meaningfully(current, volatile)
    assert changed_meaningfully(current, changed)


def _publishable_capture(tmp_path: Path, captured_at: datetime) -> Path:
    image = tmp_path / "context-lab.png"
    _write_image(image, (255, 255, 255), size=(2880, 1800))
    write_json(image.with_suffix(".status.json"), _live_status())
    write_json(image.with_suffix(".json"), asdict(_provenance(captured_at)))
    return image


def test_capture_gate_passes_a_fresh_proven_capture(tmp_path: Path) -> None:
    image = _publishable_capture(tmp_path, datetime.now(timezone.utc))

    assert validate_capture(image, _shot(tmp_path)) == []


def test_capture_gate_rejects_stale_provenance(tmp_path: Path) -> None:
    image = _publishable_capture(tmp_path, datetime.now(timezone.utc) - timedelta(days=15))

    errors = validate_capture(image, _shot(tmp_path))

    assert any("15 days old" in error for error in errors)


def test_capture_gate_rejects_a_capture_whose_live_state_was_not_live(tmp_path: Path) -> None:
    image = _publishable_capture(tmp_path, datetime.now(timezone.utc))
    write_json(image.with_suffix(".status.json"), _live_status(live=False))

    errors = validate_capture(image, _shot(tmp_path))

    assert any("not served" in error for error in errors)


def test_capture_gate_rejects_an_image_without_provenance(tmp_path: Path) -> None:
    image = tmp_path / "context-lab.png"
    _write_image(image, (255, 255, 255), size=(2880, 1800))

    errors = validate_capture(image, _shot(tmp_path))

    assert any("without context-lab.json" in error for error in errors)


def test_capture_gate_allows_an_image_to_be_absent(tmp_path: Path) -> None:
    assert validate_capture(tmp_path / "not-published.png", _shot(tmp_path)) == []
