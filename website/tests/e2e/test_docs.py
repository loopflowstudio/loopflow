from playwright.sync_api import Page

# Slugs the sidebar must offer; content of each page is not pinned here.
NAV_SLUGS = [
    "getting-started",
    "waves",
    "agent-api",
    "fleet",
    "architecture",
    "lf",
    "ops",
    "config",
    "troubleshooting",
]


def test_docs_page_loads(page: Page, base_url: str):
    page.goto(f"{base_url}/docs")
    assert page.locator(".docs-layout").is_visible()
    assert page.locator(".docs-nav").is_visible()
    assert page.locator(".docs-content").is_visible()


def test_docs_sidebar_links_resolve(page: Page, base_url: str):
    page.goto(f"{base_url}/docs")
    nav = page.locator(".docs-nav")
    hrefs = [
        nav.locator("a").nth(i).get_attribute("href") for i in range(nav.locator("a").count())
    ]
    for slug in NAV_SLUGS:
        assert f"/docs/{slug}" in hrefs, f"sidebar missing /docs/{slug}"


def test_every_nav_doc_renders(page: Page, base_url: str):
    for slug in NAV_SLUGS:
        page.goto(f"{base_url}/docs/{slug}")
        assert page.url.endswith(f"/docs/{slug}"), f"/docs/{slug} redirected (missing doc?)"
        content = page.locator(".docs-content")
        assert content.locator("h1").first.is_visible(), f"/docs/{slug} has no h1"


def test_docs_sidebar_navigation(page: Page, base_url: str):
    page.goto(f"{base_url}/docs")
    page.locator(".docs-nav").locator("a", has_text="Config").click()
    assert "/docs/config" in page.url


def test_docs_nonexistent_redirects(page: Page, base_url: str):
    page.goto(f"{base_url}/docs/nonexistent-page")
    assert page.url == f"{base_url}/docs"


def test_retired_docs_redirect(page: Page, base_url: str):
    # wave-authoring merged into waves; the slug must not 500
    page.goto(f"{base_url}/docs/wave-authoring")
    assert page.url == f"{base_url}/docs"
