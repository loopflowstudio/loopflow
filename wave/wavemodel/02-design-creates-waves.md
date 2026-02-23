# Design Creates Waves + Split-Wave

`lf design` becomes the wave creation entry point. `split-wave` becomes the organic scaling mechanism.

## Design creates waves

Today:
```
lf design → scratch/wave-proposal.md → lf add-to-wave → wave/<name>/
```

New:
```
lf design → wave/<name>/ (README + YAML + items) + scratch/<branch>.md
```

When the user chooses "wave plan" in Phase 4 (Fork), design creates:
1. `wave/<name>/README.md` — four sections populated from conversation (Vision, Goals, Risks, Metrics)
2. `wave/<name>/<name>.yaml` — flow, area, direction, stimulus (asked or inferred)
3. `wave/<name>/01-*.md` etc. — the roadmap as numbered files
4. `scratch/<branch>.md` — design doc for the first item (still needed for `lf implement`)

The conversation naturally populates the content:
- Vision → Phase 1 (Dream) — "what are you trying to build?"
- Goals → Phase 2 (Detail) — concrete objectives emerge during detailing
- Risks → Phase 2 (Detail) — edge cases, unknowns, failure modes
- Metrics → Phase 2 (Detail) — "how do we know it works?"
- Roadmap (`01-*.md`, `02-*.md`) → Phase 4 (Fork) — the staged breakdown

YAML configuration can be inferred or asked at the end:
- flow: usually `ship-wave` for new waves
- area: inferred from what files/directories the conversation referenced
- direction: inferred from the persona/perspective discussed
- stimulus: ask or default to none

`add-to-wave` remains useful for promoting analysis artifacts from plan flows. But design-to-wave is the primary creation path.

## New step: split-wave

```yaml
---
interactive: true
requires: wave/<wave>/README.md
produces: wave/<child>/ directories
---
```

When a wave's roadmap grows beyond what a single wave should own:

1. Read the wave README (Vision, Goals, Risks, Metrics) and roadmap (`##-*.md`)
2. Identify natural boundaries (thematic clusters, dependency groups, independent workstreams)
3. Propose child waves to the user — each with its own scoped Vision
4. For each child wave, create:
   - `wave/<child>/README.md` — inherits parent context, gets its own scoped sections
   - `wave/<child>/<child>.yaml` — configuration (may differ from parent)
   - Move relevant phase .md files from parent to child
5. Update parent wave:
   - Roadmap items reference children instead of listing work directly
   - Parent becomes a coordination wave

## Files touched

- `rust/loopflow/src/engine/builtins/steps/interactive/design.md` — significant rewrite of Phase 4 output
- New: `rust/loopflow/src/engine/builtins/steps/interactive/split-wave.md`
