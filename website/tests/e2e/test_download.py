from playwright.sync_api import Page, expect


def test_download_page_loads(page: Page, base_url: str):
    page.goto(f"{base_url}/download")
    assert page.locator("h1", has_text="Install").is_visible()


def test_cli_install_section(page: Page, base_url: str):
    page.goto(f"{base_url}/download")
    cli_section = page.locator(".install-option")
    assert cli_section.locator("h2", has_text="Command line").is_visible()
    assert cli_section.locator("code", has_text="loopflow.studio/install.sh").is_visible()


def test_install_command_fits_without_scrolling(page: Page, base_url: str):
    page.set_viewport_size({"width": 1440, "height": 900})
    page.goto(f"{base_url}/download")
    # Measure in the site's mono; a wider fallback face would overflow before it loads.
    assert page.evaluate(
        "document.fonts.ready.then(() => document.fonts.check('16px \"Spline Sans Mono\"'))"
    )
    command = page.locator(".install-code").first
    assert command.evaluate("el => el.scrollWidth <= el.clientWidth")


def test_copy_button_copies_and_says_so(page: Page, base_url: str):
    page.context.grant_permissions(["clipboard-read", "clipboard-write"])
    page.goto(f"{base_url}/download")
    button = page.locator(".copy-btn")
    button.click()
    expect(button).to_have_text("Copied")
    copied = page.evaluate("navigator.clipboard.readText()")
    assert copied == "curl -fsSL https://loopflow.studio/install.sh | sh"


def test_mac_app_section(page: Page, base_url: str):
    page.goto(f"{base_url}/download")
    mac_section = page.locator(".mac-app-option")
    assert mac_section.locator("h2", has_text="Mac app").is_visible()
    dmg_link = mac_section.locator("a", has_text="Download for Mac")
    assert dmg_link.get_attribute("href").endswith(".dmg")


def test_products_redirects_to_home(page: Page, base_url: str):
    page.goto(f"{base_url}/products")
    assert page.url == f"{base_url}/"


def test_unknown_page_is_a_site_page(page: Page, base_url: str):
    response = page.goto(f"{base_url}/no-such-page")
    assert response.status == 404
    assert page.locator("h1", has_text="Page not found").is_visible()
    assert page.locator(".site-header").is_visible()
    assert page.locator(".site-footer").is_visible()
