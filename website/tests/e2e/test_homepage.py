"""Homepage structure tests.

These pin structure — sections exist, links resolve, assets load — not copy.
Copy lives in content.yaml and should be editable without touching tests.
"""

from urllib.parse import urlsplit

from playwright.sync_api import Page, expect


def test_hero_elements_visible(homepage: Page):
    assert homepage.locator("h1", has_text="Loopflow").is_visible()
    tagline = homepage.locator(".hero .tagline")
    assert tagline.is_visible()
    assert tagline.text_content().strip()


def test_hero_ctas(homepage: Page):
    hero = homepage.locator(".hero")
    assert hero.locator(".hero-subline").text_content().strip()
    ctas = hero.locator("a.btn")
    assert ctas.count() >= 2
    hrefs = [ctas.nth(i).get_attribute("href") for i in range(ctas.count())]
    assert "/docs" in hrefs
    assert any(h.endswith(".dmg") for h in hrefs), "hero must offer the Mac app"


def test_getting_started_links_reach_guides(homepage: Page, base_url: str) -> None:
    section = homepage.locator(".getting-started-section")
    expect(section.get_by_role("listitem")).to_have_count(4)
    links = section.get_by_role("link")
    destinations = [link.get_attribute("href") for link in links.all()]
    assert len(destinations) == 2
    for destination in destinations:
        response = homepage.goto(f"{base_url}{destination}")
        assert response is not None and response.ok
        fragment = urlsplit(destination).fragment
        expect(homepage.locator(f'[id="{fragment}"]')).to_have_count(1)


def test_building_blocks(homepage: Page):
    section = homepage.locator(".building-blocks-section")
    assert section.is_visible()
    assert section.locator(".code-block").count() >= 1


def test_features_explain_subsystems_and_link_to_guides(homepage: Page, base_url: str) -> None:
    features = homepage.locator("#features article")
    assert features.count() == 6
    destinations = []
    for feature in features.all():
        expect(feature.locator("h3")).not_to_be_empty()
        expect(feature.locator(".feature-summary")).not_to_be_empty()
        expect(feature.locator(".feature-description")).not_to_be_empty()
        destinations.append(feature.get_by_role("link").get_attribute("href"))

    for destination in destinations:
        response = homepage.goto(f"{base_url}{destination}")
        assert response is not None and response.ok
        fragment = urlsplit(destination).fragment
        if fragment:
            expect(homepage.locator(f'[id="{fragment}"]')).to_have_count(1)


def test_wave_building_block_does_not_define_measures(homepage: Page):
    section = homepage.locator(".building-blocks-section")
    wave = section.locator(".building-block-item", has_text="wave/auth/GOAL.md")
    assert wave.count() == 1
    assert "## Measures" not in wave.text_content()


def test_screenshot_section_only_when_capture_exists(homepage: Page, base_url: str):
    """The demo section renders only when the capture file is present —
    a missing image never ships as a 404."""
    section = homepage.locator(".loopflow-showcase-section")
    if section.count():
        figures = section.locator("figure")
        assert figures.count() >= 1
        for i in range(figures.count()):
            src = figures.nth(i).locator("img").get_attribute("src")
            response = homepage.request.get(f"{base_url}{src}")
            assert response.ok, f"screenshot {src} rendered but does not resolve"


def test_no_legacy_homepage_sections(homepage: Page):
    assert homepage.locator(".hero-video-section").count() == 0
    assert homepage.locator(".products-section").count() == 0
    assert homepage.locator(".vocab-section").count() == 0
    assert homepage.locator(".story-section").count() == 0
    assert homepage.locator(".terminal-section").count() == 0
    assert homepage.locator("form").count() == 0  # no waitlist


def test_homepage_images_resolve(homepage: Page, base_url: str):
    imgs = homepage.locator("main img, nav img")
    for i in range(imgs.count()):
        src = imgs.nth(i).get_attribute("src")
        assert src, "image without src"
        response = homepage.request.get(f"{base_url}{src}" if src.startswith("/") else src)
        assert response.ok, f"image {src} does not resolve"


def test_install_code_in_bottom_cta(homepage: Page):
    bottom_cta = homepage.locator(".quick-install")
    assert bottom_cta.is_visible()
    install_code = bottom_cta.locator(".install-code code").first
    assert "loopflow.studio/install.sh" in install_code.text_content()
    assert bottom_cta.locator(".copy-btn").first.is_visible()


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


def test_mobile_nav_visible(page: Page, base_url: str):
    page.set_viewport_size({"width": 375, "height": 667})
    page.goto(base_url)
    nav_links = page.locator(".nav-links")
    assert nav_links.is_visible()
