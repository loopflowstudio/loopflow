import json
from pathlib import Path

from fasthtml.common import to_xml

import main


def test_provenance_caption_links_to_the_live_status_snapshot(
    monkeypatch,
    tmp_path: Path,
) -> None:
    image = tmp_path / "context-lab.png"
    image.write_bytes(b"png")
    image.with_suffix(".status.json").write_text("{}")
    image.with_suffix(".json").write_text(
        json.dumps(
            {
                "captured_at": "2026-07-20T12:00:00Z",
                "wave": "product",
                "app_version": "0.11.3",
                "app_commit": "a" * 40,
            }
        )
    )
    monkeypatch.setattr(main, "STATIC_DIR", tmp_path)

    figure = main._capture_figure(
        {
            "image": "/static/context-lab.png",
            "image_alt": "Context Lab",
            "caption": "What the agent received.",
        }
    )

    html = to_xml(figure)
    assert "Captured 2026-07-20 from the product wave" in html
    assert 'href="/static/context-lab.status.json"' in html
    assert "Loopflow 0.11.3 @ aaaaaaa" in html
    assert f'href="{main.REPO_URL}/commit/{"a" * 40}"' in html
