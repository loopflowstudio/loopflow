# 02: Tend Flow Steps

**Finish line:** `lf tend` runs the full flow against a chord-wave — scan, assess, branch to either compose a chord (draft → review → apply) or reorganize silently. The redesign chord-wave's first tend cycle runs successfully.

## Context

The tend flow and its five steps (`scan-waves`, `assess`, `draft-chord`, `review-chord`, `apply-chord`) are defined as builtins. The flow branches after assess: if pressure points exist, it composes a chord (interactive review with the human); if not, waves reorganize internally via `reorg` (a single beat, no human review).

Key concepts already in place:
- Silence: waves stay alive with no items, watching their area
- Coherence: update-wave checks whether items still make sense against the current codebase
- Attention market: the chord focuses the user's attention on the most compelling work
- Chords as coordinated mutations proposed by tend, reviewed by the human

## Remaining work

- Wire tend steps to lfd runtime state (run history, PR status, CI results) — scan-waves currently describes what to read but the data plumbing isn't built
- Test the full tend cycle end-to-end against the redesign chord-wave
- Validate that the branch routing (chord vs reorg) works correctly in the flow engine
- Add a targeted test for ops items in branch sub-flows (the validation was relaxed to allow this)
- Rename `lf ops` → `lf op` across CLI, docs, and flow YAML (aligns with `ConcreteItem::Op`, `Op` struct)

## Done when

- `lf tend` runs the full flow against the redesign chord-wave
- scan-waves reads live lfd state (runs, PRs, CI) not just filesystem
- The branch after assess routes correctly based on assessment content
- First real chord (set of mutations) is drafted, reviewed, and applied
