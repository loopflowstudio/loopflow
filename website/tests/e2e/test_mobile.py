"""Mobile visual tests for the Loopflow website.

Tests that mobile layout renders correctly without overlaps or overflow.
Run with: cd website && python dev.py test -k mobile
"""

import pytest
from playwright.sync_api import Page

# Common mobile viewport sizes
MOBILE_VIEWPORTS = [
    {"width": 375, "height": 667, "name": "iPhone SE"},
    {"width": 390, "height": 844, "name": "iPhone 12"},
    {"width": 360, "height": 640, "name": "Android small"},
]


@pytest.fixture
def mobile_page(page: Page, base_url: str):
    """Set up a mobile viewport."""
    page.set_viewport_size({"width": 375, "height": 667})
    page.goto(base_url)
    page.wait_for_load_state("networkidle")
    return page


class TestMobileNavNoOverlap:
    """Test that navigation elements don't overlap on mobile."""

    @pytest.mark.parametrize("viewport", MOBILE_VIEWPORTS, ids=lambda v: v["name"])
    def test_nav_items_no_overlap(self, page: Page, base_url: str, viewport: dict):
        """Nav items should not overlap each other."""
        page.set_viewport_size({"width": viewport["width"], "height": viewport["height"]})
        page.goto(base_url)
        page.wait_for_load_state("networkidle")

        nav_links = page.locator(".nav-links a:visible")
        count = nav_links.count()

        if count < 2:
            pytest.skip("Not enough visible nav links to test overlap")

        # Get bounding boxes of all visible nav links
        boxes = []
        for i in range(count):
            link = nav_links.nth(i)
            box = link.bounding_box()
            if box:
                boxes.append((link.text_content() or f"link_{i}", box))

        # Check no two boxes overlap
        for i, (name_a, box_a) in enumerate(boxes):
            for name_b, box_b in boxes[i + 1 :]:
                overlap = _boxes_overlap(box_a, box_b)
                assert not overlap, (
                    f"Nav items overlap on {viewport['name']}: "
                    f"'{name_a.strip()}' and '{name_b.strip()}'"
                )

    @pytest.mark.parametrize("viewport", MOBILE_VIEWPORTS, ids=lambda v: v["name"])
    def test_nav_logo_no_overlap_with_links(self, page: Page, base_url: str, viewport: dict):
        """Nav logo should not overlap with nav links."""
        page.set_viewport_size({"width": viewport["width"], "height": viewport["height"]})
        page.goto(base_url)
        page.wait_for_load_state("networkidle")

        logo = page.locator(".nav-logo")
        nav_links = page.locator(".nav-links")

        logo_box = logo.bounding_box()
        links_box = nav_links.bounding_box()

        if not logo_box or not links_box:
            pytest.skip("Could not get bounding boxes")

        overlap = _boxes_overlap(logo_box, links_box)
        assert not overlap, (
            f"Logo overlaps nav links on {viewport['name']}: "
            f"logo ends at x={logo_box['x'] + logo_box['width']:.0f}, "
            f"links start at x={links_box['x']:.0f}"
        )

    @pytest.mark.parametrize("viewport", MOBILE_VIEWPORTS, ids=lambda v: v["name"])
    def test_nav_title_no_overlap_with_links(self, page: Page, base_url: str, viewport: dict):
        """Centered nav title should not overlap with nav links on mobile."""
        page.set_viewport_size({"width": viewport["width"], "height": viewport["height"]})
        page.goto(base_url)
        page.wait_for_load_state("networkidle")

        nav_title = page.locator(".nav-title")
        nav_links = page.locator(".nav-links")

        # Skip if nav-title doesn't exist (some pages may not have it)
        if nav_title.count() == 0:
            pytest.skip("No .nav-title element")

        title_box = nav_title.bounding_box()
        links_box = nav_links.bounding_box()

        if not title_box or not links_box:
            pytest.skip("Could not get bounding boxes")

        overlap = _boxes_overlap(title_box, links_box)
        assert not overlap, (
            f"Nav title overlaps nav links on {viewport['name']}: "
            f"title at x={title_box['x']:.0f}-{title_box['x'] + title_box['width']:.0f}, "
            f"links at x={links_box['x']:.0f}-{links_box['x'] + links_box['width']:.0f}"
        )

    def test_nav_fits_in_viewport(self, mobile_page: Page):
        """Nav should not cause horizontal scroll."""
        nav = mobile_page.locator("nav")
        nav_box = nav.bounding_box()
        viewport = mobile_page.viewport_size

        if not nav_box or not viewport:
            pytest.skip("Could not get dimensions")

        assert nav_box["width"] <= viewport["width"], (
            f"Nav width ({nav_box['width']:.0f}px) exceeds viewport ({viewport['width']}px)"
        )


class TestMobileImagesNoOverflow:
    """Test that images don't overflow or cause horizontal scroll on mobile."""

    @pytest.mark.parametrize("viewport", MOBILE_VIEWPORTS, ids=lambda v: v["name"])
    def test_demo_images_fit_viewport(self, page: Page, base_url: str, viewport: dict):
        """Demo images should scale to fit viewport."""
        page.set_viewport_size({"width": viewport["width"], "height": viewport["height"]})
        page.goto(base_url)
        page.wait_for_load_state("networkidle")

        images = page.locator("main img")
        count = images.count()

        for i in range(count):
            img = images.nth(i)
            if not img.is_visible():
                continue

            box = img.bounding_box()
            if not box:
                continue

            # Image should fit within viewport (with some padding tolerance)
            max_width = viewport["width"] - 20  # Allow 10px padding each side
            assert box["width"] <= max_width, (
                f"Image {i} too wide on {viewport['name']}: {box['width']:.0f}px > {max_width}px"
            )

    def test_no_horizontal_scroll(self, mobile_page: Page):
        """Page should not have horizontal scroll on mobile."""
        # Check document width vs viewport
        doc_width = mobile_page.evaluate("document.documentElement.scrollWidth")
        viewport = mobile_page.viewport_size

        if not viewport:
            pytest.skip("Could not get viewport")

        assert doc_width <= viewport["width"] + 1, (
            f"Page causes horizontal scroll: "
            f"document width {doc_width}px > viewport {viewport['width']}px"
        )


class TestMobileHeroSection:
    """Test hero section renders correctly on mobile."""

    def test_hero_section_centered(self, mobile_page: Page):
        """Hero content should be centered on mobile."""
        hero = mobile_page.locator(".hero, .hero-section, section").first
        hero_box = hero.bounding_box()
        viewport = mobile_page.viewport_size

        if not hero_box or not viewport:
            pytest.skip("Could not get dimensions")

        # Hero should span most of the viewport width
        assert hero_box["width"] >= viewport["width"] * 0.9, (
            f"Hero too narrow: {hero_box['width']:.0f}px vs viewport {viewport['width']}px"
        )

    def test_hero_text_readable(self, mobile_page: Page):
        """Hero heading should be visible and reasonably sized."""
        heading = mobile_page.locator("h1").first

        if not heading.is_visible():
            pytest.skip("No h1 visible")

        font_size = heading.evaluate("el => parseFloat(getComputedStyle(el).fontSize)")

        # Font should be at least 24px on mobile for readability
        assert font_size >= 24, f"Hero heading too small: {font_size}px"


class TestMobileTouchTargets:
    """Test touch targets meet accessibility guidelines."""

    def test_buttons_touch_target_size(self, mobile_page: Page):
        """Buttons should be at least 44x44px for touch."""
        buttons = mobile_page.locator("a.btn, button")
        count = buttons.count()

        violations = []
        for i in range(count):
            btn = buttons.nth(i)
            if not btn.is_visible():
                continue

            box = btn.bounding_box()
            if not box:
                continue

            # WCAG recommends 44x44px minimum touch target
            if box["width"] < 44 or box["height"] < 44:
                text = btn.text_content() or btn.get_attribute("aria-label") or f"button_{i}"
                violations.append(
                    f"'{text.strip()[:20]}' is {box['width']:.0f}x{box['height']:.0f}px"
                )

        if violations:
            pytest.fail("Buttons too small for touch:\n" + "\n".join(violations))

    def test_nav_links_touch_target_size(self, mobile_page: Page):
        """Nav links should have adequate touch targets."""
        links = mobile_page.locator(".nav-links a")
        count = links.count()

        for i in range(count):
            link = links.nth(i)
            if not link.is_visible():
                continue

            box = link.bounding_box()
            if not box:
                continue

            # Nav links should be at least 32px high for touch
            assert box["height"] >= 32, (
                f"Nav link '{link.text_content()[:15]}' too short: {box['height']:.0f}px"
            )


def _boxes_overlap(box_a: dict, box_b: dict) -> bool:
    """Check if two bounding boxes overlap."""
    return not (
        box_a["x"] + box_a["width"] <= box_b["x"]
        or box_b["x"] + box_b["width"] <= box_a["x"]
        or box_a["y"] + box_a["height"] <= box_b["y"]
        or box_b["y"] + box_b["height"] <= box_a["y"]
    )
