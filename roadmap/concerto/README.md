# Concerto Backlog

Pickable work items for Concerto, the loopflow macOS app.

## Phases

| Phase | Focus | Status |
|-------|-------|--------|
| 1 | Polish (macOS local) | In progress |
| 2 | Remote access foundation | Planning |
| 3 | Mobile (iOS/iPad) | Future |

<<<<<<< HEAD
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

=======
## Generating Phase 1 items

Phase 1 items come from persona+screenshot review:

```bash
lf ux-review --direction conductor --screenshot docs/concerto-main.png
lf ux-review --direction improviser --screenshot docs/concerto-improvise.png
lf ux-review --direction returner --screenshot docs/concerto-main.png
```

>>>>>>> c75991bee (concerto: restructure roadmap with persona directions and phases 1-3)
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

<<<<<<< HEAD
Design docs: `reports/concerto/`
Personas: `.lf/directions/{conductor,improviser,returner}.md`
=======
Design docs in `reports/concerto/`:
- `00-overview.md` — Conduct & Improvise modes
- `03-conduct-ux.md` — Dashboard, connect, land
- `04-improvise-ux.md` — Area picker, step runner
- `09-phasing.md` — Phase definitions
>>>>>>> c75991bee (concerto: restructure roadmap with persona directions and phases 1-3)
