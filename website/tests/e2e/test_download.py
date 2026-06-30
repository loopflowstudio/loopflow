from playwright.sync_api import Page


def test_download_page_loads(page: Page, base_url: str):
    page.goto(f"{base_url}/download")
    assert page.locator("h1", has_text="Install").is_visible()


def test_cli_install_section(page: Page, base_url: str):
    page.goto(f"{base_url}/download")
    cli_section = page.locator(".install-option")
    assert cli_section.locator("h2", has_text="CLI").is_visible()
    assert cli_section.locator("code", has_text="loopflow.studio/install.sh").is_visible()


def test_products_redirects_to_home(page: Page, base_url: str):
    page.goto(f"{base_url}/products")
    assert page.url == f"{base_url}/"


def test_concerto_page_loads(page: Page, base_url: str):
    page.goto(f"{base_url}/concerto")
    assert page.locator("h1", has_text="Concerto").is_visible()


def test_symphonia_page_loads(page: Page, base_url: str):
    page.goto(f"{base_url}/symphonia")
    assert page.locator("h1", has_text="Symphonia").is_visible()
    assert page.locator("a", has_text="teams@loopflow.studio").is_visible()
