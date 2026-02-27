# Gate Review: review-design voice + wave cleanup

## What changed

Three changes, one intent: sharpen the `review-design` step and clean up a consumed wave item.

1. **Voice section added to `review-design`** — New section instructs agents to vary structure, tone, and entry point across design reviews. Aims to prevent formulaic rubber-stamp reviews.

2. **"Key bet" → "core decisions"** — The lens label changed from "Intent and key bet" to "Intent and core decisions." "Core decisions" is more actionable — it tells the agent to identify the decisions other choices depend on, not just the riskiest single bet.

3. **`wave/mobile/01-quote-replies.md` deleted** — The quote-replies wave item has been consumed (design doc exists at `scratch/mobile-quote-replies.md` via the loaded context). Removing it from the wave backlog reflects that it's now in-flight work, not a backlog item.

## Key choices

- **Voice section is directive, not structural.** It tells agents *how to think* ("be genuinely curious, not procedural") rather than prescribing a format. This matches the PROMPT_STYLE principle of goals providing judgment over process.

- **Both copies updated in sync.** `.lf/steps/review-design.md` and `rust/loopflow/src/engine/builtins/steps/interactive/review-design.md` have identical body content. Frontmatter correctly differs (builtin has `default_agent` and `action_style`).

- **Removed "The design doc is a bet" framing.** The original sentence mixed metaphor ("bet") with instruction ("last cheap moment to change it"). The edit keeps the actionable part.

## How it fits together

The `review-design` step is used in `ship-roadmap` and interactively via `lf review-design`. The voice section and lens rename affect all future design reviews — both interactive and flow-driven. No runtime code changes; these are prompt-only edits embedded as Rust builtins and compiled into the `lf` binary.

## Risks

- **Prompt regression** — The voice section could cause agents to over-index on novelty and skip important lenses. Mitigated by the existing "Pick the lenses that matter most" instruction, which is unchanged.
- **None** on the wave deletion — no references to `01-quote-replies.md` exist elsewhere.

## What's not included

- No changes to other steps or flows.
- No runtime code changes.
- The quote-replies implementation itself is separate work tracked by the design doc in context.
