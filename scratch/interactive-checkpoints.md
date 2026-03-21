# Interactive Checkpoints

## Problem

When a flow hits an interactive step (`review-design`, `wave/review`, `design`, etc.), the run transitions to `Waiting` and a terminal session is created — but nothing appears in the attention queue. The conductor has to notice a wave is waiting by looking at wave status indicators in the sidebar, then manually navigate to it. This defeats the queue's purpose: every human checkpoint should surface as an attention item without the conductor polling.

The infrastructure is already there. `AttentionItem` exists in Rust and Swift. The queue view renders interactive items with step-scoped detail. Resolution and reconciliation logic exists. The executor just never calls the creation path.

## Approach

Wire `AttentionItem` creation into the executor's `WaitInteractive` handler, with step-scoped context that gives the queue enough information to render useful detail without the conductor reading logs.

Three changes, all small:

### 1. Create attention item on WaitInteractive (Rust executor)

In `executor/wave/mod.rs`, after creating the terminal session and before returning from `FlowAction::WaitInteractive`:

```rust
let context = build_interactive_context(&step, &terminal_session, &run);
let item = AttentionItem {
    id: LfdId::new(),
    wave_id: wave.id().clone(),
    run_id: Some(run.id.clone()),
    kind: AttentionKind::Interactive,
    status: AttentionStatus::Surfaced,
    title: interactive_title(&step, &wave),
    summary: interactive_summary(&step, &run),
    context,
    surfaced_at: OffsetDateTime::now_utc(),
    viewed_at: None,
    resolved_at: None,
};
self.store.upsert_attention_item(&item).await?;
self.event_hub.send(Event::attention_created(item.clone()));
```

The `build_interactive_context` function produces step-scoped JSON:

| Step | Context fields |
|------|---------------|
| `review-design` | `step`, `terminal_session_id`, `design_path` (from `scratch/<slug>.md` in step requires/produces) |
| `wave/review` | `step`, `terminal_session_id`, `mutation_summary` (from `scratch/wave-mutate.md` if present) |
| `design` | `step`, `terminal_session_id` |
| Other interactive | `step`, `terminal_session_id` |

The `interactive_title` function produces human-readable titles:
- `review-design` → "Design review: {wave_name}"
- `wave/review` → "Wave review: {wave_name}"
- Other → "Interactive: {step_name}"

The `interactive_summary` reads the first meaningful line from the design doc or mutation summary to give the conductor a one-line preview of what they're being asked to review.

### 2. Auto-resolve on terminal session completion (Rust executor)

In `wait_for_terminal_session_and_resume`, after the session completes and before advancing the run:

```rust
// Resolve the attention item for this run's waiting step.
if let Some(item) = store.find_attention_item_for_run(&run.id, AttentionKind::Interactive).await? {
    resolve_attention_item(&store, &item.id).await?;
    event_hub.send(Event::attention_resolved(item.id.clone()));
}
```

Add a store query `find_attention_item_for_run(run_id, kind)` — single-row lookup by `run_id` + `kind` + non-resolved status.

The existing `reconcile_attention_items` already handles the stale case (newer run exists), but explicit resolution on completion is cleaner: the item resolves the moment the human finishes, not on the next reconciliation sweep.

### 3. Step-scoped detail rendering (Swift)

`AttentionDetailView` already switches on `item.context` and renders `step`, `designPath`, and the "Open Session" button. Two enhancements:

**a. Review-design detail:** When `context.step == "review-design"` and `designPath` is present, show a markdown preview of the design doc inline (reuse the existing `MarkdownContentView` from the multiplexer). The conductor can read the design without opening the session first.

**b. Wave/review detail:** When `context.step == "wave/review"`, parse `context.mutation_summary` (a short text summary of proposed mutations) and display it as structured text. The conductor sees what the chord is proposing before opening the session.

Both fall back gracefully to the current generic rendering if the fields are absent.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Create attention item from `lf` CLI via HTTP POST | Decouples creation from executor; steps could self-report | Adds a new responsibility to every interactive step prompt. The executor already knows the step is interactive — it should own the item lifecycle. CLI-side creation also risks missing items if the step crashes before posting. |
| Add new `AttentionKind` variants per step type | Stronger typing; queue can switch on kind | Violates the deliberate coarse-kind design. `Interactive` + `context.step` is the right discrimination axis. Adding `DesignReview`, `WaveReview`, `Calibration` as kinds would require Rust/Swift enum expansion for every new interactive step. |
| Render full design doc in queue detail | Conductor sees everything without opening terminal | Design docs can be long. A one-line summary + markdown preview (truncated) is enough to decide whether to engage. The terminal session is where the real interaction happens. |

## Key decisions

**Executor owns creation and resolution.** The attention item lifecycle is a daemon concern, not a step concern. Steps don't need to know about attention items — they just declare `interactive: true` in frontmatter and the executor handles the rest. This keeps the step prompt clean and the lifecycle consistent.

**Step-scoped context, not step-scoped kinds.** The `context.step` string is the discrimination axis. Swift already has `InteractiveAttentionContext` with `step`, `terminalSessionId`, `designPath`. We add `mutationSummary` for wave/review. Future interactive steps (calibration, code review) add fields to the context, not new enum cases.

**Explicit resolve on completion, reconciliation as backstop.** The happy path resolves the attention item the moment the terminal session completes. The existing reconciliation sweep catches anything that slips through (killed processes, stuck callbacks). No new reconciliation logic needed.

**Summary from artifact, not from step prompt.** The attention item summary comes from the design doc or mutation summary file, not from the step's prompt text. This gives the conductor a preview of *what they're reviewing*, not *what the step does*.

## Scope

- In scope: attention item creation on `WaitInteractive`, auto-resolve on session completion, step-scoped context for `review-design` and `wave/review`, enhanced detail rendering in queue view, Rust and Swift tests
- Out of scope: calibration view (item 04, depends on this), algedonic routing through wave hierarchy, new attention kinds, portfolio-level attention aggregation

## Done when

- `review-design` and `wave/review` produce attention items in the queue with typed context
- Tapping an interactive attention item shows step-scoped detail (design doc preview or mutation summary) and "Open Session" button
- Completing the terminal session resolves the attention item
- `cargo test --all` passes with new tests covering creation, context building, and resolution
- `swift test --package-path swift` passes with new tests covering context parsing and detail rendering
- A conductor running `build` flow sees the design review checkpoint appear in the queue, can preview the design, open the session, and see the item resolve when they finish

## Measure

**Before:** Interactive checkpoints produce zero attention items. The conductor discovers waiting waves by scanning the sidebar.

**After:** Every `WaitInteractive` step produces exactly one attention item. Verify with:

```bash
# Start a build flow that reaches review-design
lfq run engbot

# Check attention items appear
curl -s localhost:7475/attention | jq '.[] | select(.kind == "interactive")'

# Complete the session, verify resolution
curl -s localhost:7475/attention | jq '.[] | select(.status == "resolved")'
```

Target: 100% of interactive steps produce attention items (wave goal: "Share of unresolved human checkpoints represented as attention items → 100%").
