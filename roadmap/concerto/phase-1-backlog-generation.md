---
status: todo
phase: 1
---

# Phase 1 backlog generation

Create polish items from the screenshot review pipeline.

## Current

No Phase 1 items exist yet because the screenshot + persona review loop has not run.

## Build

```bash
uv run python scripts/generate_screenshots.py
uv run lf ux-review --direction conductor --area docs/screenshots/
uv run lf ux-review --direction improviser --area docs/screenshots/
uv run lf ux-review --direction returner --area docs/screenshots/
```

Review the generated items under `roadmap/concerto/` and keep only actionable polish work.

## Done when

- `docs/screenshots/` contains the three Concerto screenshots
- `roadmap/concerto/` includes Phase 1 items with `phase: 1`
- Each Phase 1 item includes a `screenshot:` field when applicable
