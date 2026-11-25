"""The site's three faces load from this server and the serif keeps to display sizes."""

from pathlib import Path

import pytest
import yaml
from playwright.sync_api import Page

GOOGLE_FONT_HOSTS = ("fonts.googleapis.com", "fonts.gstatic.com")

SERIF_TEXT = """() => [...document.querySelectorAll('body *')]
    .filter(el => getComputedStyle(el).fontFamily.startsWith('"Old Standard TT"'))
    .filter(el => [...el.childNodes].some(n => n.nodeType === 3 && n.textContent.trim()))
    .filter(el => el.getClientRects().length)
    .map(el => ({
        text: el.textContent.trim().slice(0, 40),
        size: parseFloat(getComputedStyle(el).fontSize),
        ligatures: getComputedStyle(el).fontVariantLigatures,
    }))"""


@pytest.mark.parametrize("path", ["/", "/docs", "/download"])
def test_faces_load_from_the_site(page: Page, base_url: str, path: str) -> None:
    requests: list[str] = []
    page.on("request", lambda request: requests.append(request.url))
    page.goto(f"{base_url}{path}")
    page.evaluate("document.fonts.ready")

    assert page.evaluate("document.fonts.check('16px \"Atkinson Hyperlegible Next\"')")
    assert page.evaluate("document.fonts.check('32px \"Old Standard TT\"')")
    h1 = page.locator("h1").first
    assert h1.evaluate("el => getComputedStyle(el).fontFamily").startswith('"Old Standard TT"')
    assert not [url for url in requests if any(host in url for host in GOOGLE_FONT_HOSTS)]


@pytest.mark.parametrize("path", ["/", "/studio", "/docs", "/download"])
@pytest.mark.parametrize("width", [1440, 390])
def test_serif_is_display_size_with_fl_apart(
    page: Page, base_url: str, path: str, width: int
) -> None:
    page.set_viewport_size({"width": width, "height": 900})
    page.goto(f"{base_url}{path}")
    page.evaluate("document.fonts.ready")

    serif = page.evaluate(SERIF_TEXT)
    assert serif, f"no serif text on {path}"
    for text in serif:
        assert text["size"] >= 24, text
        assert "no-common-ligatures" in text["ligatures"], text


FRAME_PAGES = ["/", "/studio", "/docs", "/download"]


@pytest.mark.parametrize("width", [1440, 390])
def test_every_page_has_the_same_bar_and_footer(page: Page, base_url: str, width: int) -> None:
    page.set_viewport_size({"width": width, "height": 900})
    frames = {}
    for path in FRAME_PAGES:
        page.goto(f"{base_url}{path}")
        frames[path] = {
            part: (
                page.locator(part).evaluate(
                    "el => el.outerHTML.replace(/ aria-current=\"page\"/g, '')"
                ),
                {key: page.locator(part).bounding_box()[key] for key in ("x", "width")},
            )
            for part in (".site-header", ".site-footer")
        }
    home = frames["/"]
    for path in FRAME_PAGES[1:]:
        assert frames[path] == home, path


GRAPHS = Path(__file__).resolve().parents[3] / "rust/loopflow-definitions/src/builtins/task/graph"


def _graph_steps(items: list[dict | str]) -> list[tuple[str, bool]]:
    steps = []
    for item in items:
        if isinstance(item, str):
            steps.append((item, False))
            continue
        ((kind, value),) = item.items()
        if kind == "loop":
            steps += _graph_steps(value)
        else:
            steps.append((value, kind == "interactive"))
    return steps


@pytest.mark.parametrize("graph", ["feature", "code", "research"])
def test_workflow_card_shows_the_steps_the_graph_runs(homepage: Page, graph: str) -> None:
    line = homepage.locator(".wfs li", has=homepage.locator("code", has_text=graph)).locator(
        ".line"
    )
    shown = line.evaluate(
        """el => [...el.querySelectorAll('span, b')].map(step =>
            [step.textContent.replace('→', '').trim(), step.tagName === 'B'])"""
    )
    expected = _graph_steps(yaml.safe_load((GRAPHS / f"{graph}.yaml").read_text())["steps"])
    assert [tuple(step) for step in shown] == expected


COLORS = """() => [...document.querySelectorAll('body *')].map(el => {
    const style = getComputedStyle(el);
    return [style.color, style.backgroundColor, style.borderTopColor].join(' ');
})"""


@pytest.mark.parametrize("path", ["/", "/docs", "/download"])
def test_os_dark_mode_shows_the_light_site(page: Page, base_url: str, path: str) -> None:
    page.goto(f"{base_url}{path}")
    page.emulate_media(color_scheme="light")
    light = page.evaluate(COLORS)
    page.emulate_media(color_scheme="dark")
    assert page.evaluate(COLORS) == light


def test_font_files_serve_cross_origin_and_missing_ones_are_not_found(
    page: Page, base_url: str
) -> None:
    font = page.request.get(f"{base_url}/static/fonts/atkinson-next.woff2")
    assert font.status == 200
    assert font.headers["access-control-allow-origin"] == "*"
    assert page.request.get(f"{base_url}/static/fonts/missing.woff2").status == 404


def test_favicon_ico_answers_with_the_logo(page: Page, base_url: str) -> None:
    response = page.request.get(f"{base_url}/favicon.ico")
    assert response.status == 200
    assert response.headers["content-type"].startswith("image/svg+xml")
