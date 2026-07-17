"""Story page structure tests."""

from playwright.sync_api import Page


def test_story_page_loads(page: Page, base_url: str):
    page.goto(f"{base_url}/story")
    content = page.locator(".docs-content")
    assert content.is_visible()
    assert content.locator("h1").first.is_visible()


def test_story_page_has_sections(page: Page, base_url: str):
    page.goto(f"{base_url}/story")
    assert page.locator(".docs-content h2").count() >= 2


def test_story_links_resolve_locally(page: Page, base_url: str):
    page.goto(f"{base_url}/story")
    links = page.locator(".docs-content a")
    for i in range(links.count()):
        href = links.nth(i).get_attribute("href")
        assert href, "link without href"
        if href.startswith("/"):
            response = page.request.get(f"{base_url}{href}")
            assert response.ok, f"internal link {href} does not resolve"
