# Concerto Backlog

Pickable work items for Concerto, the loopflow macOS app.

## Phases

| Phase | Focus | Status |
|-------|-------|--------|
| 1 | Polish (macOS local) | In progress |
| 2 | Remote access foundation | Planning |
| 3 | Mobile (iOS/iPad) | Future |

## Screenshot pipeline

```bash
# Generate all screenshots
uv run python scripts/generate_screenshots.py

# Review with each persona
uv run lf ux-review --direction conductor --area docs/screenshots/
uv run lf ux-review --direction improviser --area docs/screenshots/
uv run lf ux-review --direction returner --area docs/screenshots/
```

Screenshots in `docs/screenshots/`:
- `concerto-main.png` — sidebar with grouped waves
- `concerto-wave-running.png` — running wave detail
- `concerto-wave-waiting.png` — waiting wave detail

## Item format

```yaml
---
status: todo | in-progress | done
phase: 1 | 2 | 3
persona: conductor | improviser | returner  # optional
screenshot: path/to/evidence.png  # optional
---
```

## Phase 1 items

```bash
uv run python scripts/generate_screenshots.py
uv run lf ux-review --direction conductor --area docs/screenshots/
uv run lf ux-review --direction improviser --area docs/screenshots/
uv run lf ux-review --direction returner --area docs/screenshots/
```

Run the pipeline and review the output in `roadmap/concerto/` before promoting items.

## Reference

Design docs: `reports/concerto/`
Personas: `.lf/directions/{conductor,improviser,returner}.md`
