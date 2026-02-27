# Gate Review: interactive step voice + PROMPT_STYLE dynamism

## What changed

Voice sections added to all interactive steps (except `explore`) and codified in PROMPT_STYLE. The `review-design` lens naming was sharpened.

1. **PROMPT_STYLE.md** — New "Dynamic, not formulaic" principle in the Voice section. Codifies that interactive prompts should vary across sessions.

2. **Voice sections added** to `design`, `review`, `refine`, and `review-design` (both `.lf/steps/` and `rust/.../builtins/` copies). Each tailored to the step's nature. `explore` skipped — it's deliberately reactive.

3. **`review-design` sharpened** — "Key bet" → "core decisions" (more actionable). "The design doc is a bet —" sentence removed (redundant with "This is the last cheap moment to change it").

4. **Golden prompt updated** — `tests/goldens/builtin_review.md` regenerated to include the new Voice section from the builtin review step.

5. **Quote-replies design doc** added to `scratch/mobile-quote-replies.md` as reference for future implementation. Wave backlog item preserved.

## Key choices

- **Voice sections are directive, not structural.** They tell agents *how to approach* the session, not what format to follow. Matches PROMPT_STYLE's principle of judgment over process.

- **Both copies updated in sync.** `.lf/steps/` and `rust/.../builtins/steps/interactive/` have identical body content. Frontmatter correctly differs (builtins have `default_agent` and `action_style`).

- **`explore` excluded.** Its purpose is to be a reactive tool — "the human is in charge" is the right posture. Adding dynamism instructions would fight its nature.

## Risks

- **Prompt regression** — Voice sections could cause agents to over-index on novelty and skip substance. Mitigated by unchanged structural instructions ("Pick the lenses that matter most", phase ordering in design, etc.).

## Pre-existing notes

- **design.md body divergence** — `.lf/steps/design.md` and `rust/.../builtins/.../design.md` have pre-existing body differences (branch naming paragraph, "sprints" vs "staged wave items"). Not introduced by this branch. Out of scope here.

## What's not included

- No runtime code changes.
- Quote-replies implementation is separate work tracked by `wave/mobile/01-quote-replies.md`.
