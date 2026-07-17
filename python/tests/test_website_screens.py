"""Behavioral tests for the living website capture pipeline."""

from __future__ import annotations

import plistlib
import sys
from datetime import datetime, timedelta, timezone
from pathlib import Path

import pytest
from PIL import Image

SCRIPTS_DIR = Path(__file__).resolve().parents[2] / "scripts"
sys.path.insert(0, str(SCRIPTS_DIR))
try:
    from generate_screenshots import (
        live_capture_environment,
        load_manifest,
        select_screenshots,
    )
    from website_screens import (
        CaptureProvenance,
        CaptureUnavailable,
        changed_meaningfully,
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


def test_website_set_captures_three_real_views() -> None:
    manifest = load_manifest(SCRIPTS_DIR / "screenshots.yaml")

    shots = select_screenshots(manifest, "website")

    assert {shot.view for shot in shots} == {"context-lab", "wave", "roadmap"}
    assert {shot.type for shot in shots} == {"live"}
    assert {shot.wave for shot in shots} == {"product"}


def test_roadmap_capture_does_not_auto_select_a_wave(tmp_path: Path) -> None:
    manifest = load_manifest(SCRIPTS_DIR / "screenshots.yaml")
    shots = {shot.view: shot for shot in select_screenshots(manifest, "website")}

    roadmap = live_capture_environment(shots["roadmap"], tmp_path / "roadmap.png")
    context_lab = live_capture_environment(shots["context-lab"], tmp_path / "context-lab.png")

    assert "LOOPFLOW_UI_TEST_SELECT_BRANCH" not in roadmap
    assert context_lab["LOOPFLOW_UI_TEST_SELECT_BRANCH"] == "product"


def test_live_capture_requires_a_served_wave_with_a_live_task() -> None:
    require_live_wave(_live_status(), "product")

    with pytest.raises(CaptureUnavailable, match="not served"):
        require_live_wave(_live_status(live=False), "product")
    with pytest.raises(CaptureUnavailable, match="no live Task"):
        require_live_wave(_live_status(process_alive=False), "product")


def test_app_build_provenance_comes_from_the_promoted_bundle(tmp_path: Path) -> None:
    executable = tmp_path / "Loopflow.app/Contents/MacOS/Loopflow"
    executable.parent.mkdir(parents=True)
    executable.write_bytes(b"app")
    info_plist = executable.parent.parent / "Info.plist"
    info_plist.write_bytes(
        plistlib.dumps(
            {
                "CFBundleShortVersionString": "0.11.4-dev",
                "LoopflowSourceCommit": "a" * 40,
                "LoopflowSourceDirty": False,
            }
        )
    )

    build = read_app_build(executable)

    assert build.version == "0.11.4-dev"
    assert build.commit == "a" * 40
    assert build.source_dirty is False


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


def test_capture_gate_rejects_stale_provenance(tmp_path: Path) -> None:
    image = tmp_path / "context-lab.png"
    status = tmp_path / "context-lab.status.json"
    _write_image(image, (255, 255, 255), size=(2880, 1800))
    write_json(status, _live_status())
    captured_at = datetime.now(timezone.utc) - timedelta(days=15)
    provenance = CaptureProvenance(
        captured_at=captured_at.isoformat(),
        wave="product",
        view="context-lab",
        app_version="0.11.3",
        app_commit="a" * 40,
        source_dirty=False,
        pixel_width=2880,
        pixel_height=1800,
        status_snapshot="/static/context-lab.status.json",
    )
    write_json(image.with_suffix(".json"), provenance.as_dict())

    errors = validate_capture(
        image,
        expected_wave="product",
        expected_view="context-lab",
        logical_width=1440,
    )

    assert any("15 days old" in error for error in errors)


def test_capture_gate_allows_an_image_to_be_absent(tmp_path: Path) -> None:
    errors = validate_capture(
        tmp_path / "not-published.png",
        expected_wave="product",
        expected_view="context-lab",
        logical_width=1440,
    )

    assert errors == []
