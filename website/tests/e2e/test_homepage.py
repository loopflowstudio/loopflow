"""Homepage sections, examples, destinations, and capture layout."""

from urllib.parse import urlsplit

import pytest
from playwright.sync_api import Page, expect


def test_homepage_names_loopflow_and_coding_agents(homepage: Page) -> None:
    expect(homepage.locator("h1")).to_have_attribute(
        "aria-label",
        "Loopflow is how you design, implement, demo, release and study software with agents.",
    )
    expect(homepage.locator("h1 .rotating span")).to_have_count(5)
    expect(homepage.locator(".home-hero .either")).to_have_text(
        "Conduct an orchestra of coding agents and hear every part clearly."
    )
    assert homepage.locator("main > section[id]").evaluate_all(
        "sections => sections.map(section => section.id)"
    ) == ["mac", "works"]


def test_hero_offers_the_mac_app_or_the_cli(homepage: Page) -> None:
    hero = homepage.locator(".home-hero")
    expect(hero.get_by_role("link", name="Download for Mac")).to_have_attribute(
        "href", "https://downloads.loopflow.studio/Loopflow-latest.dmg"
    )
    assert "loopflow.studio/install.sh" in hero.locator(".install-row pre").text_content()
    expect(hero.get_by_role("button", name="Copy", exact=True)).to_be_visible()
    expect(homepage.locator("#mac .win")).to_be_in_viewport()


def test_cli_page_makes_the_case_for_engineers(page: Page, base_url: str) -> None:
    page.goto(f"{base_url}/cli")
    expect(page.locator("h1")).to_have_attribute(
        "aria-label",
        "lf is the CLI for writing, running, sharing and studying agent workflows.",
    )
    expect(page.locator("h1 .rotating span")).to_have_count(4)
    assert page.locator("main > section[id]").evaluate_all(
        "sections => sections.map(section => section.id)"
    ) == ["graphs", "where", "start"]
    assert "loopflow.studio/install.sh" in page.locator("#start .install-row pre").text_content()
    expect(page.locator("#start").get_by_role("button", name="Copy", exact=True)).to_be_visible()
    expect(page.locator(".home-hero .install-row")).to_have_count(0)
    expect(page.locator(".home-hero .eyebrow, #graphs h2")).to_have_count(0)
    assert page.locator("#graphs .plain li b").all_text_contents()[:2] == [
        "Launcher",
        "Skills",
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


def test_pictures_and_sections_tell_the_page(homepage: Page) -> None:
    expect(homepage.locator("#mac .tasks li")).to_have_count(5)
    expect(homepage.locator(".system .wfs > li code")).to_have_text(["feature", "code", "research"])
    card = homepage.locator(".clihero .term").text_content()
    for command in ("lf wt create", "--machine mini", "lf pr open", "lf land --wait-and-fix", "lf account"):
        assert command in card
    expect(homepage.locator("#works li")).to_have_count(5)
    for link in homepage.locator("#works a").all():
        expect(link).to_have_attribute("target", "_blank")
    expect(homepage.locator(".belief").get_by_role("link", name="Download for Mac")).to_have_attribute(
        "href", "https://downloads.loopflow.studio/Loopflow-latest.dmg"
    )


def test_no_superseded_or_review_content(homepage: Page) -> None:
    expect(
        homepage.locator(".home-stage, .home-autonomy, .review-note, .review-bar, form")
    ).to_have_count(0)
    body = homepage.locator("body").text_content()
    for text in ("Review note for Jack", "goes here", "stands in", "localhost"):
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
    names = homepage.locator(".clihero article p code")
    assert names.count() >= 2
    for name in names.all():
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
