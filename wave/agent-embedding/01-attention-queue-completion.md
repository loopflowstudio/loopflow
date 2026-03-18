---
asana_id: '1213718096106716'
linear_id: 0f0f48cb-741f-4746-8e75-76113f00b058
---
# 01: Attention Queue Completion

**Finish line:** Design review and chord review checkpoints surface as typed attention items with queue-specific detail and actions, so the attention queue covers every human decision in build and tend flows.

## Context

The attention queue foundation now exists: `AttentionItem` storage and APIs in `lfd`, websocket updates, and a macOS queue home screen that already handles code review, queue failures, and step failures. Terminal waiting work now has its own `TerminalSession` flow, so the remaining queue gap is specifically about the human review checkpoints that still lack durable attention items.

The remaining gap is coverage. Design review and chord review checkpoints still fall back to raw context and never get created by the executor or tend flow. Until those two paths are real, the queue cannot fully replace drilling into individual waves.

The naming model also needs to stay clean while we finish coverage. Today the code-review path still leans on legacy payload shape and `step: code_review`; this item should normalize the queue onto canonical step ids instead of adding more top-level enums or one-off Swift cases.

The contract should stay coarse:

- `interactive_step` — a human needs to review, decide, or continue work
- `algedonic` — the system is signaling pressure, breakage, or blocked progress

The more specific meaning should come from a canonical step identifier in the attention payload, not from proliferating kinds like `design_review` or `calibration`.

## Spec: attention identity and step naming

Use two axes:

1. **Kind** — urgency class
2. **Step** — semantic checkpoint identity

### Kind stays coarse

Keep:

- `interactive_step`
- `algedonic`

Do not add new top-level kinds for code review, design review, calibration, or future queue surfaces. Those are checkpoint types, not urgency classes.

### Step becomes the semantic discriminator

Every attention item should carry a canonical step id in `context.step`. That step id is the stable contract for:

- Swift decoding and view selection
- action button routing
- lifecycle/reconciliation rules
- future analytics
- future built-in prompt naming

Use noun/verb-style ids with `/` separators and **no `.md` suffix**.

Initial target ids:

- `code/review` — code review / ship-ready checkpoint
- `code/design` — design review / kickoff checkpoint
- `chord/review` — chord review / calibration checkpoint

Queue failures and step failures can also carry `context.step`, but they remain `kind: algedonic`.

Examples:

```json
{
  "kind": "interactive_step",
  "context": {
    "step": "code/review",
    "pr_url": "https://github.com/org/repo/pull/42"
  }
}
```

```json
{
  "kind": "interactive_step",
  "context": {
    "step": "code/design",
    "design_path": "scratch/agent-embedding.md"
  }
}
```

```json
{
  "kind": "interactive_step",
  "context": {
    "step": "chord/review",
    "summary": "Three member waves need retargeting"
  }
}
```

### Why step ids instead of a new enum

Do not add a second backend taxonomy like:

```rust
enum InteractiveAttentionType {
    CodeReview,
    DesignReview,
    Calibration,
}
```

That would create three vocabularies that drift:

- prompt step names
- attention subtype enums
- Swift detail view routing

One stable step id is enough.

### Relationship to built-in prompts

The long-term direction is to reorganize built-in prompts around the same noun/verb ids:

- `code/review.md`
- `code/design.md`
- `chord/review.md`

That rename is **not required to finish this item**. For now:

- attention payloads should adopt the canonical ids immediately
- built-in prompt files can keep their current names temporarily
- routing code may map current step names like `review`, `review-design`, and `tend/review-chord` to canonical ids

Follow-up roadmap item after this work lands: rename built-ins and flows so the filesystem, flow references, and attention payloads all use the same ids.

### Swift modeling guidance

Swift should continue to expose typed contexts for rendering, but the discriminator should come from `context.step` plus payload shape, not from new top-level attention kinds.

Expected queue detail groupings:

- `interactive_step + code/review` → review/ship UI
- `interactive_step + code/design` → design review UI
- `interactive_step + chord/review` → chord review / calibration UI
- `algedonic + queue/*` → queue failure UI
- `algedonic + code/*` with error payload → step failure UI

The `.raw` fallback should remain only as a defensive last resort, not a normal path for modeled checkpoints.

## What to build

1. **Design review attention creation.** Wire `review-design` / `kickoff` outputs into `kind: interactive_step` attention items with `context.step = "code/design"`, stable IDs, typed context, and resolution rules tied to the wave advancing or being redirected.

2. **Chord review attention creation.** Wire `tend/draft-chord` / `tend/review-chord` into `kind: interactive_step` attention items with `context.step = "chord/review"` that capture assessment summary, proposed mutations, and any human notes that should feed later tend cycles.

3. **Step-id driven queue detail and actions.** Replace the current raw JSON fallback for design review and chord review with dedicated Swift decoding, filters, detail layouts, and action buttons keyed off canonical `context.step` values.

4. **Lifecycle and urgency polish.** Ensure the new checkpoint types participate in urgency sorting, viewed/resolved transitions, history, and websocket updates the same way queue failures and code reviews do, without introducing new top-level attention kinds.

5. **Proof through tests.** Add Rust and Swift coverage that shows these items are created, rendered, and resolved end to end, including canonical `context.step` routing.

6. **Roadmap the built-in rename explicitly.** Capture the follow-up built-in prompt reorganization so we do not leave attention payload ids permanently out of sync with built-in step paths.

## Built-in prompt reorg follow-up

This item should leave behind a concrete rename plan, but not perform the full migration unless it becomes necessary for shipping.

Target structure:

- `rust/loopflow/src/engine/builtins/steps/code/review.md`
- `rust/loopflow/src/engine/builtins/steps/code/design.md`
- `rust/loopflow/src/engine/builtins/steps/chord/review.md`

Likely mappings from current built-ins:

- `interactive/review.md` → `code/review.md`
- `interactive/review-design.md` → `code/design.md`
- `tend/review-chord.md` → `chord/review.md`

Questions for the follow-up migration:

- whether `kickoff` remains a planning step or becomes part of `code/design`
- whether `tend/draft-chord` and `tend/review-chord` collapse into one canonical review surface with different phases
- whether flow YAML should reference canonical ids directly before or after file moves

Done well, the attention payload, built-in prompt path, and queue UI all switch on the same identifier.

## Done when

- `review-design` or `kickoff` produces `kind: interactive_step` attention items with `context.step = "code/design"` and actionable queue detail
- `tend/draft-chord` or `tend/review-chord` produces `kind: interactive_step` attention items with `context.step = "chord/review"` and mutation review actions
- The queue UI exposes code review, design review, and chord review distinctly and no modeled checkpoint falls back to `.raw` JSON in normal use
- Reconciliation resolves design-review and chord-review items when the human action clears the underlying condition
- The roadmap explicitly records the future built-in prompt reorg to noun/verb-style canonical ids
- A conductor can handle build and tend checkpoints from the queue without opening a wave detail first
