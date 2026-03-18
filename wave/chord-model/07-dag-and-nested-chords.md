# 07: DAG and Nested Chords

**Finish line:** Chord-waves can contain other chord-waves, forming a DAG. Acyclicity is enforced. A VSM chord (five waves, one per system) is representable as a nested structure.

## Context

The redesign chord-wave already proves the first level: a wave can coordinate other waves by pointing its `area` at their directories. The remaining question is how far that model stretches. This item validates nesting and uses the same area-derived membership model to introduce a default top-level chord-wave.

## What to build

1. **DAG validation.** When a wave adds another `wave/<name>/` directory to `area`, treat that as an edge in the chord graph. If the target is itself a chord-wave, walk the implied graph and reject cycles.

2. **Default chord-wave creation.** `lfq init` or equivalent creates a default chord-wave for a repo and seeds its `area` with the existing waves.

3. **Restructuring through tend.** The default chord-wave's first tend cycle reads the existing waves, compares them against the redesign direction, and proposes restructuring for human review instead of requiring manual wave surgery.

4. **Nested UI support.** Concerto can render nested chord-waves so the default chord-wave sits above project chord-waves and leaf waves.

## Done when

- Adding a chord-wave as a member of another chord-wave works
- Cycle detection rejects invalid membership
- A default chord-wave can be created and absorb existing waves
- Its first tend cycle can propose restructuring
- A human can approve or reject the restructuring proposal
