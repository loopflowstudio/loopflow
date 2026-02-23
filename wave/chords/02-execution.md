# 02: Execution

Implement the chord iteration cycle in lfd — trigger, start all child waves in parallel, wait for all to complete, repeat.

## What exists after this

A chord fires its stimulus and all child waves execute in parallel. lfd manages the lifecycle: starting children, waiting for completion, and cycling back to the next trigger. Nesting works — a child chord is just another wave from the parent's perspective.

## How it differs from fork

Fork lifted to the wave level, with three key differences:

1. **Persistence** — fork branches are ephemeral. Chord voices persist across iterations.
2. **Listening** — fork branches are deaf to each other. Chord voices listen (Phase 03).
3. **Stimulus ownership** — the chord owns the trigger, not the children.

## What to build

### Iteration cycle

1. **Trigger** — stimulus fires (loop/cron/watch/once)
2. **Start all** — child waves begin executing in parallel
3. **Wait all** — chord waits for all waves to complete
4. **Cleanup** — if applicable
5. Back to waiting for next trigger

Phase 03 adds the listen step between trigger and start.

### Stimulus ownership

- The chord owns the stimulus configuration
- Child voices don't have independent triggers
- When the chord fires, all children fire
- A child chord manages its own internal iteration cycle once started

### Nesting

- Parent starts a child chord, waits for it, treats it as one wave
- Child chord runs its own iteration cycle internally
- Parent has no visibility into child's internal waves (opacity)

### Parallel execution

- All child waves start concurrently
- Chord waits for the slowest child before completing the iteration
- Individual child completion doesn't trigger anything until all are done

## Open questions

- Failure semantics: one voice fails, does the chord continue or halt all?
- Timeout: should there be a max wait for the slowest child?
- Cleanup policy: what happens between iterations (branch cleanup, artifact management)?
- Cancellation: how does stopping a chord propagate to children?

## Done when

- A chord with 2+ voice children executes them in parallel on trigger
- Chord waits for all children before completing the iteration
- Loop stimulus causes repeated iteration cycles
- A chord containing a child chord executes correctly (nested iteration)
- Solo waves (Voice) behave identically to current behavior
- Integration test: chord with 2 voices → parallel execution → all complete → iteration done
