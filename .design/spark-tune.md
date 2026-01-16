# spark-tune: LLM-Assisted UX Iteration for Maestro

**Status**: Phase 1 Complete

Infrastructure for LLMs to see, understand, and iterate on the Maestro app UX—including screenshots, simulated user research, and minimal automated UI testing.

## What's Done

### Screenshot Capture (`lf capture`)

A CLI command captures application windows and saves them as context for LLM tasks.

```bash
lf capture Maestro                  # capture window → .design/screenshots/capture-<timestamp>.png
lf capture Maestro --name main-view # custom filename → .design/screenshots/main-view.png
lf capture Maestro --open           # capture and open in Preview
lf capture --list                   # list available windows
```

See `.design/capture.md` for implementation details.

### Image Context Support

Images are now first-class context in loopflow tasks. When you include screenshots via `-x`, they're tracked separately and passed to agents:

```bash
lf review -x .design/screenshots/   # include all screenshots as context
```

The prompt includes an `<lf:images>` section telling agents where to find the images. Codex receives images via `-i` flag; Claude and Gemini read them from the filesystem.

See `.design/image-context.md` for implementation details.

## What's Left

### UX Analysis Pipeline

The vision is a pipeline of prompts for UX iteration:

```
.lf/
  ux-audit.lf       # Analyze screenshots + vision → identify gaps
  ux-research.lf    # Simulate users, run through tasks
  ux-design.lf      # Propose UI improvements based on findings
  ux-build.lf       # Implement proposed changes
```

These prompt files haven't been created yet.

### State Export (Maestro Swift side)

Adding a Debug menu to Maestro to export app state as JSON for LLM analysis. Not yet implemented.

### UI Testing

Minimal smoke tests that catch obvious regressions. Not yet implemented.

## Usage

With the current implementation, UX iteration workflow is:

```bash
# 1. Capture current state
lf capture Maestro --name before

# 2. Run UX audit with screenshots
lf review -x .design/screenshots/ "Analyze this UI for usability issues"

# 3. Make changes...

# 4. Capture after
lf capture Maestro --name after
```

## Files Changed

- `src/loopflow/capture.py`: Window discovery and screenshot capture
- `src/loopflow/files.py`: `GatherResult` dataclass, `is_image()`, `format_image_references()`
- `src/loopflow/context.py`: `image_files` field in `PromptComponents`
- `src/loopflow/launcher.py`: Image passing to Codex via `-i` flag
- `src/loopflow/cli/__init__.py`: `lf capture` command
- `tests/test_capture.py`: Window matching tests
- `tests/test_files.py`: Image handling tests
