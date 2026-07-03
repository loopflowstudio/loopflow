---
priority: medium
asana_id: '1216273057538130'
---

# Prune the flow/step vocabulary for the wave-agent world

**Finish line:** The builtin `build`/`ops` flows and steps are trimmed to what
still earns its place now that goals are the authoring surface and VSM systems
ship as charters. No flow survives just because it once did; the vocabulary maps
to how waves actually run.

## Context

The vocabulary grew up around *directed, one-shot flows* (`govern-*`
`scan→assess→mutate` pipelines, `build`/`ops` chains). The goals model changes
the frame: a Wave loops a Goal that *dispatches* flows as inner work. Some flows
are still the right hands; others encode assumptions from before goals existed
(the `direction` perspective field, inline operate prompts, staged pipelines a
looping agent would compose differently).

This is the complement of `3-vocabulary-completeness` — that item *adds* missing
atoms (scaffold, run, integrate) for greenfield builds; this one *removes* atoms
that no longer fit. Both are about making the vocabulary honest.

Related cleanups already scoped elsewhere, sequence against them:

- `workflows/3-remove-directions` — retire the `direction` field and redistribute
  its perspective text into step-skill bodies.
- `workflows/3-unify-operate-prompt` — fold the inline `loopflow.goal` operating
  prompt into `LOOPFLOW_DOC`; retire the duplicate.

## Open decision — gstack

`gstack` (~38 steps, 3 flows, a Python converter, cleanly namespaced) is dormant.
Keep it parked as a namespaced skill bundle or cut it entirely — either way it's
its own small PR. Decide as part of this pruning pass; don't let it rot
half-alive. (Also tracked in `wave/root/backlog.md`.)

## What to shape

- **Inventory `build`/`ops` flows and steps** against the wave-agent contract
  (read roadmap/metrics → dispatch a flow → create subwaves → run adhoc flows).
  For each: does a looping goal still need this as a distinct flow, or would the
  agent compose it from smaller pieces?
- **Delete, don't deprecate.** Per repo style, no compatibility shims — remove the
  flow and its steps, lean on git for history.
- **Keep the charters and their hand-flows.** The five `govern-*` flows stay as
  the VSM systems' hands (see `4-vsm-standing-loops`); this pruning targets
  `build`/`ops`, not `govern`.

## Done when

- Every surviving `build`/`ops` flow has a clear reason to exist in the
  goal-dispatch world; the rest are deleted.
- The gstack keep-or-cut decision is made and executed.
- `cargo test` and `uv run pytest python/tests/` pass with the pruned set.
