# Chords

Simultaneous multi-wave execution with inter-voice listening. The data model becomes explicitly musical — waves compose into chords, voices listen to each other, the hierarchy deepens.

**Hierarchy:** Chord > Wave > Flow > Step

## North Star

A chord fires its stimulus, all child voices execute in parallel, each voice listens to what siblings did last iteration before starting its own work. Persistence across iterations and inter-voice awareness are the key differentiators from today's fork.

## Design Decisions

**Chords are waves.** A Chord can be used anywhere a Wave can be used. The recursive `Wave` enum (Voice | Chord) means a chord can contain chords — arbitrary nesting, opaque to the parent.

**Voicing over configuration.** Going from wave schema to instantiated wave is "voicing" — the choices of direction, area, model, parameters. Same schema, different voicings = different concrete waves. Already exists implicitly in fork drafts; this names the concept.

**Stimulus ownership at the chord level.** The chord owns the trigger (loop/cron/watch/once). Child voices don't have independent triggers — they fire when the chord fires.

**Listening is a step, not a context source.** Runs at the start of each chord iteration (after the first). Produces modified plans as durable artifacts, not a summary riding along in every step's context.

**Instantiated waves, not schemas.** For now, chords contain concrete instantiated waves, not abstract templates. An instantiated wave has at most one parent chord (or none if solo).

## Data Model

```rust
enum Wave {
    Voice(WaveData),
    Chord { data: WaveData, waves: Vec<Wave> },
}
```

Key properties:
- Recursive: a chord can contain chords (arbitrary nesting)
- A nested chord is opaque to its parent — the parent sees it as one wave, not its internals
- An instantiated wave has at most one parent chord (or none if solo)

## Phases

| # | Phase | Focus | Status |
|---|-------|-------|--------|
| 01 | Data Model | Wave enum (Voice/Chord), SQLite storage, voicing | |
| 02 | Execution | Chord iteration cycle: trigger → start all → wait all | |
| 03 | Listen Step | Inter-voice communication via PR digestion and plan adaptation | |

## Future Directions

### Rhythm (research)

A rhythm pairs exactly 2 waves with a temporal relationship — tempo ratios like 6:1 or 3:1 where one voice runs more frequently than the other.

- `Rhythm { waves: (Wave, Wave), ratio: (u32, u32) }` as a third Wave variant
- Could unify chord and rhythm into a single recursive structure
- Killer app: manager pattern where after every 6 engineer commits, a manager voice reviews

Theoretically compelling but alternating execution semantics need more thought. Ship simultaneous chord first.

### Multi-user (lfd-hub)

- lfd stays simple: one user, one machine, local chords
- lfd-hub (private codebase) orchestrates across lfd instances
- The listen step protocol is the clean seam — lfd-hub can inject remote updates the same way lfd injects local ones
- "Chords with other people" — baked into the theoretical foundation, not bolted on

## Open Questions

- CLI/API surface: how do users create and manage chords?
- Failure semantics: one voice fails, does the chord continue?
- Cleanup policy between iterations
- First iteration: no listen step (nothing to listen to), or a "hello" step?

## Done When (wave complete)

- Wave enum supports Voice and Chord variants in storage and API
- Chords execute child waves in parallel with stimulus ownership
- Listen step runs at iteration start, producing adapted plans from sibling output
- Nesting works: a chord containing a chord behaves correctly
