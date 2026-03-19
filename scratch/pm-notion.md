# PM priority-bucket redo for Asana and Linear

## What this PR should do

Defer actual Notion integration.

Use this PR to redo the roadmap model so it works cleanly with the providers we already have:
- Asana
- Linear

The goal is to prove that the new shared planning model is right before adding Notion on top of it.

## Problem

The current model assumes roadmap items form one exact ordered queue:
- prompts teach `01-*`, `02-*` stages
- `ingest` picks the lowest-numbered item
- PM sync carries `rank` as the main planning signal

That is a bad fit for the tools we actually use. The remote UIs are better at priority buckets than exact shared ordering, and the prompt/docs language is reinforcing the wrong abstraction.

## New model

Roadmaps are binned into four semantic buckets:
- **P0** — The current codebase is broken and needs to be fixed before forward progress can continue.
- **P1** — This is a clear next step.
- **P2** — This is a big idea and a "when not an if" item, but not immediately the right next thing.
- **P3** — This is speculative.

The prompts should assume those four buckets exist in some form. Provider integrations should adapt to the vocabulary they find.

## Provider mapping for this PR

### Asana

Use a custom field that carries the four bucket meanings.

The remote UI may say `P0/P1/P2/P3` or something more native like `Urgent/High/Medium/Low`; the adapter should preserve the bucket meaning.

### Linear

Use Linear's native priority values:
- `P0` ↔ `Urgent`
- `P1` ↔ `High`
- `P2` ↔ `Medium`
- `P3` ↔ `Low`

The point is semantic parity, not label parity.

## Scope

### In scope

1. Update built-in prompt/docs guidance so roadmap writing bins work into the four buckets instead of numeric stages.
2. Update `ingest` to understand the bucket model and pick the highest-priority non-empty bucket first.
3. Redo the shared PM model enough that Asana and Linear can round-trip the bucket meaning cleanly.
4. Keep provider-facing labels native where possible.

### Out of scope

- Notion integration itself
- README sync
- supporting-doc import
- solving exact within-bucket ordering
- arbitrary user-configurable priority taxonomies

## Likely files

- `rust/loopflow/src/engine/builtins/steps/ops/update-wave.md`
- `rust/loopflow/src/engine/builtins/steps/plan/ingest.md`
- `rust/loopflow/src/engine/builtins/steps/interactive/design.md`
- `docs/wave-authoring.md`
- `rust/loopflow/src/ops/ingest.rs`
- `rust/loopflow/src/lfd/pm/mod.rs`
- `rust/loopflow/src/lfd/pm/asana.rs`
- `rust/loopflow/src/lfd/pm/linear.rs`
- `rust/loopflow/src/ops/pm.rs`
- tests covering prompt/docs assumptions, ingest behavior, and provider mapping

## Key decisions

### 1. Prompts define meaning, not spelling

Prompts should talk in the semantics of the four buckets. They should not hardcode that every remote tool literally says `P0/P1/P2/P3`.

### 2. Ingest becomes bucket-aware

`ingest` should choose from the highest non-empty bucket.

Do not block this PR on deciding how to choose between multiple items in the same bucket.

### 3. Native provider language wins in the remote UI

Loopflow keeps the shared meaning. The PM tool can use its own words if that is the more natural UX.

### 4. Notion is explicitly deferred

This PR should leave the codebase ready for Notion rather than adding Notion immediately.

## Done when

- roadmap-writing guidance teaches four semantic buckets
- `ingest` understands those buckets
- Asana and Linear can express and sync the bucket model cleanly
- the codebase is in a better position to add Notion afterwards
