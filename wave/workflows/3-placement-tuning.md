# Placement Tuning

**Finish line:** each built-in flow lives in the category it actually earns through use, not the one it landed in during the reorg. No lingering "maybe this should move" comments.

The reorg collapsed 13 partially-overlapping labels into three meaningful ones (build/govern/ops). That's the right shape. Placement of individual flows within those categories is a different question — and one the catalog makes visible but doesn't answer.

Two adjustments already surfaced during the reorg itself:

- `s1-build` → `govern/flow/` (no kickoff/design ceremony; it's the autonomous "just build" path that `govern-operations` triggers).
- `sync` → `ops/flow/` (rebase → integrate-upstream is git hygiene, not build work).

Expect more. This item stays open as a continuing-concern tracker.

## Process

Not a single refactor. Each move is its own small change:

1. A use-site surfaces an awkwardness ("this flow lives in build/ but it's always triggered by a cron, never by a human").
2. File a note here with the concrete case.
3. Move the flow, update the tree, update `flow_tests.rs`.
4. Leave a one-line rationale in the commit message.

## Candidates to watch

- `build-or-silent` — build or govern? It's `ingest → maybe(build)`. Currently build. But the cron-driven use feels govern-adjacent. Decide once it's been cron-triggered in anger a few times.
- `queue` — `gate → update-wave → deploy`. Looks like ops plumbing with a build step glued on.
- `incident` — currently build. Triggered by CI failure, which is an autonomous signal. Could be govern.

## Out of scope

- Subdividing `ops/` further (git/release/wave subcategories). 12 items flat is readable; splitting without a forcing function adds navigation overhead for no clarity win.
- Renaming flows. Placement moves, names stay.
- Adding new flows. This is reorg, not feature work.

## Done condition

This item is done when:

- No flow has been moved in the last ~two months of active use.
- Review of each category asks "would I put this here today?" and the answer is yes for all entries.

Until then, it stays open as a sensor.
