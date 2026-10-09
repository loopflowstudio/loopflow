"""Homepage sections, examples, destinations, and capture layout."""

from urllib.parse import urlsplit

import pytest
from playwright.sync_api import Page, expect


def test_approved_homepage_sections(homepage: Page) -> None:
    expect(homepage.locator("h1")).to_have_text("Loopflow lets anyone craft software.")
    assert homepage.locator(".home-hero .needs-list li").count() == 2
    assert homepage.locator("main > section[id]").evaluate_all(
        "sections => sections.map(section => section.id)"
    ) == ["mac", "faq", "start"]
    assert homepage.locator(".claim .number").all_text_contents() == [
        f"{number:02}" for number in range(1, 5)
    ]
    expect(homepage.locator(".faq dt")).to_have_count(9)


def test_hero_leads_with_the_mac_app(homepage: Page) -> None:
    hero = homepage.locator(".home-hero")
    expect(hero.get_by_role("link", name="Download for Mac")).to_have_attribute(
        "href", "https://downloads.loopflow.studio/Loopflow-latest.dmg"
    )
    expect(hero.locator(".cost")).to_contain_text("has its own cost")
    assert hero.locator(".window img").bounding_box()["width"] > 1000
    hero.get_by_role("link", name="Or install the command line ↓").click()
    expect(homepage.locator("#start h2")).to_be_in_viewport()


def test_cli_page_makes_the_case_for_engineers(page: Page, base_url: str) -> None:
    page.goto(f"{base_url}/cli")
    expect(page.locator("h1")).to_have_attribute(
        "aria-label",
        "lf is the CLI for writing, running, sharing and studying agent workflows.",
    )
    expect(page.locator("h1 .rotating span")).to_have_count(4)
    assert page.locator("main > section[id]").evaluate_all(
        "sections => sections.map(section => section.id)"
    ) == ["flows", "where", "start"]
    assert "loopflow.studio/install.sh" in page.locator(".home-hero .install-row pre").text_content()
    expect(page.locator(".home-hero").get_by_role("button", name="Copy", exact=True)).to_be_visible()
    expect(page.locator(".home-hero .eyebrow, #flows h2")).to_have_count(0)
    assert page.locator("#flows .plain li b").all_text_contents()[:2] == [
        "Flows",
        "Steps",
    ]
    expect(page.get_by_role("link", name="Download for Mac")).to_have_count(0)
    expect(page.locator("main img")).to_have_count(0)


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
