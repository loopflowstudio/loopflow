# Concerto Backlog

Pickable work items for Concerto, the loopflow macOS app.

## Phases

| Phase | Focus | Status |
|-------|-------|--------|
| 1 | Polish (macOS local) | In progress |
| 2 | Remote access foundation | Planning |
| 3 | Mobile (iOS/iPad) | Future |

## Generating Phase 1 items

Phase 1 items come from persona+screenshot review:

```bash
lf ux-review --direction conductor --area docs/concerto-main.png
lf ux-review --direction improviser --area docs/concerto-main.png
lf ux-review --direction returner --area docs/concerto-main.png
```

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

Design docs in `reports/concerto/`:
- `00-overview.md` — Conduct & Improvise modes
- `03-conduct-ux.md` — Dashboard, connect, land
- `04-improvise-ux.md` — Area picker, step runner
- `09-phasing.md` — Phase definitions
