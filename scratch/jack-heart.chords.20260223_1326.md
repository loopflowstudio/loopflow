# Chords

## Vision

The data model becomes explicitly musical. Waves compose into chords. Voices listen to each other. The hierarchy deepens.

**Hierarchy:** Chord > Wave > Flow > Step

## Data model

```rust
enum Wave {
    Voice(WaveData),
    Chord { data: WaveData, waves: Vec<Wave> },
}
```

Key properties:
- A Chord can be used anywhere a Wave can be used
- Recursive: a chord can contain chords (arbitrary nesting)
- A nested chord is opaque to its parent — the parent sees it as one wave, not its internals
- An instantiated wave has at most one parent chord (or none if solo)

### Wave schema vs instantiated wave

- **Wave schema**: abstract template, can be instantiated across many chords
- **Instantiated wave**: concrete, running, at most one parent
- For now, chords contain instantiated waves, not schemas

### Voicing

Voicing is the process of going from wave schema to instantiated wave. The choices made when instantiating: direction, area, model, parameters. Same schema, different voicings = different concrete waves.

Already exists implicitly in fork drafts:
```yaml
- fork:
    step: reduce
    drafts:
      - direction: infra-engineer
      - direction: designer
```
Those drafts are voicings. This names the concept.

## Execution model

Chord execution is fork-like (fork lifted to the wave level), with key differences from today's fork:

1. **Persistence** — fork branches are ephemeral. Chord voices persist across iterations.
2. **Listening** — fork branches are deaf to each other. Chord voices listen.
3. **Stimulus ownership** — the chord owns the trigger, not the children.

### Iteration cycle

1. **Trigger** — stimulus fires (loop/cron/watch/once)
2. **Listen step** — each voice runs a listen step to digest sibling output from previous iteration
3. **Start all** — child waves begin executing in parallel
4. **Wait all** — chord waits for all waves to complete
5. **(Cleanup if applicable)**
6. Back to waiting for next trigger

### Nesting

When a chord contains a child chord, the child is just another wave from the parent's perspective. Parent starts it, waits for it, listens to it. Child manages its own internal waves and listening internally.

### Stimulus ownership

The chord owns the stimulus (loop/cron/watch/once). Child voices don't have independent triggers — they fire when the chord fires.

## Listening

Listening is a **step**, not a context source. Runs at the start of each chord iteration (after the first).

### What the listen step does

1. Gets sibling voices' recent PR(s) injected into context (via a default-off config for full PR content)
2. Reads your own wave roadmap and design docs
3. Adapts plans in response to what siblings did
4. Writes updated wave/scratch docs
5. Then your normal flow runs with the updated plans

The output is **modified plans**, not a summary riding along in every step's context. The adaptation is baked into your artifacts before you start working.

### Why a step, not a context source

- Doesn't pollute the context of every step in the flow
- Discrete moment of reflection: "what did my chord-mates do, how should I adjust?"
- Produces durable artifacts (updated docs) rather than ephemeral context
- The listen step can be as cheap or expensive as needed (config controls PR content depth)

## Rhythm (research, not implementing now)

A rhythm pairs exactly 2 waves with a temporal relationship — tempo ratios like 6:1 or 3:1 where one voice runs more frequently than the other. Key ideas being explored:

- `Rhythm { waves: (Wave, Wave), ratio: (u32, u32) }` as a third Wave variant
- A chord could be composed of nested rhythms: `rhythm(rhythm(a, b), c)` = 3-voice chord
- A 1:1 rhythm and a 2-voice chord would be equivalent base cases
- This could unify chord and rhythm into a single recursive structure
- Killer app: manager pattern where after every 6 engineer commits, a manager voice reviews

**Status:** Theoretically compelling but the alternating execution semantics need more thought. Shipping the straightforward simultaneous chord first, then exploring rhythm as an extension.

## Multi-user (future, not lfd)

- lfd stays simple: one user, one machine, local chords
- lfd-hub (private codebase, first commercial product) orchestrates across lfd instances
- lfd-hub routes updates between machines, handles auth/discovery
- The listen step protocol is the clean seam — lfd-hub can inject remote updates the same way lfd injects local ones
- Monetization: coordination at team scale. Same pattern as git/GitHub.
- "Chords with other people" — baked into the theoretical foundation, not bolted on

## Open questions

- CLI/API surface: how do users create and manage chords?
- Storage: recursive Wave enum in SQLite
- Failure semantics: one voice fails, does the chord continue?
- Cleanup policy between iterations
- First iteration: no listen step (nothing to listen to), or a "hello" step?
