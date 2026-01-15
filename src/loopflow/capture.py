"""Screenshot capture for LLM context."""

import subprocess
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

from Quartz import (
    CGWindowListCopyWindowInfo,
    kCGWindowListOptionOnScreenOnly,
    kCGNullWindowID,
    kCGWindowListExcludeDesktopElements,
)


@dataclass
class WindowInfo:
    """Information about a visible window."""
    window_id: int
    app_name: str
    title: str
    bounds: dict


def list_windows() -> list[WindowInfo]:
    """List visible windows with titles and IDs."""
    options = kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements
    windows = CGWindowListCopyWindowInfo(options, kCGNullWindowID)

    results = []
    for win in windows:
        # Skip windows without names or with empty titles
        app_name = win.get("kCGWindowOwnerName", "")
        title = win.get("kCGWindowName", "")
        window_id = win.get("kCGWindowNumber", 0)
        bounds = win.get("kCGWindowBounds", {})

        # Skip menu bar, dock, and other system elements
        layer = win.get("kCGWindowLayer", 0)
        if layer != 0:
            continue

        # Skip tiny windows (likely invisible or UI elements)
        width = bounds.get("Width", 0)
        height = bounds.get("Height", 0)
        if width < 100 or height < 100:
            continue

        if app_name and window_id:
            results.append(WindowInfo(
                window_id=window_id,
                app_name=app_name,
                title=title or "",
                bounds=bounds,
            ))

    return results


def find_window(name: str) -> WindowInfo | None:
    """Find window by fuzzy name match.

    Matches against app name or window title, case-insensitive.
    Prefers exact matches, then prefix matches, then substring matches.
    """
    windows = list_windows()
    name_lower = name.lower()

    # Score each window
    scored: list[tuple[int, WindowInfo]] = []
    for win in windows:
        app_lower = win.app_name.lower()
        title_lower = win.title.lower()

        # Exact match on app name or title
        if app_lower == name_lower or title_lower == name_lower:
            scored.append((0, win))
        # Prefix match
        elif app_lower.startswith(name_lower) or title_lower.startswith(name_lower):
            scored.append((1, win))
        # Substring match
        elif name_lower in app_lower or name_lower in title_lower:
            scored.append((2, win))

    if not scored:
        return None

    # Return best match
    scored.sort(key=lambda x: x[0])
    return scored[0][1]


class ScreenCaptureError(Exception):
    """Failed to capture screenshot."""
    pass


def capture_window(window: WindowInfo, output_path: Path) -> Path:
    """Capture screenshot of a window.

    First tries window ID capture (-l), falls back to region capture (-R)
    using window bounds if that fails.
    """
    output_path.parent.mkdir(parents=True, exist_ok=True)

    # Try window ID capture first (works on some macOS versions/apps)
    result = subprocess.run(
        ["screencapture", "-l", str(window.window_id), "-x", str(output_path)],
        capture_output=True,
    )

    if result.returncode == 0 and output_path.exists():
        return output_path

    # Fall back to region capture using window bounds
    bounds = window.bounds
    x = int(bounds.get("X", 0))
    y = int(bounds.get("Y", 0))
    w = int(bounds.get("Width", 0))
    h = int(bounds.get("Height", 0))

    if w == 0 or h == 0:
        raise ScreenCaptureError(f"Window has invalid bounds: {bounds}")

    result = subprocess.run(
        ["screencapture", "-R", f"{x},{y},{w},{h}", "-x", str(output_path)],
        capture_output=True,
    )

    if result.returncode != 0 or not output_path.exists():
        stderr = result.stderr.decode().strip()
        if "could not create image" in stderr:
            raise ScreenCaptureError(
                "Screen Recording permission required.\n"
                "Grant permission: System Settings → Privacy & Security → Screen Recording"
            )
        raise ScreenCaptureError(f"screencapture failed: {stderr}")

    return output_path


def generate_screenshot_path(name: str | None, repo_root: Path) -> Path:
    """Generate output path for screenshot."""
    screenshots_dir = repo_root / ".design" / "screenshots"

    if name:
        # Use provided name
        filename = f"{name}.png"
    else:
        # Generate timestamped name
        timestamp = datetime.now().strftime("%Y%m%d-%H%M%S")
        filename = f"capture-{timestamp}.png"

    return screenshots_dir / filename
