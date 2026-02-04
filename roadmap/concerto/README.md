# Concerto Backlog

Pickable work items for Concerto, the loopflow macOS app.

## Phases

| Phase | Focus | Status |
|-------|-------|--------|
| 1 | Polish (macOS local) | Complete |
| 2 | Remote access foundation | In progress |
| 3 | Mobile (iOS/iPad) | Future |

## Screenshot pipeline

```bash
# Generate snapshots (fast, no permissions)
uv run python scripts/generate_screenshots.py --snapshot-only --repo-path ~/src/loopflow-demos --no-clone

# Generate UI test screenshots (flow-focused, slower)
uv run python scripts/generate_screenshots.py --ui-test-only --repo-path ~/src/loopflow-demos --no-clone

# Review with each persona
uv run lf ux-review --direction conductor --area docs/screenshots/
uv run lf ux-review --direction improviser --area docs/screenshots/
uv run lf ux-review --direction listener --area docs/screenshots/
```

Screenshots in `docs/screenshots/`:
- `concerto-main.png` — sidebar with grouped waves
- `concerto-wave-running.png` — running wave detail
- `concerto-wave-waiting.png` — waiting wave detail

Swift screenshots are generated via in-app snapshots (no Screen Recording permission).

## Item format

```yaml
---
status: todo | in-progress | done
phase: 1 | 2 | 3
persona: conductor | improviser | listener  # optional
order: 1  # optional
screenshot: path/to/evidence.png  # optional
---
```

## Phase 1 items

```bash
uv run python scripts/generate_screenshots.py --snapshot-only --repo-path ~/src/loopflow-demos --no-clone
uv run python scripts/generate_screenshots.py --ui-test-only --repo-path ~/src/loopflow-demos --no-clone
uv run lf ux-review --direction conductor --area docs/screenshots/
uv run lf ux-review --direction improviser --area docs/screenshots/
uv run lf ux-review --direction listener --area docs/screenshots/
```

Run the pipeline and review the output in `roadmap/concerto/` before promoting items.

## Phase 1 summary

Shipped polish for local macOS workflows:
- Attention summary and grouping in the sidebar
- History and recency for recent activity
- Waiting state actions (connect + PR badges)
- Running state progress and elapsed time
- Empty state that teaches and invites action
- Quick experiment flow without waves

## Phase 2 focus

Remote access foundation:
- WaveService protocol abstraction (transport-agnostic service layer)
- lfd registration + auth groundwork (see Phase 2 items)
- gRPC terminal streaming groundwork

## If screenshots are blocked

```bash
# Run the UX experiments manually and log friction
cat reports/concerto/07-ux-experiments.md
```

Write Phase 1 backlog items directly from observed friction in Concerto.
Add `screenshot:` only when you have one.

## Reference

Design docs: `reports/concerto/`
Personas: `.lf/directions/{conductor,improviser,listener}.md`
