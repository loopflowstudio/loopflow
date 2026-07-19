import json
from pathlib import Path

from fasthtml.common import to_xml

import main

SHOWCASE_ITEM = {
    "image": "/static/context-lab.png",
    "image_alt": "Context Lab",
    "caption": "What the agent received.",
}


def test_provenance_caption_renders_when_the_sidecar_parses(
    monkeypatch,
    tmp_path: Path,
) -> None:
    image = tmp_path / "context-lab.png"
    image.write_bytes(b"png")
    image.with_suffix(".json").write_text(
        json.dumps(
            {
                "captured_at": "2026-07-20T12:00:00Z",
                "wave": "product",
                "app_version": "0.11.3",
            }
        )
    )
    monkeypatch.setattr(main, "STATIC_DIR", tmp_path)

    html = to_xml(main._capture_figure(SHOWCASE_ITEM))

    assert "Captured 2026-07-20 from the product wave" in html
    assert "Loopflow 0.11.3" in html


def test_figure_renders_without_a_sidecar(monkeypatch, tmp_path: Path) -> None:
    image = tmp_path / "context-lab.png"
    image.write_bytes(b"png")
    monkeypatch.setattr(main, "STATIC_DIR", tmp_path)

    figure = main._capture_figure(SHOWCASE_ITEM)

    assert figure is not None
    html = to_xml(figure)
    assert 'src="/static/context-lab.png"' in html
    assert "What the agent received." in html
    assert "loopflow-showcase-provenance" not in html
