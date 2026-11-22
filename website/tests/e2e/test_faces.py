"""The site's three faces load from this server and the serif keeps to display sizes."""

import pytest
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
