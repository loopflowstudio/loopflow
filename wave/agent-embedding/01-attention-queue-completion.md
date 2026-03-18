---
asana_id: '1213718096106716'
linear_id: 0f0f48cb-741f-4746-8e75-76113f00b058
---
# 01: Attention Queue Completion

**Finish line:** The attention system has two clean paths — interactive (human checkpoint) and algedonic (system escalation) — with an HTTP API that lets `lf` create and resolve attention items, so flow semantics stay in the CLI.

## What shipped

### Kind collapse

`AttentionKind` collapsed from 5 fine-grained variants to 2 urgency classes:

| Path | Meaning | Created by |
|------|---------|------------|
| `interactive` | `lf` at a step needing human input | `lf` via `POST /attention` |
| `algedonic` | System escalation — failure, blockage | `lfd` (step failure, queue block) or `lf` via API |

`context.step` is the semantic discriminator within each path. Legacy kind strings (`design_review`, `code_review`, `calibration`, `queue_failure`, `step_failure`) accepted during migration via `FromStr` (Rust) and `init?(rawValue:)` (Swift).

### lf-owns-attention architecture

Attention creation removed from the executor. The daemon no longer has flow semantics for interactive checkpoints. Two creation paths remain:

1. **HTTP API** — `POST /attention` and `POST /attention/{id}/resolve` for `lf` to create/resolve items when hitting interactive steps
2. **Daemon policy** — `create_step_failure_attention` (repair chain exhausted) and queue block attention (merge queue failed)

Code review attention removed entirely — loopflow uses `land`, not PR review gates.

### Swift UI

- `InteractiveAttentionContext` (step, terminalSessionId, designPath) and `AlgedonicAttentionContext` (step, error, reason, conflictFiles)
- Queue filter: All / Interactive / Escalations
- Detail views: "Open Session" action for interactive, "Retry" for algedonic
- Kind-based context decoding via `AttentionItem.context(kind:json:)`

### Reconciliation

Context-field dispatch instead of kind-based. Queue blocks (reason + conflict_files) never auto-resolve. Step failures resolve when run superseded. Interactive items resolve when wave restarted (fallback).

## What's next (not this PR)

### `lf` calling the attention API

Requires the daemon-aware CLI contract (lfd wave 02). When `lf` hits `WaitInteractive` for a checkpoint step, it should `POST /attention` with the appropriate context. When the step completes, it should `POST /attention/{id}/resolve`.

### Algedonic escalation routing

The algedonic path should route through the wave hierarchy: child wave → parent wave → root wave → human. Only the root wave's escalation surfaces as a human attention item. This makes agent-to-agent escalation possible within a wave family before bothering a human.

### Built-in prompt reorg

Rename step files so filesystem path, flow YAML reference, and `context.step` all use the same `noun/verb` canonical identifiers:

- `interactive/review.md` → `code/review.md`
- `interactive/review-design.md` → `code/design.md`
- `tend/review-chord.md` → `chord/review.md`

### Tend remote branch awareness

Surface unlanded sibling branches in calibration context so conductors can see integration pressure.

## Done when

- `AttentionKind` has exactly two variants: `Interactive` and `Algedonic`
- `POST /attention` and `POST /attention/{id}/resolve` exist and work
- Executor does not create interactive attention items
- Swift queue UI renders interactive and algedonic with typed contexts (no `.raw` fallback for modeled items)
- Legacy kind strings decode correctly in both Rust and Swift
- All Rust and Swift tests pass
