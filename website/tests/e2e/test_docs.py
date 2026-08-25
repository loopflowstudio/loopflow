from playwright.sync_api import Page

# Slugs the sidebar must offer; content of each page is not pinned here.
NAV_SLUGS = [
    "getting-started",
    "waves",
    "authoring",
    "agent-api",
    "conducting",
    "lf",
    "config",
    "subscriptions",
    "security",
    "troubleshooting",
]


def test_docs_page_loads(page: Page, base_url: str):
    page.goto(f"{base_url}/docs")
    assert page.locator(".docs-layout").is_visible()
    assert page.locator(".docs-nav").is_visible()
    assert page.locator(".docs-content").is_visible()
    assert page.locator(".docs-nav-area-title").all_text_contents() == [
        "Start",
        "Plan and conduct",
        "Build and extend",
        "Reference",
    ]


def test_docs_sidebar_links_resolve(page: Page, base_url: str):
    page.goto(f"{base_url}/docs")
    nav = page.locator(".docs-nav")
    hrefs = [nav.locator("a").nth(i).get_attribute("href") for i in range(nav.locator("a").count())]
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


def test_docs_security_page_states_forwarded_authority_guarantees(
    page: Page, base_url: str
):
    page.goto(f"{base_url}/docs/security")
    content = page.locator(".docs-content")
    assert content.is_visible()
    text = content.inner_text()
    assert "Only the first is general containment" in text
    assert "workspace-write" in text
    assert "fail closed" in text
    assert "Doppler master credential never" in text
    assert "second SSH" in text
    assert "per-process control capability" in text


def test_docs_subscriptions_page_owns_account_selection(
    page: Page, base_url: str
):
    page.goto(f"{base_url}/docs/subscriptions")
    content = page.locator(".docs-content")
    assert content.is_visible()
    text = content.inner_text()
    assert "--only-account" in text
    assert "repository account route" in text
    assert "target-side --account" in text
    assert "local or forwarded provenance" in text


def test_docs_nonexistent_redirects(page: Page, base_url: str):
    page.goto(f"{base_url}/docs/nonexistent-page")
    assert page.url == f"{base_url}/docs"


def test_developer_architecture_is_hidden_from_public_docs(page: Page, base_url: str):
    page.goto(f"{base_url}/docs")
    assert page.locator('.docs-nav a[href="/docs/architecture"]').count() == 0

    page.goto(f"{base_url}/docs/architecture")
    assert page.url == f"{base_url}/docs"


def test_developer_architecture_has_its_own_reading_surface(
    page: Page, base_url: str
):
    page.goto(f"{base_url}/architecture")
    assert page.locator('meta[name="robots"]').get_attribute("content") == "noindex,nofollow"
    assert page.locator(".docs-content h1").inner_text() == "Architecture"
    assert page.locator(".docs-nav-architecture").is_visible()
    assert page.locator(".docs-outline").is_visible()
    assert page.locator(".docs-content").evaluate(
        "el => getComputedStyle(el).backgroundColor"
    ) == "rgba(0, 0, 0, 0)"
    assert page.locator('a[href="/architecture/execution"]').count() >= 1
    assert page.locator(".docs-md-link a").get_attribute("href") == "/architecture.md"

    response = page.request.get(f"{base_url}/architecture")
    assert response.headers["x-robots-tag"] == "noindex, nofollow"


def test_architecture_area_routes_render(page: Page, base_url: str):
    for slug in (
        "execution",
        "planning",
        "delivery",
        "homes",
        "data",
        "codebase",
        "reference",
    ):
        page.goto(f"{base_url}/architecture/{slug}")
        assert page.url.endswith(f"/architecture/{slug}")
        assert page.locator(".docs-content h1").is_visible()


def test_retired_docs_redirect(page: Page, base_url: str):
    # wave-authoring merged into waves, ops merged into lf, fleet renamed
    # to conducting; no retired slug may 500
    for retired in ("wave-authoring", "ops", "fleet"):
        page.goto(f"{base_url}/docs/{retired}")
        assert page.url == f"{base_url}/docs"
