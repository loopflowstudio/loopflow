"""Mobile visual tests for the Loopflow website.

Tests that mobile layout renders correctly without overlaps or overflow.
Run with: cd website && uv run pytest tests/test_mobile.py -v
"""

import pytest
from playwright.sync_api import Page


MOBILE_VIEWPORT = {"width": 375, "height": 812}  # iPhone X


class TestMobileNavigation:
    """Test navigation doesn't overlap on mobile viewports."""

    def test_nav_items_no_overlap(self, page: Page, server: str):
        """Nav items should not overlap each other on mobile."""
        page.set_viewport_size(MOBILE_VIEWPORT)
        page.goto(server)
        page.wait_for_load_state("networkidle")

        nav_links = page.locator(".nav-links > li:visible")
        count = nav_links.count()

        if count < 2:
            pytest.skip("Not enough nav items to test overlap")

        # Get bounding boxes of all visible nav items
        boxes = []
        for i in range(count):
            item = nav_links.nth(i)
            if item.is_visible():
                box = item.bounding_box()
                if box:
                    boxes.append((i, box))

        # Check no two items overlap horizontally
        for i, (idx_a, box_a) in enumerate(boxes):
            for idx_b, box_b in boxes[i + 1:]:
                # Check if boxes overlap
                overlap_x = (
                    box_a["x"] < box_b["x"] + box_b["width"] and
                    box_a["x"] + box_a["width"] > box_b["x"]
                )
                overlap_y = (
                    box_a["y"] < box_b["y"] + box_b["height"] and
                    box_a["y"] + box_a["height"] > box_b["y"]
                )

                if overlap_x and overlap_y:
                    pytest.fail(
                        f"Nav items {idx_a} and {idx_b} overlap at mobile viewport. "
                        f"Item {idx_a}: x={box_a['x']:.0f}, w={box_a['width']:.0f}. "
                        f"Item {idx_b}: x={box_b['x']:.0f}, w={box_b['width']:.0f}."
                    )

    def test_nav_logo_no_overlap_with_links(self, page: Page, server: str):
        """Nav logo should not overlap with nav links."""
        page.set_viewport_size(MOBILE_VIEWPORT)
        page.goto(server)
        page.wait_for_load_state("networkidle")

        logo = page.locator(".nav-logo")
        links = page.locator(".nav-links")

        logo_box = logo.bounding_box()
        links_box = links.bounding_box()

        if not logo_box or not links_box:
            pytest.skip("Could not get bounding boxes")

        # Check horizontal overlap
        logo_right = logo_box["x"] + logo_box["width"]
        links_left = links_box["x"]

        if logo_right > links_left:
            pytest.fail(
                f"Nav logo overlaps with nav links. "
                f"Logo ends at x={logo_right:.0f}, links start at x={links_left:.0f}."
            )

    def test_nav_title_no_overlap_with_links(self, page: Page, server: str):
        """Centered nav title should not overlap with nav links."""
        page.set_viewport_size(MOBILE_VIEWPORT)
        page.goto(server)
        page.wait_for_load_state("networkidle")

        nav_title = page.locator(".nav-title")
        links = page.locator(".nav-links")

        # Skip if nav-title doesn't exist
        if nav_title.count() == 0:
            pytest.skip("No .nav-title element")

        title_box = nav_title.bounding_box()
        links_box = links.bounding_box()

        if not title_box or not links_box:
            pytest.skip("Could not get bounding boxes")

        # Check for overlap
        title_right = title_box["x"] + title_box["width"]
        links_left = links_box["x"]
        title_left = title_box["x"]
        links_right = links_box["x"] + links_box["width"]

        # Check horizontal overlap (nav title and links are on same row)
        horizontal_overlap = title_left < links_right and title_right > links_left
        # Check vertical overlap
        vertical_overlap = (
            title_box["y"] < links_box["y"] + links_box["height"] and
            title_box["y"] + title_box["height"] > links_box["y"]
        )

        if horizontal_overlap and vertical_overlap:
            pytest.fail(
                f"Nav title overlaps with nav links. "
                f"Title at x={title_box['x']:.0f}-{title_right:.0f}, "
                f"links at x={links_box['x']:.0f}-{links_right:.0f}."
            )

    def test_nav_fits_in_viewport(self, page: Page, server: str):
        """Navigation should fit within viewport width."""
        page.set_viewport_size(MOBILE_VIEWPORT)
        page.goto(server)
        page.wait_for_load_state("networkidle")

        nav_container = page.locator("nav .container")
        box = nav_container.bounding_box()

        if not box:
            pytest.skip("Could not get nav container bounding box")

        # Nav should not exceed viewport width (accounting for padding)
        if box["x"] + box["width"] > MOBILE_VIEWPORT["width"]:
            pytest.fail(
                f"Nav container exceeds viewport. "
                f"Container width: {box['width']:.0f}px, viewport: {MOBILE_VIEWPORT['width']}px."
            )


class TestMobileImages:
    """Test images fit properly on mobile viewports."""

    def test_demo_images_fit_viewport(self, page: Page, server: str):
        """Demo images should scale to fit mobile viewport."""
        page.set_viewport_size(MOBILE_VIEWPORT)
        page.goto(server)
        page.wait_for_load_state("networkidle")

        # Scroll to load any lazy images
        page.evaluate("window.scrollTo(0, document.body.scrollHeight)")
        page.wait_for_timeout(500)
        page.evaluate("window.scrollTo(0, 0)")

        images = page.locator(".demo-gif")
        count = images.count()

        for i in range(count):
            img = images.nth(i)
            if not img.is_visible():
                continue

            box = img.bounding_box()
            if not box:
                continue

            # Image should not exceed viewport width (with small margin for padding)
            max_width = MOBILE_VIEWPORT["width"] - 24  # Account for container padding
            if box["width"] > max_width + 1:  # +1 for rounding
                pytest.fail(
                    f"Demo image {i} exceeds viewport width. "
                    f"Image width: {box['width']:.0f}px, max allowed: {max_width}px."
                )

    def test_images_no_horizontal_scroll(self, page: Page, server: str):
        """Page should not have horizontal scroll due to images."""
        page.set_viewport_size(MOBILE_VIEWPORT)
        page.goto(server)
        page.wait_for_load_state("networkidle")

        # Scroll through page
        page.evaluate("window.scrollTo(0, document.body.scrollHeight)")
        page.wait_for_timeout(500)

        # Check for horizontal overflow
        has_overflow = page.evaluate("""
            document.documentElement.scrollWidth > document.documentElement.clientWidth
        """)

        if has_overflow:
            scroll_width = page.evaluate("document.documentElement.scrollWidth")
            client_width = page.evaluate("document.documentElement.clientWidth")
            pytest.fail(
                f"Page has horizontal overflow on mobile. "
                f"Scroll width: {scroll_width}px, viewport: {client_width}px."
            )


class TestMobileLayout:
    """Test overall mobile layout renders correctly."""

    def test_hero_section_centered(self, page: Page, server: str):
        """Hero section should be centered on mobile."""
        page.set_viewport_size(MOBILE_VIEWPORT)
        page.goto(server)
        page.wait_for_load_state("networkidle")

        hero = page.locator(".hero")

        # On mobile, hero should have text-align: center
        text_align = hero.evaluate("el => getComputedStyle(el).textAlign")

        assert text_align == "center", (
            f"Hero section should be centered on mobile, got text-align: {text_align}"
        )

    def test_buttons_touch_target_size(self, page: Page, server: str):
        """Buttons should meet minimum touch target size (44x44px)."""
        page.set_viewport_size(MOBILE_VIEWPORT)
        page.goto(server)
        page.wait_for_load_state("networkidle")

        buttons = page.locator("a.btn, button")
        count = buttons.count()

        min_touch_size = 44

        for i in range(min(count, 10)):
            button = buttons.nth(i)
            if not button.is_visible():
                continue

            box = button.bounding_box()
            if not box:
                continue

            if box["height"] < min_touch_size:
                text = button.text_content() or "unknown"
                pytest.fail(
                    f"Button '{text[:20]}' has height {box['height']:.0f}px, "
                    f"minimum touch target is {min_touch_size}px."
                )
