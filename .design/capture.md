# lf capture: Screenshot Capture CLI

**Status**: Implemented

Add a CLI command to capture screenshots of application windows, saving them to `.design/screenshots/` for use as context in loopflow tasks.

## Why This Extends the Branch

The `spark-tune` branch added image context support: `GatherResult` separates images from text, `format_image_references()` tells agents where images are, and Codex receives images via `-i` flag.

What's missing: there's no easy way to **capture** screenshots. The spark-tune design doc mentions `screencapture-maestro` as a planned tool. This expansion implements that capability as a generic `lf capture` command.

## Usage

```bash
# Capture a window by name (fuzzy match)
lf capture Maestro                  # → .design/screenshots/capture-<timestamp>.png

# Capture with a custom name
lf capture Maestro --name main-view # → .design/screenshots/main-view.png

# Capture and open for verification
lf capture Maestro --open           # opens screenshot in Preview after capture

# List windows (help choosing)
lf capture --list

# Then use in a task
lf ux-audit -x .design/screenshots/
```

## Implementation

### Data Flow

```
lf capture <window-name>
    ↓
find_window(name) → WindowInfo (id, bounds)
    ↓
capture_window() tries:
  1. screencapture -l <window_id>
  2. screencapture -R <bounds> (fallback)
    ↓
.design/screenshots/<name>.png
```

### Key Functions

```python
# src/loopflow/capture.py

@dataclass
class WindowInfo:
    window_id: int
    app_name: str
    title: str
    bounds: dict

def list_windows() -> list[WindowInfo]:
    """List visible windows using Quartz CGWindowListCopyWindowInfo."""

def find_window(name: str) -> WindowInfo | None:
    """Fuzzy match: exact > prefix > substring, case-insensitive."""

def capture_window(window: WindowInfo, output_path: Path) -> Path:
    """Capture via -l flag, fallback to -R region capture."""
```

### Window Discovery

Uses `pyobjc-framework-Quartz` to query `CGWindowListCopyWindowInfo`. Filters to layer 0 windows (normal app windows) with reasonable dimensions (>100x100).

## Output Location

Screenshots go to `.design/screenshots/` with automatic directory creation:

```
.design/
  screenshots/
    maestro-20250115-143022.png    # timestamped
    main-view.png                   # named
```

## Integration with spark-tune Pipeline

```bash
# Capture current state
lf capture Maestro --name before

# Run UX audit with screenshots
lf ux-audit -x .design/screenshots/

# After changes, capture again
lf capture Maestro --name after

# Compare
lf ux-design -x .design/screenshots/
```

## Constraints

- **macOS only**: Uses `screencapture` CLI and Quartz APIs
- **Screen Recording permission required**: User must grant permission in System Settings
- **PNG format**: Best quality for UI screenshots
- **Single window**: Captures one window at a time (no multi-window compositions)

## Files Changed

- `src/loopflow/capture.py` (new): `WindowInfo`, `list_windows()`, `find_window()`, `capture_window()`, `ScreenCaptureError`
- `src/loopflow/cli/__init__.py`: Added `capture` command
- `tests/test_capture.py` (new): Tests for window matching logic
- `pyproject.toml`: Added `pyobjc-framework-Quartz` dependency

## Verification

```bash
# List windows
lf capture --list

# Capture by name
lf capture Maestro --name main-view

# Use in UX pipeline
lf ux-audit -x .design/screenshots/
```
