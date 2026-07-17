"""The agent-facing retrieval layer: raw markdown, llms.txt, sitemap.

Most readers of these docs are agents. They fetch markdown, not HTML —
these tests pin the .md endpoints, content negotiation, markdown 404s,
and the machine indexes.
"""

from playwright.sync_api import Page


def test_doc_md_endpoint(page: Page, base_url: str):
    response = page.request.get(f"{base_url}/docs/waves.md")
    assert response.status == 200
    assert "text/markdown" in response.headers["content-type"]
    body = response.text()
    assert body.startswith("---\n")
    assert "canonical_url: https://loopflow.studio/docs/waves" in body
    assert "# Waves" in body


def test_content_negotiation(page: Page, base_url: str):
    response = page.request.get(f"{base_url}/docs/waves", headers={"Accept": "text/markdown"})
    assert response.status == 200
    assert "text/markdown" in response.headers["content-type"]
    assert response.headers.get("vary") == "Accept"

    # FastHTML adds its own Vary on HTML responses; ours must survive beside it.
    html = page.request.get(f"{base_url}/docs/waves", headers={"Accept": "text/html"})
    assert html.status == 200
    assert "Accept" in html.headers.get("vary", "")


def test_markdown_404_suggests_pages(page: Page, base_url: str):
    response = page.request.get(f"{base_url}/docs/wavez.md")
    assert response.status == 404
    assert "text/markdown" in response.headers["content-type"]
    body = response.text()
    assert "/docs/waves.md" in body
    assert "/llms.txt" in body


def test_llms_txt_is_spec_shaped(page: Page, base_url: str):
    body = page.request.get(f"{base_url}/llms.txt").text()
    lines = body.splitlines()
    assert lines[0] == "# Loopflow"
    assert lines[1].startswith("> ")
    assert "## Docs" in body
    assert "/docs/agent-api.md)" in body
    assert "/llms-full.txt" in body


def test_llms_full_txt(page: Page, base_url: str):
    response = page.request.get(f"{base_url}/llms-full.txt")
    assert response.status == 200
    body = response.text()
    assert "# Waves" in body
    assert "# The Agent API" in body


def test_sitemap(page: Page, base_url: str):
    response = page.request.get(f"{base_url}/sitemap.xml")
    assert response.status == 200
    body = response.text()
    assert "<loc>https://loopflow.studio/docs/waves</loc>" in body
    assert "<loc>https://loopflow.studio/story</loc>" not in body
    assert "<lastmod>" in body


def test_view_as_markdown_link(page: Page, base_url: str):
    page.goto(f"{base_url}/docs/waves")
    link = page.locator(".docs-md-link a")
    assert link.get_attribute("href") == "/docs/waves.md"
