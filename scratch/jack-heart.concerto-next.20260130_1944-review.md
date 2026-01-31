# Design Review: Concerto Phase 1 Backlog Generation

Branch: `jack-heart.concerto-next.20260130_1944`

## What was implemented

Created a systematic process for generating Phase 1 polish work from persona-based UX reviews:

1. **Screenshot generation pipeline** — `scripts/generate_screenshots.py` produces snapshots and UI test screenshots of Concerto states
2. **Persona directions** — `.lf/directions/` contains conductor, improviser, listener, ceo, and product-designer perspectives
3. **UX review step** — `.lf/steps/ux-review.md` runs persona-focused reviews against screenshots
4. **Phase 1 backlog items** — Six ordered items in `roadmap/concerto/` capturing polish priorities

## Key choices

**Screenshots as artifacts**: Static images are reviewable by both humans and LLMs without requiring Screen Recording permissions or live app access. Rejected live testing (not repeatable by agents) and heuristic audits (loses persona insights).

**Ordered backlog over flat list**: Phase 1 items are numbered (`20260131-01-*` through `20260131-06-*`) to establish clear priority. README documents the canonical order.

**Persona synthesis over single viewpoint**: Items include `sources: [conductor, listener, ...]` frontmatter showing which personas identified the issue. This catches overlap and validates priority.

**Codex sandbox refactor**: Changed from deprecated `--dangerously-bypass-approvals-and-sandbox` to explicit `--sandbox danger-full-access`. The new approach is consistent with Codex CLI's current API and separates sandbox mode from approval policy.

## How it fits together

```
.lf/directions/       → personas (conductor, improviser, listener, ceo, product-designer)
.lf/steps/ux-review.md → runs review with persona lens
scripts/generate_screenshots.py → produces artifacts
docs/screenshots/      → generated screenshots
roadmap/concerto/      → Phase 1 items with frontmatter
```

The loop: generate screenshots → run reviews per persona → synthesize into backlog items → human reviews before promoting.

## Risks and bottlenecks

- **Screenshot generation requires macOS build**: Cannot run headless on CI without display/permission issues
- **UX review depends on screenshots existing**: If screenshot generation fails, the review step fails too
- **Manual promotion**: Items go to `roadmap/` only after human review—this is intentional but adds friction
- **Minor schema inconsistency**: README says `persona: conductor | improviser | listener` but items use `persona: concerto`. The `sources:` field captures contributing personas, so this may be intentional (product area vs. user type)

## What's not included

- **Improvise mode screenshots**: Requires beta flag work; deferred
- **Automated CI integration**: Screenshot generation is manual
- **Screenshot diff/regression testing**: Not in scope for Phase 1
- **Swift code implementation of Phase 1 items**: This branch sets up the backlog; implementation is future work

## Test status

- Python tests: 673 passed (fixed 3 broken tests in `test_launcher.py`)
- Rust: cargo fmt and clippy pass
- No new tests added (documentation-focused branch)

## Files changed

| Area | Changes |
|------|---------|
| `roadmap/concerto/` | README updated, 6 new Phase 1 items, `phase-1-backlog-generation.md` removed |
| `.lf/directions/` | Added ceo.md, listener.md, product-designer.md; removed returner.md |
| `scripts/` | `generate_screenshots.py` enhanced, `screenshots.yaml` updated |
| `src/loopflow/lf/launcher.py` | Codex yolo mode sandbox refactor |
| `tests/test_launcher.py` | Fixed contradictory assertions in yolo tests |
| `swift/` | Views moved from Improvise/ to Views/, CaptureService replaced with SnapshotService |
