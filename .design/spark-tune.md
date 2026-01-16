# spark-tune: LLM-Assisted UX Iteration for Maestro

**Status**: In Progress

Infrastructure for LLMs to see, understand, and iterate on the Maestro app UX.

## What's Done

### Image Context Support (loopflow)

Images are first-class context in loopflow tasks. Include screenshots via `-x`:

```bash
lf review -x .design/screenshots/   # include all screenshots as context
```

The prompt includes an `<lf:images>` section telling agents where to find the images. Codex receives images via `-i` flag; Claude and Gemini read them from the filesystem.

See `.design/image-context.md` for implementation details.

### Screenshot Capture (Maestro)

Keyboard shortcut `Cmd+Shift+S` captures Maestro's window to `.design/screenshots/` for LLM review. Shows in Finder after capture.

See `.design/maestro-capture.md` for spec.

### UX Review Task (loopflow)

Single prompt for UX iteration: `.lf/ux-review.lf`

Analyzes screenshots for visual issues and friction, then proposes concrete fixes. Outputs to `.design/ux-review.md`.

## What's Next

### State/Accessibility Export (Maestro)

Export structured data for LLM analysis:
- Accessibility tree as JSON
- App state as JSON

Structured data is more useful than screenshots for debugging. Screenshots are for visual design review.

## Usage

```bash
# In Maestro: Cmd+Shift+S to capture (multiple states)

# In terminal:
lf ux-review     # Analyze and propose fixes

# Then implement:
lf implement .design/ux-review.md
```

## Files Changed (this branch)

- `src/loopflow/files.py`: `GatherResult` dataclass, `is_image()`, `format_image_references()`
- `src/loopflow/context.py`: `image_files` field in `PromptComponents`
- `src/loopflow/launcher.py`: Image passing to Codex via `-i` flag
- `tests/test_files.py`: Image handling tests
