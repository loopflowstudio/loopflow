# 02: Execution

Implement the chord iteration cycle in lfd — inherited trigger tick, start all child waves in parallel, wait for all to complete, repeat.

## What exists after this

A top-level chord fires its stimulus and all descendants execute for that tick in parallel. lfd manages the lifecycle: starting children, waiting for completion, and cycling back to the next trigger. Nesting works — a child chord is just another wave from the parent's perspective and does not run its own independent scheduler.

## How it differs from fork

Fork lifted to the wave level, with three key differences:

1. **Persistence** — fork branches are ephemeral. Chord voices persist across iterations.
2. **Listening** — fork branches are deaf to each other. Chord voices listen (Phase 03).
3. **Stimulus ownership** — the chord owns the trigger, not the children.

## What Phase 01 established

The trigger and executor infrastructure is already enum-aware — triggers call `wave.id()`, `wave.name()`, etc. through the accessor methods. The run entrypoint guard (409 for nested waves) is in place. The fork executor (`wave/fork.rs`) was updated to use the new Wave API and is a potential foundation for parallel child execution. The depth cap (`MAX_CHORD_DEPTH = 8`) applies to nesting here too.

## What to build

### Iteration cycle

1. **Trigger** — top-level stimulus fires (loop/cron/watch/once)
2. **Start all** — child waves begin executing in parallel
3. **Wait all** — chord waits for all waves to complete
4. **Mark result** — set iteration outcome (`ok` / `has_failure`)
5. **Cleanup** — if applicable
6. Back to waiting for next trigger

Phase 03 adds the listen step between trigger and start.

### Stimulus ownership

- Only top-level waves own stimulus configuration
- Nested waves (child voices and child chords) do not have independent triggers
- When the owning ancestor fires, descendant execution is triggered for that tick
- Child chords are orchestrators/grouping nodes, not schedulers
- Direct runs on nested wave IDs are rejected (no implicit reroute)

### Nesting

- Parent starts a child chord, waits for it, treats it as one wave
- Child chord executes once per parent tick (no independent trigger loop)
- Parent has no visibility into child's internal waves (opacity)

### Parallel execution

- All child waves start concurrently
- Chord waits for the slowest child before completing the iteration
- Individual child completion doesn't trigger anything until all are done

### Failure semantics

- If one descendant fails, continue running all other descendants for that tick
- Do not halt sibling waves on first failure
- Tick is marked `has_failure` if any descendant fails
- Parent waits for all started descendants before finishing the tick

## Open questions

- Timeout: should there be a max wait for the slowest child?
- Cleanup policy: what happens between iterations (branch cleanup, artifact management)?
- Cancellation: how does stopping a chord propagate to children? The existing `stop_wave` route kills a single process — needs to walk children.
- Failure status detail beyond `has_failure`: do we add per-child status summary?
- How much of the fork executor infrastructure can be reused vs. needs new parallel orchestration code?

## Done when

- A chord with 2+ voice children executes them in parallel on trigger
- Chord waits for all children before completing the iteration
- Loop stimulus on a top-level chord causes repeated iteration cycles
- A chord containing a child chord executes correctly (nested, inherited tick)
- If one descendant fails, siblings still run and tick is marked failed
- Solo waves (Voice) behave identically to current behavior
- Integration test: chord with 2 voices → parallel execution → all complete → iteration done
- Integration test: chord with 2 voices where one fails → sibling completes → tick marked failed
