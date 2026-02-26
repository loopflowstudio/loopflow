# Phase 03: Surface-adaptive prompts

**Wave:** living
**Phase:** 03
**Status:** picked

## What it unlocks

Same wave step works across all surfaces (headless, session, TUI, mobile). Agents get surface-appropriate behavior without the step author writing surface-specific logic.

## From the wave plan

> Same step adapts to headless, session, TUI, mobile

Goal from README: "Same wave step works across all surfaces (headless, session, TUI, mobile)"

## Context

Phases 01 (skill injection) and 02 (wave memory) established the pattern: the prompt is the interface, the filesystem is the mechanism. Phase 03 extends this — the prompt assembly pipeline already knows what surface it's running on (CLI vs session vs wave executor). It should thread that knowledge into the assembled prompt so steps can adapt their behavior.

Key surfaces:
- **Headless** (auto mode) — no user interaction, make assumptions, write questions to scratch/
- **Session** (interactive) — user is present, can ask questions, richer output
- **TUI** — terminal UI, constrained display
- **Mobile** (Concerto) — touch interaction, limited screen real estate

The step prompt stays the same. The surface context injected into the assembled prompt tells the agent how to behave.

## Success criteria

- A step run headless behaves differently from the same step run interactively (e.g., asks questions vs writes to scratch/questions.md)
- Step authors don't write `if headless then...` — the surface adaptation is in the assembled context, not the step
- No new APIs or tools — surface info flows through prompt assembly like memory does
- Cold start on any surface works without configuration
