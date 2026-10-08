import pytest
from playwright.sync_api import Page, expect


def test_navbar_links_exist(homepage: Page):
    nav = homepage.locator("nav")
    assert nav.locator("a", has_text="Docs").is_visible()
    assert nav.locator("a", has_text="Flows").is_visible()
    assert nav.locator("a", has_text="Tasks").is_visible()


def test_navbar_docs_link(homepage: Page, base_url: str):
    homepage.locator("nav").locator("a", has_text="Docs").click()
    assert "/docs" in homepage.url


@pytest.mark.parametrize("path", ["/", "/docs"])
def test_flows_navigation(page: Page, base_url: str, path: str) -> None:
    page.goto(f"{base_url}{path}")
    page.get_by_role("navigation", name="Main navigation").get_by_role(
        "link", name="Flows", exact=True
    ).click()
    expect(page).to_have_url(f"{base_url}/#flows")
    heading = page.locator("#flows h2")
    expect(heading).to_be_in_viewport()
    heading_box = heading.bounding_box()
    nav_box = page.get_by_role("navigation", name="Main navigation").bounding_box()
    assert heading_box["y"] >= nav_box["y"] + nav_box["height"]


def test_navbar_install_link(homepage: Page, base_url: str):
    homepage.goto(f"{base_url}/docs")
    homepage.locator("nav").locator("a", has_text="Install").click()
    assert homepage.url == f"{base_url}/download"
    expect(homepage.locator("h1", has_text="Install")).to_be_visible()


def test_github_link_external(homepage: Page, base_url: str):
    homepage.goto(f"{base_url}/docs")
    github_link = homepage.locator("nav").locator("a", has_text="GitHub")
    assert github_link.get_attribute("href") == "https://github.com/loopflowstudio/loopflow"
    assert github_link.get_attribute("target") == "_blank"


def test_brand_link_to_home(page: Page, base_url: str):
    page.goto(f"{base_url}/docs")
    page.locator(".nav-logo").click()
    assert page.url == f"{base_url}/"


def test_cli_redirects_to_docs(page: Page, base_url: str):
    page.goto(f"{base_url}/cli")
    assert "/docs" in page.url


def test_nav_title_no_overlap_with_links_desktop(homepage: Page):
    """Nav title should not overlap with nav links on desktop."""
    nav_title = homepage.locator(".home-page .brand")
    nav_links = homepage.locator(".home-page nav")

    title_box = nav_title.bounding_box()
    links_box = nav_links.bounding_box()

    assert title_box and links_box

    # Check for overlap
    title_right = title_box["x"] + title_box["width"]
    links_left = links_box["x"]

    assert title_right <= links_left, (
        f"Nav title overlaps with nav links on desktop: "
        f"title ends at x={title_right:.0f}, links start at x={links_left:.0f}"
    )
