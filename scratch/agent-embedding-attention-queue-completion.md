# Attention Queue Completion — PR Plan

## What shipped in this PR

### Architecture: lf owns attention, lfd owns storage

Attention creation moved out of the executor. `lfd` no longer decides when interactive checkpoints produce attention items — that's `lf`'s job via HTTP API. The daemon retains two algedonic creation paths that are genuinely daemon policy:

- **Step failure** — repair chain exhausted, daemon escalates
- **Queue block** — merge queue failed, daemon escalates

Everything else (interactive checkpoints like design review, code review, chord review) will be created by `lf` calling `POST /attention` when it hits those steps. That wiring happens in lfd wave 02 (daemon-aware CLI contract), not this PR.

### Kind collapse: Interactive / Algedonic

Collapsed `AttentionKind` from 5 variants to 2:

| Old | New |
|-----|-----|
| `DesignReview`, `CodeReview`, `Calibration` | `Interactive` |
| `QueueFailure`, `StepFailure` | `Algedonic` |

`context.step` is the semantic discriminator. Legacy kind strings accepted via `FromStr` (Rust) and `init?(rawValue:)` (Swift).

### HTTP API for attention

- `POST /attention` — `lf` creates attention items with kind, context, wave/run IDs
- `POST /attention/{id}/resolve` — `lf` resolves items when interactive steps complete
- `GET /attention` — unchanged, list/filter

### Swift UI

- Two context types: `InteractiveAttentionContext` (step, terminalSessionId, designPath) and `AlgedonicAttentionContext` (step, error, reason, conflictFiles)
- Queue filter: All / Interactive / Escalations
- Detail views with "Open Session" (interactive) and "Retry" (algedonic) actions
- Kind-based context decoding via `AttentionItem.context(kind:json:)`

### Reconciliation

Context-field dispatch instead of kind-based:
- Queue blocks: `reason` + `conflict_files` present → never auto-resolve
- Step failures: `error` without `reason` → resolve when run superseded
- Interactive: resolve when wave restarted (fallback for orphaned items)

## What's left before this PR lands

### 1. Commit the kind collapse
**Status:** Code complete, tests passing, not committed.

Files changed:
- `rust/loopflow/src/lfd/types/attention.rs` — 2-variant enum
- `rust/loopflow/src/lfd/attention.rs` — removed code review creation, interactive step creation, interactive resolution; simplified to queue block + step failure only
- `rust/loopflow/src/lfd/executor/wave/mod.rs` — removed attention creation from WaitInteractive and Complete handlers
- `rust/loopflow/src/lfd/http/routes/attention.rs` — added create/resolve handlers, fixed test variants
- `rust/loopflow/src/lfd/http/mod.rs` — wired new routes
- `rust/loopflow/src/lfd/queue.rs` — updated to `attention_id_for_queue_block`
- `rust/loopflow/src/lfd/store/{sqlite,postgres}.rs` — updated kind references
- `swift/LoopflowCore/Models/AttentionItem.swift` — collapsed model
- `swift/LoopflowCore/State/AttentionStore.swift` — updated sort weights
- `swift/Concerto/Platform/macOS/Views/AttentionQueueView.swift` — new filter/detail
- `swift/LoopflowCore/Services/LocalWaveService.swift` — context parsing
- `swift/ConcertoTests/AttentionStoreTests.swift` — new tests

### 2. Update wave item doc
**Status:** Not started.

`wave/agent-embedding/01-attention-queue-completion.md` still describes the old fine-grained taxonomy and executor-side creation. Rewrite to match reality: collapsed kinds, HTTP API, lf-owns-attention.

### 3. Verify no stale references in other test files
**Status:** Grep showed clean — only legacy compat code references old strings.

Files to double-check:
- `swift/ConcertoTests/EventServiceTests.swift`
- `swift/ConcertoTests/WaveTests.swift`
- `swift/ConcertoTests/WaveRowTests.swift`

### 4. Clean up scratch
**Status:** Not started.

- This file needs to be this file (done)
- `scratch/lfd-tmux-architecture-study.md` — mostly emptied by diff, should either remove or keep as pointer to wave/lfd/01
- `scratch/lfd-terminal-multiplexer-landscape.md` — same, belongs in wave/lfd docs now

## Out of scope (future work)

- **`lf` calling `POST /attention`** — requires daemon-aware CLI contract (lfd wave 02)
- **Algedonic escalation routing** — child wave → parent wave → root wave → human
- **Built-in prompt reorg** — rename step files to `code/review.md` canonical ids
- **Tend remote branch awareness** — surface unlanded sibling branches in calibration
