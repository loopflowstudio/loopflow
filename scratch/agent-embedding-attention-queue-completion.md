# Attention Queue Completion

## Problem

The attention queue handles code review, queue failures, and step failures — but design review and chord calibration checkpoints never become attention items. A conductor running parallel waves still has to drill into individual wave details to find interactive checkpoints. The queue's promise — every human decision in one place — is incomplete.

Who benefits: any conductor running build or tend flows. Why now: the queue foundation, terminal sessions, and wave workspace all shipped. Coverage is the last gap before the queue can fully replace wave-by-wave inspection.

## Approach

Wire attention item creation into the `WaitInteractive` handler in the executor. When a flow pauses for an interactive step, the executor already creates a terminal session and emits `WaveWaiting`. Add attention item creation at the same seam, using the step name to determine the attention kind and context shape.

Keep the existing fine-grained `AttentionKind` variants (`DesignReview`, `CodeReview`, `Calibration`) for now. Add a canonical `context.step` field to all attention items as the semantic discriminator for Swift view routing and future taxonomy collapse. This matches the spec's sequencing guidance: ship coverage first, collapse kinds later.

### Rust: creation functions

Add two new creation functions in `attention.rs`:

```rust
pub async fn create_interactive_step_attention(
    store: &SharedStore,
    wave: &Wave,
    run: &WaveRun,
    step_name: &str,
    terminal_session_id: &LfdId,
) -> Result<AttentionItem, String>
```

This single function handles both design review and calibration by mapping the step name to the appropriate kind:

| Step name pattern | Kind | `context.step` | Extra context fields |
|---|---|---|---|
| `review-design`, `kickoff` | `DesignReview` | `code/design` | `design_path` (scratch slug) |
| `review-chord` | `Calibration` | `chord/review` | `chord_path` (scratch slug) |
| Other interactive | Skip — no attention item | — | — |

Steps like `design`, `explore`, `demo`, `refine` are exploratory and don't represent checkpoints that need queue surfacing. Only checkpoint-type interactive steps (where a human verdict gates flow progress) produce attention items.

All attention contexts include `terminal_session_id` so the queue UI can route directly to the right terminal.

### Rust: executor wiring

In `WaitInteractive` handler (executor/wave/mod.rs ~line 508), after creating the terminal session and before returning:

```rust
FlowAction::WaitInteractive { step } => {
    let terminal_session = self.create_terminal_session(&wave, &run, &step).await?;
    let terminal_session_id = terminal_session.id.clone();
    self.spawn_terminal_session_watcher(/* ... */);

    // NEW: create attention item for checkpoint steps
    if let Some(item) = create_interactive_step_attention(
        &self.store, &wave, &run, &step.step.name, &terminal_session_id
    ).await? {
        self.event_hub.send(Event::attention_created(item));
    }

    run.status = WaveRunStatus::Waiting;
    // ... rest unchanged
}
```

### Rust: resolution

Attention items for interactive steps resolve when the terminal session completes. In `wait_for_terminal_session_and_resume`, after confirming the session ended successfully and before resuming the flow, resolve any attention items for this run that match the interactive step kinds:

```rust
// Resolve interactive attention when session completes
resolve_interactive_attention(&self.store, &run_id, &self.event_hub).await;
```

This function queries attention items for the run, filters to `DesignReview` or `Calibration` with status != `Resolved`, and marks them resolved. The existing `should_resolve_when_wave_restarted` reconciliation stays as a fallback for edge cases (crashed watchers, orphaned items).

### Rust: also add `context.step` to existing attention items

Update `create_code_review_attention` to include `"step": "code/review"` in context. Update `create_step_failure_attention` to prefix the step name with canonical form where possible. This makes the step field universal across all attention items.

### Swift: typed contexts

Add two new context cases to `AttentionContext`:

```swift
case designReview(DesignReviewAttentionContext)
case calibration(CalibrationAttentionContext)

struct DesignReviewAttentionContext {
    let step: String           // "code/design"
    let designPath: String?    // scratch file path
    let terminalSessionId: String?
}

struct CalibrationAttentionContext {
    let step: String           // "chord/review"
    let chordPath: String?     // scratch file path
    let terminalSessionId: String?
}
```

Update `AttentionContext.context(json:)` to decode these based on the `step` field:
- `step` starts with `"code/design"` → `.designReview`
- `step` starts with `"chord/"` → `.calibration`
- `pr_url` present → `.codeReview` (unchanged)
- `reason` present → `.queueFailure` (unchanged)
- `error` present without `step` starting with above → `.stepFailure` (unchanged)

### Swift: queue detail views

In `AttentionDetailView`, add rendering for the new contexts:

**Design review detail:**
- Title: wave name + "needs design review"
- Summary from attention item
- Action: "Open Session" → navigates to the terminal session (via `terminalSessionId`)
- The terminal session is where the actual review-design interactive step runs

**Calibration detail:**
- Title: wave name + "chord review ready"
- Summary from attention item
- Action: "Open Session" → navigates to the terminal session
- The terminal session is where the actual review-chord interactive step runs

Both cases route the conductor to the embedded terminal where the interactive step agent is waiting. The attention item surfaces the checkpoint; the terminal session is where work happens.

### Tend: remote branch awareness

Also extend the tend scan so it looks at remote branches and spots unlanded work that appears to belong to the same wave family. The point is not to create more attention items directly from git state; it is to give `tend/scan-waves` and calibration a better picture of integration pressure.

Proposed shape:

- Add a remote-branch summary to the wave/tend data path, sourced from `git ls-remote --heads <remote>` or equivalent daemon-side git query
- Match candidate branches using the existing branch naming schema plus wave/worktree naming conventions (`<user>.<wave>.<timestamp>`, run branches, related stacked branches)
- Filter to branches that are not already merged/landed and are not the wave's currently tracked branch
- Surface the result as "unlanded related branches" / "integration candidates" in tend assessment context
- Use that signal in calibration UI and queue copy so the conductor can see that there is sibling work waiting to be integrated

This is a tend-awareness feature, not a replacement for explicit trigger relationships. The goal is: if there is remote work that looks like part of this wave but has not landed yet, calibration should notice it and factor it into recommendations.

### Swift: action routing

Add to `RepoState` or equivalent:

```swift
func openTerminalSession(_ sessionId: String)
```

This selects the wave containing the session and switches to the terminal tab showing that session. The plumbing for this already exists in `TerminalWorkspaceStore` — the action button just needs to trigger navigation.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Collapse to coarse kinds (`interactive_step` / `algedonic`) first | Cleaner model upfront, but requires migrating existing code review and failure paths simultaneously | Spec says ship coverage first, collapse later. Touching code review now risks breaking what works. |
| Create attention items for all interactive steps | Uniform coverage | Exploratory steps (design, explore, demo) aren't checkpoints — surfacing them adds noise, not signal |
| Attention items without terminal session links | Simpler context | The whole point is "act on it from the queue" — without the session link, the conductor still has to find the right terminal manually |
| Separate creation functions per kind | More explicit | One function with a step-name match table is simpler and easier to extend. The match table is the source of truth for which steps are checkpoints. |

## Key decisions

1. **One creation function, step-name dispatch.** A single `create_interactive_step_attention` maps step names to kinds. Adding a new checkpoint type means adding one match arm, not a new function + new call site.

2. **`context.step` as universal field.** All attention items get a canonical step identifier. This is the migration bridge: Swift can start routing on `context.step` now, and when kinds collapse to `interactive_step` / `algedonic` later, the step field is already the discriminator.

3. **Resolution on session completion, not just wave restart.** Interactive attention items resolve immediately when their terminal session ends. The reconciliation fallback handles crashes. This means the queue stays clean — no stale items lingering after the conductor finishes a review.

4. **Only checkpoint steps get attention items.** `review-design`, `kickoff`, `review-chord`, and `code-review` are checkpoints. `design`, `explore`, `demo`, `refine` are exploratory. The distinction: a checkpoint gates flow progress on a human verdict. An exploratory step is an open-ended session.

5. **Terminal session ID in context.** The queue's "Open Session" action needs to navigate to the right terminal. Including the session ID in the attention context makes this a single lookup.

## Scope

- **In scope:**
  - `create_interactive_step_attention` in `attention.rs` with step-to-kind mapping
  - Wiring in `WaitInteractive` handler
  - Resolution in `wait_for_terminal_session_and_resume`
  - Adding `context.step` to existing code review and step failure items
  - Swift `DesignReviewAttentionContext` and `CalibrationAttentionContext`
  - Queue detail views and "Open Session" action for both new contexts
  - Rust and Swift tests proving creation, rendering, and resolution
  - Roadmap note for future kind collapse and built-in prompt reorg

- **Out of scope:**
  - Collapsing `AttentionKind` to coarse urgency classes (follow-up)
  - Renaming built-in prompt files to canonical `code/review.md` structure (follow-up)
  - Attention items for exploratory interactive steps
  - Changes to code review creation path (already works)

## Done when

- `review-design` or `kickoff` hitting `WaitInteractive` creates a `DesignReview` attention item with `context.step = "code/design"` and `terminal_session_id`
- `review-chord` hitting `WaitInteractive` creates a `Calibration` attention item with `context.step = "chord/review"` and `terminal_session_id`
- Code review attention items include `context.step = "code/review"`
- Queue UI renders design review and calibration with typed detail views (no `.raw` fallback)
- "Open Session" action navigates to the terminal session where the interactive step is running
- Attention items resolve when the terminal session completes
- Reconciliation fallback still resolves orphaned items when wave restarts
- `cargo test` covers creation, context shape, and resolution for both new kinds
- Swift tests cover context decoding and store behavior for new attention contexts

## Built-in prompt reorg roadmap

This item leaves behind the follow-up tracked in the wave item spec (lines 162–183 of `wave/agent-embedding/01-attention-queue-completion.md`). The target: rename built-in step files so the filesystem path, flow YAML reference, and `context.step` value all use the same `noun/verb` canonical identifiers. Not required for this PR.
