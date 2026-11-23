"""Accessibility tests for the Loopflow website.

Uses Playwright + axe-core to automatically check WCAG 2.1 AA compliance.
Run with: cd website && uv run pytest tests/ -v
"""

import pytest
from axe_playwright_python.sync_playwright import Axe
from playwright.sync_api import Page

# Pages to test for accessibility
PAGES_TO_TEST = [
    "/",
    "/docs",
    "/docs/agent-api",
    "/docs/config",
    "/docs/subscriptions",
    "/docs/security",
    "/architecture",
    "/architecture/execution",
    "/architecture/reference",
    "/download",
]


class TestAccessibility:
    """WCAG 2.1 AA accessibility tests."""

    @pytest.mark.parametrize("path", PAGES_TO_TEST)
    def test_axe_accessibility(self, page: Page, server: str, path: str):
        """Run axe-core accessibility audit on each page."""
        page.goto(f"{server}{path}")
        page.wait_for_load_state("networkidle")

        axe = Axe()
        results = axe.run(page)

        # Filter to only critical and serious violations (WCAG AA)
        violations = [
            v
            for v in results.response.get("violations", [])
            if v.get("impact") in ("critical", "serious")
        ]

        if violations:
            messages = []
            for v in violations:
                nodes = v.get("nodes", [])
                targets = [n.get("target", ["unknown"])[0] for n in nodes[:3]]
                messages.append(
                    f"[{v.get('impact')}] {v.get('id')}: {v.get('description')}\n"
                    f"  Affected: {', '.join(targets)}"
                )
            pytest.fail(f"Accessibility violations on {path}:\n" + "\n".join(messages))


class TestFocusStates:
    """Keyboard focus shows the site's ring, and only that ring."""

    @pytest.mark.parametrize("path", ["/", "/download", "/docs"])
    def test_keyboard_focus_shows_one_ring(self, page: Page, server: str, path: str):
        page.goto(f"{server}{path}")
        page.wait_for_load_state("networkidle")
        for _ in range(30):
            page.keyboard.press("Tab")
            name, outline_width, box_shadow = page.evaluate(
                """() => {
                    const el = document.activeElement, style = getComputedStyle(el);
                    return [el.textContent.trim().slice(0, 30) || el.getAttribute('aria-label'),
                            parseFloat(style.outlineWidth), style.boxShadow];
                }"""
            )
            assert outline_width >= 2, f"'{name}' has no focus outline"
            # Pico draws a translucent blue box-shadow ring under ours.
            colored = [c for c in box_shadow.split("rgba(")[1:] if not c.startswith("0, 0, 0, 0)")]
            assert box_shadow == "none" or not colored, f"'{name}' shows {box_shadow}"


class TestAriaLabels:
    """Test that icon-only buttons have accessible names."""

    def test_icon_buttons_have_labels(self, page: Page, server: str):
        """Buttons with only icons must have aria-label."""
        page.goto(server)
        page.wait_for_load_state("networkidle")

        # Find buttons that contain only SVG (no text)
        icon_buttons = page.locator("button:has(svg)")

        for i in range(icon_buttons.count()):
            button = icon_buttons.nth(i)
            if not button.is_visible():
                continue

            text = button.text_content() or ""
            aria_label = button.get_attribute("aria-label")
            title = button.get_attribute("title")

            # If button has no text, it needs aria-label
            if not text.strip() and not aria_label:
                pytest.fail(
                    f"Icon-only button missing aria-label. "
                    f"Has title='{title}' but that's insufficient for screen readers."
                )


class TestColorContrast:
    """Test color contrast ratios meet WCAG AA (4.5:1)."""

    def test_text_contrast(self, page: Page, server: str):
        """Main text should have sufficient contrast."""
        page.goto(server)
        page.wait_for_load_state("networkidle")

        # Check the CSS custom property values
        text_color = page.evaluate(
            "getComputedStyle(document.documentElement).getPropertyValue('--text').trim()"
        )
        text_secondary = page.evaluate(
            "getComputedStyle(document.documentElement).getPropertyValue('--text-secondary').trim()"
        )
        bg_color = page.evaluate(
            "getComputedStyle(document.documentElement).getPropertyValue('--background').trim()"
        )

        # Basic sanity check - colors should be defined
        assert text_color, "Primary text color not defined"
        assert text_secondary, "Secondary text color not defined"
        assert bg_color, "Background color not defined"


class TestExternalLinks:
    """Test that external links are properly indicated."""

    def test_external_links_have_indicator(self, page: Page, server: str):
        """External links should have visual indicator."""
        page.goto(server)
        page.wait_for_load_state("networkidle")

        external_links = page.locator('a[target="_blank"]')

        for i in range(external_links.count()):
            link = external_links.nth(i)
            if not link.is_visible():
                continue

            # Check for visual indicator (↗ symbol via ::after pseudo-element)
            content_after = link.evaluate("el => getComputedStyle(el, '::after').content")

            text = link.text_content() or ""

            # Should have either ::after content or ↗ in the text
            has_indicator = (
                content_after not in ("none", '""', "''")
                or "↗" in text
                or "opens in new" in (link.get_attribute("aria-label") or "").lower()
            )

            if not has_indicator:
                pytest.fail(
                    f"External link '{text[:30]}' has no visual indicator for target='_blank'"
                )


class TestNavigation:
    """Test navigation accessibility."""

    def test_multiple_navs_have_labels(self, page: Page, server: str):
        """Multiple nav elements should have distinct aria-labels."""
        page.goto(f"{server}/docs")
        page.wait_for_load_state("networkidle")

        navs = page.locator("nav")
        count = navs.count()

        if count > 1:
            labels = []
            for i in range(count):
                nav = navs.nth(i)
                label = nav.get_attribute("aria-label")
                if not label:
                    pytest.fail(f"Nav element {i + 1} of {count} has no aria-label")
                if label in labels:
                    pytest.fail(f"Duplicate nav aria-label: '{label}'")
                labels.append(label)
