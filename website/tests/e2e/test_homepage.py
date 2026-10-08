"""Homepage sections, examples, destinations, and capture layout."""

from urllib.parse import urlsplit

import pytest
from playwright.sync_api import Page, expect


def test_approved_homepage_sections(homepage: Page) -> None:
    expect(homepage.locator("h1")).to_have_text("A software engineering CLI for your agents.")
    assert homepage.locator("main > section[id]").evaluate_all(
        "sections => sections.map(section => section.id)"
    ) == ["flows", "mac", "faq", "start"]
    assert homepage.locator(".claim .number").all_text_contents() == [
        f"{number:02}" for number in range(1, 9)
    ]
    expect(homepage.locator(".faq dt")).to_have_count(9)


def test_hero_carries_the_install_command_and_its_cost(homepage: Page) -> None:
    hero = homepage.locator(".home-hero")
    assert "loopflow.studio/install.sh" in hero.locator("pre").text_content()
    expect(hero.get_by_role("button", name="Copy")).to_be_visible()
    expect(hero.locator(".cost")).to_contain_text("has its own cost")


def test_both_surfaces_sit_side_by_side(homepage: Page) -> None:
    agents = homepage.locator(".pane.agents").bounding_box()
    you = homepage.locator(".pane.you").bounding_box()
    assert agents["y"] == you["y"]
    assert agents["x"] + agents["width"] <= you["x"] + 1


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
    names = homepage.locator(".claim p code, .faq code")
    assert names.count() >= 4
    for name in names.all():
        assert name.text_content().startswith("lf")
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
    assert page.url == f"{base_url}/"


def test_llms_txt(page: Page, base_url: str):
    response = page.goto(f"{base_url}/llms.txt")
    assert response is not None
    assert response.status == 200
    body = response.text()
    assert "/docs" in body
    assert "install.sh" in body


def test_company_menu_reaches_the_studio_and_each_effort(page: Page, base_url: str) -> None:
    for label, path, heading in [
        ("Loopflow Studio", "/studio", "We make three things."),
        ("Etude", "/etude", "Experiment orchestration."),
        ("Fantasia", "/fantasia", "Superhuman Magic: The Gathering bots."),
        ("About", "/about", "Jack Heart"),
    ]:
        page.goto(base_url)
        page.locator("nav summary", has_text="Company").click()
        page.locator("nav .menu").get_by_role("link", name=label, exact=True).click()
        assert page.url == f"{base_url}{path}"
        expect(page.locator("h1")).to_have_text(heading)
