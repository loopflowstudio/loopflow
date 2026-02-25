# Interactive Sessions in Flow Execution

When a wave flow hits an interactive step (design, review, explore, refine), lfd should surface an agent session in Concerto automatically.

## Current state

Start Wave → interactive design step works: user enters a wave name, Concerto creates the wave and launches `lf design` in the wave's worktree via embedded terminal. On exit, `lf ops commit --push` auto-commits and pushes.

This is a single interactive step, not a flow. The user runs follow-up steps manually from the StepRunner.

## Progression

1. **Now:** Start wave → interactive `design` → commit+push. Single step, manual follow-up.
2. **This phase:** Default NUX flow becomes `design → ship → review`. lfd runs the flow; when it hits an interactive step, Concerto surfaces the session inline. Auto steps run headless as today.
3. **Steady state:** Waves run the `ship-roadmap` loop (`ingest → kickoff → review-design → ship → review`). Interactive steps surface in Concerto; auto steps run in the background.

## What needs to happen

Route wave executor interactive steps through the session orchestration path:

1. lfd recognizes a step as interactive (from step frontmatter or config)
2. lfd creates a session and signals Concerto via WebSocket that an interactive session is waiting
3. Concerto surfaces the session inline on the wave detail panel (InteractiveSessionView takes over)
4. User interacts with the session until the step completes
5. lfd auto-commits, then continues the flow to the next step

## Dependencies

- agentapi phases 01-03 (shipped): session API, harness, Concerto UI
- wavemodel phase 03 (shipped): design-first onboarding, wave content display
- Runtime convergence: wave executor and session paths share the same orchestration

## What's not here

- Multi-session per step (parallel agents within one interactive step)
- Session handoff between users
- Background sessions that don't require user interaction (those just run as auto steps)
- Switching InteractiveSessionView from embedded terminal to session API (separate agentapi work)
