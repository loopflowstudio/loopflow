# Image Context Support

**Status**: Implemented

Enable images (screenshots, diagrams) as first-class context in loopflow tasks. This directly enables spark-tune's UX iteration pipeline.

## What Changed

### New Data Structures

```python
# files.py
@dataclass
class GatherResult:
    text_files: list[tuple[Path, str]]
    image_files: list[Path]

_IMAGE_EXTENSIONS = {".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".tiff", ".ico"}

def is_image(path: Path) -> bool:
    return path.suffix.lower() in _IMAGE_EXTENSIONS
```

### Modified Functions

- `gather_files()` now returns `GatherResult` instead of `list[tuple[Path, str]]`
- `format_image_references()` added to create `<lf:images>` section in prompts
- `PromptComponents.image_files` new field tracks images for the session
- `build_codex_command()` accepts `images` parameter and passes via `-i` flag
- `build_model_command()` and `build_model_interactive_command()` pass images to backends

### Backend Support

| Backend | How Images Are Passed |
|---------|----------------------|
| Claude Code | Referenced in prompt; agent uses Read tool to view |
| Codex | Passed via `-i <file>` CLI flag |
| Gemini | Referenced in prompt; agent reads from workspace |

## Usage

```bash
# Include screenshots in UX audit
lf ux-audit -x .design/screenshots/

# Single image context
lf : "describe this UI" -x screenshot.png

# Glob pattern for all images
lf review -x "**/*.png"
```

The prompt includes:

```xml
<lf:images>
The following images are available. Use your Read tool to view them:
- .design/screenshots/main.png
- .design/screenshots/sidebar.png
</lf:images>
```

## Files Changed

- `src/loopflow/files.py`: Added `GatherResult`, `is_image()`, `format_image_references()`
- `src/loopflow/context.py`: Added `image_files` to `PromptComponents`, updated `gather_prompt_components()` and `format_prompt()`
- `src/loopflow/launcher.py`: Added `images` parameter to all command builders
- `src/loopflow/cli/run.py`: Wired `components.image_files` through to command builders
- `src/loopflow/lfd/runner.py`: Wired images through for agent execution
- `tests/test_files.py`: Updated all tests for new return type, added image-specific tests
