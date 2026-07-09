from playwright.sync_api import Page


def test_docs_page_loads(page: Page, base_url: str):
    page.goto(f"{base_url}/docs")
    assert page.locator(".docs-layout").is_visible()
    assert page.locator(".docs-nav").is_visible()
    assert page.locator(".docs-content").is_visible()


def test_docs_sidebar_has_links(page: Page, base_url: str):
    page.goto(f"{base_url}/docs")
    nav = page.locator(".docs-nav")
    assert nav.locator("a", has_text="Home").is_visible()
    assert nav.locator("a", has_text="Config").is_visible()
    assert nav.locator("a", has_text="Get Started").is_visible()


def test_docs_sidebar_navigation(page: Page, base_url: str):
    page.goto(f"{base_url}/docs")
    page.locator(".docs-nav").locator("a", has_text="Config").click()
    assert "/docs/config" in page.url


def test_docs_config_page(page: Page, base_url: str):
    page.goto(f"{base_url}/docs/config")
    content = page.locator(".docs-content")
    assert content.is_visible()


def test_docs_ops_page(page: Page, base_url: str):
    page.goto(f"{base_url}/docs/ops")
    content = page.locator(".docs-content")
    assert content.is_visible()


def test_docs_nonexistent_redirects(page: Page, base_url: str):
    page.goto(f"{base_url}/docs/nonexistent-page")
    assert page.url == f"{base_url}/docs"
