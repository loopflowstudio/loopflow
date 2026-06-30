from playwright.sync_api import Page


TAGLINE = "Coding agents that work in a team and build over time."
CONCERTO_DMG_URL = "https://downloads.loopflow.studio/LoopflowConcerto-latest.dmg"


def test_hero_elements_visible(homepage: Page):
    assert homepage.locator("h1", has_text="Loopflow").is_visible()
    assert homepage.locator(".tagline", has_text=TAGLINE).is_visible()


def test_tagline(homepage: Page):
    assert homepage.locator(".tagline", has_text=TAGLINE).is_visible()


def test_download_concerto_cta(homepage: Page):
    cta = homepage.locator(".hero a", has_text="Download for Mac").first
    assert cta.is_visible()
    assert cta.get_attribute("href") == CONCERTO_DMG_URL


def test_docs_cta_navigates_to_docs(homepage: Page):
    homepage.locator(".hero a", has_text="Read the docs").first.click()
    assert "/docs" in homepage.url


def test_hero_video(homepage: Page):
    section = homepage.locator(".hero-video-section")
    assert section.is_visible()
    assert section.locator("video.demo-video").is_visible()
    assert section.locator("video.demo-video").get_attribute("poster") == "/static/concerto-main.png"


def test_capabilities_section(homepage: Page):
    section = homepage.locator(".capabilities-section")
    assert section.is_visible()
    assert section.locator(".capability-item").count() == 6


def test_building_blocks(homepage: Page):
    section = homepage.locator(".building-blocks-section")
    assert section.is_visible()
    assert section.locator(".code-block").count() == 4


def test_products_section(homepage: Page):
    section = homepage.locator(".products-section")
    assert section.is_visible()
    assert section.locator(".product-card").count() == 2
    assert section.locator("h3", has_text="Concerto").is_visible()
    assert section.locator("h3", has_text="Server").is_visible()


def test_no_legacy_homepage_sections(homepage: Page):
    assert homepage.locator(".variant-toggle").count() == 0
    assert homepage.locator(".vocab-section").count() == 0
    assert homepage.locator(".paired-panel").count() == 0
    assert homepage.locator(".properties-section").count() == 0
    assert homepage.locator(".use-cases-section").count() == 0


def test_copy_button_exists(homepage: Page):
    copy_btn = homepage.locator(".copy-btn").first
    assert copy_btn.is_visible()


def test_install_code_in_bottom_cta(homepage: Page):
    bottom_cta = homepage.locator(".quick-install")
    assert bottom_cta.is_visible()
    install_code = bottom_cta.locator(".install-code code").first
    assert "loopflow.studio/install.sh" in install_code.text_content()


def test_bottom_cta_has_download_and_docs(homepage: Page):
    bottom_cta = homepage.locator(".quick-install")
    assert bottom_cta.is_visible()
    assert bottom_cta.locator("a", has_text="Download for Mac").is_visible()
    assert bottom_cta.locator("a", has_text="Read the docs").is_visible()


def test_landing_variant_url_returns_404(page: Page, base_url: str):
    response = page.goto(f"{base_url}/landing/blend")
    assert response is not None
    assert response.status == 404


def test_agents_redirect_to_docs(page: Page, base_url: str):
    page.goto(f"{base_url}/agents")
    assert "/docs" in page.url


def test_mobile_nav_visible(page: Page, base_url: str):
    page.set_viewport_size({"width": 375, "height": 667})
    page.goto(base_url)
    nav_links = page.locator(".nav-links")
    assert nav_links.is_visible()
