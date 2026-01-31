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
python scripts/generate_screenshots.py

# Review with each persona
lf ux-review --direction conductor --area docs/screenshots/
lf ux-review --direction improviser --area docs/screenshots/
lf ux-review --direction returner --area docs/screenshots/
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

## Reference

Design docs: `reports/concerto/`
Personas: `.lf/directions/{conductor,improviser,returner}.md`
