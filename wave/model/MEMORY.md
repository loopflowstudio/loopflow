# Wave memory — model

## Patterns

- Wave items live in `wave/model/<n>-<name>.md` with `asana_id` frontmatter (no
  local status/priority fields yet — status lives in Asana).
- `wave/model/model.yaml` declares `flow: ship-wave, mode: manual, pm.provider: asana`.
- Area scope is the lfd engine + builtins surface, not the whole repo:
  `rust/loopflow/src/lfd/`, `rust/loopflow/src/lfd/http/`,
  `rust/loopflow/src/engine/`, `python/loopflow/`,
  `rust/loopflow/src/engine/builtins/steps/`,
  `rust/loopflow/src/engine/builtins/flows/`.
- Ship-already markers live inline in each item doc (e.g. "shipped" annotations in
  `2-planning-flow.md`, `3-vsm-flow.md`, `3-wave-scheduling.md`). Read these
  before starting work — several items have partial completion already merged.

## Roadmap as of 2026-04-23

Remaining items (by filename prefix; lower = earlier tier):

- 2-concurrent-ingest — claim coordination (partial: PM claim via Linear/Asana
  shipped per recent commits; Notion + ordering normalization outstanding)
- 2-planning-flow — chord-tree up/down traversal primitive still missing
- 3-vsm-flow — top-level `vsm` flow chaining the 8 shipped s-level steps
- 3-wave-discovery-and-root-chord — disk scanner + root chord auto-create
- 3-wave-scheduling — `loops`/`crons`/`parent` replacing `mode`+top-level `flow`
- 4-api-expansion — `/v0/waves/{id}/files|file|diff`, steps/flows/directions search
- 4-dag-and-nested-chords — nested chord membership + cycle check
- 4-letta-integration — persistent memory service for the redesign chord
- 4-wave-mutation — typed, logged, reversible mutation API

## Preferences

- Wave flow is `ship-wave`, not `build`. Expect ingest → refresh-plan →
  implement → gate → pr → land style rather than the longer design loop.
- Items are scoped so each produces one PR. Target ~1000 LOC; split milestones if
  they genuinely grow larger.

## Learnings

- 2026-04-23: Headless fire with no user prompt — stopped rather than auto-picking
  an item. Ingest is destructive toward Asana claim state, so a misfire should
  not silently grab an item.
