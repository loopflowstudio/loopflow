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

## Phase 1 ordered set

Use the ordered backlog below as the canonical Phase 1 list:

1. `20260131-02-history-and-recency.md`
2. `20260131-03-waiting-state-actionable.md`
3. `20260131-04-running-state-progress-and-connect.md`
4. `20260131-05-empty-state-creates-and-teaches.md`
5. `20260131-06-quick-experiment-path.md`

Attention summary and grouping (formerly item 01) is complete—shipped in the current branch.

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
