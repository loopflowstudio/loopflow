import json
from pathlib import Path

import pytest
from fasthtml.common import to_xml

import main

SHOWCASE = {
    "pointer": {"left": 36, "top": 63},
    "highlight": {"left": 21, "top": 61, "width": 76, "height": 4},
    "note": "Actual captures.",
    "notes": [],
}


def _frame(name: str) -> dict[str, str]:
    return {
        "id": name,
        "label": name.title(),
        "image": f"/static/{name}.png",
        "image_alt": name,
        "caption": f"The {name}.",
    }


def test_showcase_renders_all_available_captures(monkeypatch, tmp_path: Path) -> None:
    for name in ("wave", "session"):
        (tmp_path / f"{name}.png").write_bytes(b"png")
    monkeypatch.setattr(main, "STATIC_DIR", tmp_path)
    monkeypatch.setattr(
        main,
        "SHOWCASE_CONTENT",
        {**SHOWCASE, "items": [_frame(name) for name in ("wave", "session", "missing")]},
    )

    html = to_xml(main._showcase_section())

    assert html.index('src="/static/wave.png"') < html.index('src="/static/session.png"')
    assert 'href="/static/session.png"' in html
    assert "/static/missing.png" not in html


def test_showcase_is_absent_without_captures(monkeypatch, tmp_path: Path) -> None:
    monkeypatch.setattr(main, "STATIC_DIR", tmp_path)
    monkeypatch.setattr(main, "SHOWCASE_CONTENT", {**SHOWCASE, "items": [_frame("missing")]})

    assert main._showcase_section() is None


@pytest.mark.parametrize("wave", ["product", None])
def test_provenance_caption_renders_when_the_sidecar_parses(
    monkeypatch,
    tmp_path: Path,
    wave: str | None,
) -> None:
    image = tmp_path / "wave.png"
    image.write_bytes(b"png")
    image.with_suffix(".json").write_text(
        json.dumps(
            {
                "captured_at": "2026-07-20T12:00:00Z",
                "wave": wave,
                "app_version": "0.11.3",
            }
        )
    )
    monkeypatch.setattr(main, "STATIC_DIR", tmp_path)
    monkeypatch.setattr(main, "SHOWCASE_CONTENT", {**SHOWCASE, "items": [_frame("wave")]})

    html = to_xml(main._showcase_section())

    assert "Captured 2026-07-20" in html
    assert ("from the product wave" in html) == (wave == "product")
    assert "Loopflow 0.11.3" in html


def test_frame_renders_without_a_sidecar(monkeypatch, tmp_path: Path) -> None:
    (tmp_path / "wave.png").write_bytes(b"png")
    monkeypatch.setattr(main, "STATIC_DIR", tmp_path)
    monkeypatch.setattr(main, "SHOWCASE_CONTENT", {**SHOWCASE, "items": [_frame("wave")]})

    html = to_xml(main._showcase_section())

    assert 'src="/static/wave.png"' in html
    assert "The wave." in html
    assert "loopflow-showcase-provenance" not in html
