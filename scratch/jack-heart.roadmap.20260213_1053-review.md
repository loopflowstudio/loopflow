# Design review: redesign the design step

## What was implemented

Rewrote the `design.md` interactive step prompt from a scope-first approach ("what's the smallest version?") to a dream-first approach with four phases: Dream, Detail, Size-check, Fork.

The prompt now lets users explore the full idea before deciding scope, then explicitly forks into either "ship as one commit" (proceed to `lf implement`) or "roadmap it" (break into stages, proceed to `lf add-to-roadmap`).

## Key choices

**Four-phase structure over freeform.** The old prompt biased toward brevity from the start, which meant scope constraints arrived before the idea was fully explored. The new prompt separates ideation (phases 1-2) from scoping (phases 3-4), letting the detailing process itself reveal what matters most.

**~1000 words / ~1000 LOC as heuristics, not rules.** These are soft signals that trigger the roadmap conversation. The user can override in either direction. Bias is toward single commits when close.

**Explicit fork checkpoint.** The Phase 4 fork serves double duty: it's both a scoping decision and a natural session exit signal. The agent tells the user exactly what command to run next.

**Roadmap output format reuses existing conventions.** `scratch/roadmap-proposal.md` uses the same `status: proposed` / Context / Scope / Approach structure that `add-to-roadmap` already consumes.

## How it fits together

The design step is one prompt in a chain: `design → implement → gate` (the `ship` flow). This change doesn't affect downstream steps — `implement` still reads `scratch/<branch>.md` as before. The new roadmap fork adds a second exit path (`design → add-to-roadmap`) that feeds into the plan flows.

## Risks and bottlenecks

**Prompt length.** The new prompt is longer than the old one (~116 lines vs ~65 lines). This is acceptable — the phases need clear instructions to prevent agents from collapsing back to scope-first habits. But worth watching if the prompt grows further.

**Phase discipline.** An agent might skip the Dream/Detail phases and jump to Size-check. The "don't skip ahead" instruction mitigates this, but it's a soft constraint.

## What's not included

- No changes to `roadmap`, `add-to-roadmap`, `kickoff`, or `ingest` prompts
- No changes to flows — the step chains work the same way
- The roadmap fork path (`scratch/roadmap-proposal.md`) relies on the existing `add-to-roadmap` step to promote items; no new automation added
