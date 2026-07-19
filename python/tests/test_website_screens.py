"""Behavioral tests for the website capture pipeline."""

from __future__ import annotations

import plistlib
import subprocess
import sys
from dataclasses import asdict
from datetime import datetime, timedelta, timezone
from pathlib import Path

import pytest

SCRIPTS_DIR = Path(__file__).resolve().parents[2] / "scripts"
sys.path.insert(0, str(SCRIPTS_DIR))
try:
    import website_screens
    from website_screens import (
        CaptureProvenance,
        CaptureUnavailable,
        LiveCapture,
        capture,
        capture_environment,
        captured_wave,
        load_captures,
        read_app_version,
        sidecar_path,
        validate_capture,
        write_json,
    )
finally:
    sys.path.pop(0)


def _shot(name: str = "context-lab", view: str = "context-lab") -> LiveCapture:
    return LiveCapture(
        name=name,
        view=view,
        wave="product",
        width=1440,
        height=900,
        delay=8,
        output=Path("website/static") / f"{name}.png",
    )


def _write_capture(tmp_path: Path, captured_at: datetime) -> Path:
    image = tmp_path / "context-lab.png"
    image.write_bytes(b"png")
    provenance = CaptureProvenance(
        captured_at=captured_at.isoformat(),
        wave="product",
        app_version="0.11.3",
    )
    write_json(sidecar_path(image), asdict(provenance))
    return image


# --- Manifest ---


def test_website_set_captures_three_real_views() -> None:
    shots = load_captures()

    assert {shot.view for shot in shots} == {"context-lab", "wave", "roadmap"}
    assert captured_wave(shots) == "product"
    assert all(shot.output.parent == Path("website/static") for shot in shots)


# --- Capture environment ---


def test_roadmap_capture_does_not_auto_select_a_wave(tmp_path: Path) -> None:
    shots = {shot.view: shot for shot in load_captures()}

    roadmap = capture_environment(shots["roadmap"], tmp_path / "roadmap.png")
    context_lab = capture_environment(shots["context-lab"], tmp_path / "context-lab.png")

    assert "LOOPFLOW_UI_TEST_SELECT_BRANCH" not in roadmap
    assert context_lab["LOOPFLOW_UI_TEST_SELECT_BRANCH"] == "product"


def test_capture_timeout_preserves_text_stderr(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    def _time_out(*args, **kwargs):
        raise subprocess.TimeoutExpired(args[0], 30, stderr="window did not settle")

    monkeypatch.setattr(website_screens.subprocess, "run", _time_out)

    with pytest.raises(RuntimeError, match="window did not settle"):
        capture(
            _shot(),
            executable=Path("Loopflow"),
            repo_path=tmp_path,
            output=tmp_path / "capture.png",
        )


# --- App version ---


def test_app_version_comes_from_the_installed_bundle(tmp_path: Path) -> None:
    executable = tmp_path / "Loopflow.app/Contents/MacOS/Loopflow"
    executable.parent.mkdir(parents=True)
    executable.write_bytes(b"app")
    (executable.parent.parent / "Info.plist").write_bytes(
        plistlib.dumps({"CFBundleShortVersionString": "0.11.4-dev"})
    )

    assert read_app_version(executable) == "0.11.4-dev"


def test_a_missing_app_is_unavailable_not_a_crash(tmp_path: Path) -> None:
    with pytest.raises(CaptureUnavailable, match="missing"):
        read_app_version(tmp_path / "Loopflow.app/Contents/MacOS/Loopflow")


# --- Gate ---


def test_capture_gate_passes_a_fresh_sidecar_round_trip(tmp_path: Path) -> None:
    image = _write_capture(tmp_path, datetime.now(timezone.utc))

    assert validate_capture(image) == ([], [])


def test_capture_gate_warns_on_stale_provenance_without_failing(tmp_path: Path) -> None:
    image = _write_capture(tmp_path, datetime.now(timezone.utc) - timedelta(days=15))

    errors, warnings = validate_capture(image)

    assert errors == []
    assert any("15 days old" in warning for warning in warnings)


def test_capture_gate_rejects_an_image_without_provenance(tmp_path: Path) -> None:
    image = tmp_path / "context-lab.png"
    image.write_bytes(b"png")

    errors, _ = validate_capture(image)

    assert any("without context-lab.json" in error for error in errors)


def test_capture_gate_rejects_an_unparseable_sidecar(tmp_path: Path) -> None:
    image = tmp_path / "context-lab.png"
    image.write_bytes(b"png")
    sidecar_path(image).write_text('{"captured_at": "not a timestamp"}')

    errors, _ = validate_capture(image)

    assert any("invalid provenance" in error for error in errors)


def test_capture_gate_allows_an_image_to_be_absent(tmp_path: Path) -> None:
    assert validate_capture(tmp_path / "not-published.png") == ([], [])
