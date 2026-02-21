# interactive/review

Replace the current `plan/review` (rename to `plan/research`), create a new `interactive/review` step.

## What to build

An interactive step that walks the human through the current diff. Reads the gate briefing doc if present, reads the diff, then has a structured conversation about model clarity, simplification, controversial decisions, demo plan, and learnings. Produces nothing — the conversation leads to either direct code changes or a design doc for the next iteration.

## Rename

- `plan/review.md` → `plan/research.md`
- Update all flow YAML files that reference `review` to reference `research`
- Update flows README

## New step: `interactive/review.md`

```yaml
---
interactive: true
requires: diff vs main
produces: code changes | design doc | nothing
---
```

Walk the human through the current diff. If `scratch/<branch>-review.md` exists (from gate), use it as the briefing. Otherwise, read the diff cold.

### Arc

Each phase is a conversation pause. Present findings, wait for reaction, adjust.

1. **Orient** — Summarize the shape of the change in 2-3 sentences. If gate briefing exists, use it. Don't recite the diff.

2. **Core model** — Walk through the central data structures and APIs. Explain the model, then ask: is this the clearest expression of the product semantics? Are names right? Are boundaries right?

3. **Simplify** — Propose concrete alternatives. Show what a simpler version looks like — different type hierarchies, merged structs, eliminated indirection. Not "you could simplify" — show the code.

4. **Contentious calls** — Surface decisions reasonable people would disagree on. Frame as tradeoffs, not problems.

5. **Demo plan** — Propose specific commands, workflows, UX to exercise the change.

6. **Learnings** — What did building this reveal? What would we do differently?

### Key principles

- Each phase pauses for human input. Don't monologue.
- If something should change, change it directly or write a design doc. No review artifacts.
- The gate doc is the agenda, not a script. Skip sections that don't apply.
- Focus on structural decisions, not formatting or style.

## Constraints

- Don't break existing flows that use `review` — they must reference `research` after rename
- The new step must be `interactive: true`
- No produced artifact — the step's value is the conversation and resulting action

## Done when

- `plan/research.md` exists with old review content
- `plan/review.md` is gone
- All flow YAMLs reference `research` not `review`
- `interactive/review.md` exists with the new interactive step
- `cargo test` passes (golden prompt tests may need updating)
