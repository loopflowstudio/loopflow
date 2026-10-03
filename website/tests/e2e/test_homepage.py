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


def test_example_loopflow_names_its_steps(homepage: Page) -> None:
    diagram = homepage.locator(".hero svg[role=img]")
    expect(diagram).to_be_visible()
    assert diagram.locator("circle").count() >= 2
    assert "returns to" in diagram.get_attribute("aria-label")


def test_ownership_strip_lists_what_stays_yours(homepage: Page) -> None:
    cells = homepage.locator(".home-owned h2")
    assert cells.count() >= 3
    for cell in cells.all():
        expect(cell).not_to_be_empty()


def test_plan_links_what_exists_to_where_it_is_shown(homepage: Page) -> None:
    stages = homepage.locator(".home-way")
    expect(stages).to_have_count(3)
    for stage in stages.all():
        expect(stage.locator("h3")).not_to_be_empty()
        for link in stage.get_by_role("link").all():
            expect(homepage.locator(link.get_attribute("href"))).to_have_count(1)


def test_autonomy_levels_mark_where_the_person_comes_in(homepage: Page, base_url: str) -> None:
    levels = homepage.locator(".home-level")
    expect(levels).to_have_count(3)
    present = [level.locator("li.you").count() for level in levels.all()]
    assert present[0] > 0 and present[-1] == 0
    destination = homepage.locator(".home-legend a").get_attribute("href")
    response = homepage.goto(f"{base_url}{destination}")
    assert response is not None and response.ok
    expect(homepage.locator(f'[id="{urlsplit(destination).fragment}"]')).to_have_count(1)


def test_features_explain_subsystems_and_link_to_guides(homepage: Page, base_url: str) -> None:
    features = homepage.locator("#features article")
    assert features.count() == 6
    destinations = []
    for feature in features.all():
        expect(feature.locator("h3")).not_to_be_empty()
        expect(feature.locator(".feature-description")).not_to_be_empty()
        destinations.append(feature.get_by_role("link").get_attribute("href"))

    for destination in destinations:
        response = homepage.goto(f"{base_url}{destination}")
        assert response is not None and response.ok
        fragment = urlsplit(destination).fragment
        if fragment:
            expect(homepage.locator(f'[id="{fragment}"]')).to_have_count(1)


def test_product_window_steps_through_its_captures(homepage: Page) -> None:
    stage = homepage.locator(".home-stage")
    tabs = homepage.locator(".home-stage-tabs button")
    assert tabs.count() >= 2
    last = tabs.nth(tabs.count() - 1)
    last.click()
    expect(last).to_have_attribute("aria-pressed", "true")
    expect(stage.locator(".home-shot.is-current")).to_have_attribute(
        "data-frame", last.get_attribute("data-frame")
    )
    expect(homepage.locator(".home-stage-caption")).to_have_text(last.get_attribute("data-caption"))


def test_desktop_pictures_open_at_full_size(homepage: Page, base_url: str) -> None:
    links = homepage.locator(".home-stage-links a")
    sources = homepage.locator(".home-stage img").evaluate_all(
        "images => images.map(image => image.getAttribute('src'))"
    )
    assert [link.get_attribute("href") for link in links.all()] == sources
    for source in sources:
        response = homepage.request.get(f"{base_url}{source}")
        assert response.ok, f"screenshot {source} rendered but does not resolve"


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
