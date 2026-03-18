# 02: Tend Flow Steps

**Finish line:** `lf tend` runs the full flow against a chord-wave — scan, assess, branch to either compose a chord (draft → review → apply) or reorganize silently. The redesign chord-wave's first tend cycle runs successfully.

## Context

The tend flow and its five steps (`scan-waves`, `assess`, `draft-chord`, `review-chord`, `apply-chord`) are defined as builtins. The flow branches after assess: if pressure points exist, it composes a chord (interactive review with the human); if not, waves reorganize internally via `reorg` (a single beat, no human review).

The scheduling prerequisite is now done:
- the redesign chord-wave is `mode: cron` with daily heartbeat plus `wave` and `block` triggers
- member waves are `mode: managed`
- merged PRs and persistent queue blocks already wake the chord through lfd's trigger runtime

That means the remaining work here is about usable tend input/output, branch execution, and the first real cycle — not about inventing another wakeup path.

Key concepts already in place:
- Silence: waves stay alive with no items, watching their area
- Coherence: update-wave checks whether items still make sense against the current codebase
- Attention market: the chord focuses the user's attention on the most compelling work
- Chords as coordinated mutations proposed by tend, reviewed by the human

## Remaining work

- Replace `scan-waves` shelling-out/ad-hoc gathering with a shared lfd-backed view of runs, PR status, CI, and queue blocks
- Exercise the full tend cycle end-to-end against the redesign chord-wave instead of validating the pieces in isolation
- Validate the branch routing (`tend-chord` vs `reorg`) in the flow engine with a targeted test, including ops items in branch sub-flows
- Make `apply-chord` mutate real wave state through the structured mutation path from item 06, so the first real chord can land instead of stopping at proposal

## Done when

- `lf tend` runs the full flow against the redesign chord-wave
- `scan-waves` reads live lfd state (runs, PRs, CI, persistent blocks) instead of only shell commands and filesystem snapshots
- The branch after assess routes correctly based on assessment content
- First real chord (set of mutations) is drafted, reviewed, and applied
