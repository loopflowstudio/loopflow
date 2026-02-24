# 03: Listen Step

Inter-voice communication through a dedicated step that runs at the start of each chord iteration. Voices digest sibling output and adapt plans before their own work begins.

## What exists after this

Each voice in a chord runs a listen step at the start of every iteration (after the first). The listen step injects sibling PR content, reads the voice's own plans, adapts in response, and writes updated artifacts. The voice's normal flow then runs with adapted plans baked in.

## Why a step, not a context source

- Doesn't pollute the context of every step in the flow
- Discrete moment of reflection: "what did my chord-mates do, how should I adjust?"
- Produces durable artifacts (updated docs) rather than ephemeral context
- Can be as cheap or expensive as needed (config controls PR content depth)

## What to build

### Listen step mechanics

1. Get sibling voices' recent PR(s) injected into context
2. Read own wave roadmap and design docs
3. Adapt plans in response to what siblings did
4. Write updated wave/scratch docs
5. Normal flow runs with the updated plans

The output is **modified plans**, not a summary riding along in every step's context. The adaptation is baked into artifacts before work starts.

### Sibling output injection

- Identify sibling voices within the same chord
- Gather recent PR(s) from each sibling since last iteration
- Default-off config for full PR content (diffs, comments)
- Lightweight mode: PR titles and summaries only

### Iteration-aware scheduling

- First iteration: skip listen step (nothing to listen to yet)
- Subsequent iterations: listen step runs before start-all
- Listen step must complete for all voices before any voice starts its main flow

### Configuration

- Per-chord or per-voice listen depth (full PR / summary only / off)
- Listen step prompt is configurable (default provided)
- Opt-out for voices that don't need to listen

## What we'll learn

- Whether PR content is the right granularity for inter-voice communication
- How much context the listen step needs to make useful adaptations
- Whether voices need to listen to specific siblings or all of them
- First iteration semantics: skip entirely, or run a "hello" step for initial coordination?

## Open questions

- Should the listen step have access to sibling wave/ docs in addition to PRs?
- How to handle a voice that has nothing new since last iteration (no PR)?
- Should listen step output be visible to siblings in the next iteration's listen?

## Done when

- Listen step runs automatically at the start of chord iteration 2+
- Sibling PR content is injected into listen step context
- Listen step produces updated artifacts (wave/scratch docs)
- Voice's main flow uses the adapted plans
- Skipped on first iteration
- Config controls PR content depth (full/summary/off)
- Integration test: 2-voice chord → iteration 1 (no listen) → iteration 2 (listen adapts plans) → verify adaptation
