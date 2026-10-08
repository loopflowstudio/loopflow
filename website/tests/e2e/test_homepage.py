"""Homepage sections, examples, destinations, and capture layout."""

from urllib.parse import urlsplit

import pytest
from playwright.sync_api import Page, expect


def test_approved_homepage_sections(homepage: Page) -> None:
    expect(homepage.locator("h1")).to_have_text("A command line for great software engineering.")
    assert homepage.locator("main > section").evaluate_all(
        "sections => sections.map(section => section.id)"
    ) == ["", "", "flows", "tasks", "mac", "why", "start"]
    expect(homepage.locator(".strip > div")).to_have_count(4)
    expect(homepage.locator("#flows .parts article")).to_have_count(3)
    assert homepage.locator(".work-row .tag").evaluate_all(
        "tags => tags.map(tag => tag.firstChild.textContent.trim())"
    ) == ["Wave", "Project", "Task", "Workflow"]


def test_hero_actions_reach_the_flow_and_install(homepage: Page) -> None:
    for label, target in [("See it ↓", "#flows"), ("Install ↓", "#start")]:
        homepage.locator(".home-hero").get_by_role("link", name=label).click()
        expect(homepage.locator(target + " h2")).to_be_in_viewport()


def test_example_loopflow_names_its_steps(homepage: Page) -> None:
    diagram = homepage.locator("#flows svg[role=img]")
    expect(diagram).to_be_visible()
    expect(diagram.locator("circle.you")).to_have_count(2)
    assert "returns to implement" in diagram.get_attribute("aria-label")
    assert "exits to land" in diagram.get_attribute("aria-label")


def test_guide_links_resolve(homepage: Page, base_url: str) -> None:
    destinations = homepage.locator(
        'main a[href^="/docs"], main a[href^="/architecture"]'
    ).evaluate_all("links => links.map(link => link.getAttribute('href'))")
    assert destinations
    for destination in destinations:
        response = homepage.goto(f"{base_url}{destination}")
        assert response is None or response.ok
        fragment = urlsplit(destination).fragment
        if fragment:
            expect(homepage.locator(f'[id="{fragment}"]')).to_have_count(1)


def test_captures_reserve_space_and_load(homepage: Page, base_url: str) -> None:
    for image in homepage.locator("main img").all():
        source = image.get_attribute("src")
        assert int(image.get_attribute("width")) > 0
        assert int(image.get_attribute("height")) > 0
        image.scroll_into_view_if_needed()
        box = image.bounding_box()
        assert box["height"] == pytest.approx(box["width"] / 1.6, abs=1)
        assert homepage.request.get(f"{base_url}{source}").ok


def test_install_has_commands_and_the_mac_download(homepage: Page) -> None:
    install = homepage.locator("#start")
    assert "loopflow.studio/install.sh" in install.locator("pre").text_content()
    assert "lf init" in install.locator("pre").text_content()
    expect(install.get_by_role("link", name="Download for Mac")).to_have_attribute(
        "href", "https://downloads.loopflow.studio/Loopflow-latest.dmg"
    )


def test_no_superseded_or_review_content(homepage: Page) -> None:
    expect(
        homepage.locator(".home-stage, .home-autonomy, .review-note, .review-bar, form")
    ).to_have_count(0)
    body = homepage.locator("body").text_content()
    for text in ("Linear", "Discord", "Review note for Jack", "cmux’s sidebar"):
        assert text not in body
    assert not homepage.locator(
        '[src*="/directions/"], [href*="/directions/"], script[src*="home.js"]'
    ).count()


@pytest.mark.parametrize("width", [390, 1440])
def test_page_fits_and_terminals_scroll_within_their_cells(
    page: Page, base_url: str, width: int
) -> None:
    page.set_viewport_size({"width": width, "height": 900})
    page.goto(base_url)
    assert page.evaluate("document.documentElement.scrollWidth") == width
    assert page.evaluate("document.body.scrollWidth") == width
    for terminal in page.locator(".term").all():
        box = terminal.bounding_box()
        assert box["x"] >= 0 and box["x"] + box["width"] <= width
        expect(terminal).to_have_attribute("tabindex", "0")


def test_body_command_names_use_monospace(homepage: Page) -> None:
    names = homepage.locator(".strip code, .lede code")
    expect(names).to_have_count(2)
    for name in names.all():
        expect(name).to_have_text("lf")
        assert "JetBrains Mono" in name.evaluate("el => getComputedStyle(el).fontFamily")


def test_landing_variant_url_returns_404(page: Page, base_url: str):
    response = page.goto(f"{base_url}/landing/blend")
    assert response is not None
    assert response.status == 404


def test_legacy_redirects(page: Page, base_url: str):
    page.goto(f"{base_url}/agents")
    assert "/docs" in page.url
    page.goto(f"{base_url}/team")
    assert page.url == f"{base_url}/"
    page.goto(f"{base_url}/story")
    assert page.url == f"{base_url}/"
    page.goto(f"{base_url}/loopflow")
    assert page.url == f"{base_url}/download"


def test_llms_txt(page: Page, base_url: str):
    response = page.goto(f"{base_url}/llms.txt")
    assert response is not None
    assert response.status == 200
    body = response.text()
    assert "/docs" in body
    assert "install.sh" in body
