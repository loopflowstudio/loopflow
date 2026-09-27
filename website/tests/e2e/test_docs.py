from pathlib import Path
from urllib.parse import unquote, urlsplit

from playwright.sync_api import Page

from dev import INSTALL_REWRITES

# Slugs the sidebar must offer; content of each page is not pinned here.
NAV_SLUGS = [
    "index",
    "glossary",
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
        target = "/docs" if slug == "index" else f"/docs/{slug}"
        assert target in hrefs, f"sidebar missing {target}"


def test_every_nav_doc_renders(page: Page, base_url: str):
    for slug in NAV_SLUGS:
        page.goto(f"{base_url}/docs/{slug}")
        assert page.url.endswith(f"/docs/{slug}"), f"/docs/{slug} redirected (missing doc?)"
        content = page.locator(".docs-content")
        assert content.locator("h1").first.is_visible(), f"/docs/{slug} has no h1"


def test_public_docs_links_and_markdown_match_sources(page: Page, base_url: str) -> None:
    sources = Path(__file__).resolve().parents[3] / "docs"
    fragments = {}
    links = []
    for slug in NAV_SLUGS:
        path = "/docs" if slug == "index" else f"/docs/{slug}"
        page.goto(f"{base_url}{path}")
        content = page.locator(".docs-content")
        fragments[path] = set(
            content.locator("[id]").evaluate_all("elements => elements.map(element => element.id)")
        )
        links.extend(
            (path, href)
            for href in content.locator("a[href]").evaluate_all(
                "elements => elements.map(element => element.href)"
            )
        )
        markdown = page.request.get(f"{base_url}/docs/{slug}.md")
        negotiated = page.request.get(f"{base_url}{path}", headers={"Accept": "text/markdown"})
        assert markdown.status == negotiated.status == 200
        assert markdown.text() == negotiated.text()
        source = (sources / f"{slug}.md").read_text()
        for original, public in INSTALL_REWRITES:
            source = source.replace(original, public)
        assert markdown.text().endswith(source)

    for source, href in links:
        target = urlsplit(href)
        if target.netloc != urlsplit(base_url).netloc or not target.path.startswith("/docs"):
            continue
        path = target.path.removesuffix(".md")
        path = "/docs" if path == "/docs/index" else path
        assert path in fragments, f"{source} links to missing public page {href}"
        if target.fragment:
            assert unquote(target.fragment) in fragments[path], f"{source} has broken link {href}"


def test_docs_sidebar_navigation(page: Page, base_url: str):
    page.goto(f"{base_url}/docs")
    page.locator(".docs-nav").locator("a", has_text="Config").click()
    assert "/docs/config" in page.url


def test_docs_security_page_states_forwarded_authority_guarantees(page: Page, base_url: str):
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
    assert "not a hard containment boundary" in text
    assert "Codex then bypasses vendor approvals" in text
    assert "do not restrict all other access" in text


def test_docs_subscriptions_page_owns_account_selection(page: Page, base_url: str):
    page.goto(f"{base_url}/docs/subscriptions")
    content = page.locator(".docs-content")
    assert content.is_visible()
    text = content.inner_text()
    assert "--only-account" in text
    assert "repository account route" in text
    assert "target-side --account" in text
    assert "local or forwarded" in text
    assert "whether the login belongs to this" in text
    assert "machine or was offered by the origin" in text


def test_docs_nonexistent_redirects(page: Page, base_url: str):
    page.goto(f"{base_url}/docs/nonexistent-page")
    assert page.url == f"{base_url}/docs"


def test_developer_architecture_is_hidden_from_public_docs(page: Page, base_url: str):
    page.goto(f"{base_url}/docs")
    assert page.locator('.docs-nav a[href="/docs/architecture"]').count() == 0

    page.goto(f"{base_url}/docs/architecture")
    assert page.url == f"{base_url}/docs"


def test_developer_architecture_has_its_own_reading_surface(page: Page, base_url: str):
    page.goto(f"{base_url}/architecture")
    assert page.locator('meta[name="robots"]').get_attribute("content") == "noindex,nofollow"
    assert page.locator(".docs-content h1").inner_text() == "Architecture"
    assert page.locator(".docs-nav-architecture").is_visible()
    assert page.locator(".docs-outline").is_visible()
    assert (
        page.locator(".docs-content").evaluate("el => getComputedStyle(el).backgroundColor")
        == "rgba(0, 0, 0, 0)"
    )
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
