# 07: DAG Enforcement and Default Chord

**Finish line:** Chord-waves can contain other chord-waves (forming a DAG). Acyclicity is enforced. The default chord-wave concept is validated and ready to absorb existing waves.

## Context

The redesign chord-wave coordinates four waves. But what about the existing five waves (foundation, trust, context, concerto, scale)? And what about Cadenza as a second project?

Two concepts mature here:

1. **Chord DAG.** Chord-waves containing chord-waves. The default chord-wave at the top. A project chord-wave (redesign) as a member. Acyclicity validation prevents loops.

2. **Default chord-wave.** Every project gets one. It's the thing that tends all waves. When the redesign chord-wave has proven tend works, the default chord-wave absorbs the existing five waves and restructures them through tend cycles — not manual reshuffling.

## What to build

1. **DAG validation.** When a wave's `area` adds another `wave/<name>/` directory, treat that as a membership edge. If the target is itself a chord-wave, walk the implied graph and reject cycles. Store only waves; membership is derived from area.

2. **Default chord-wave creation.** `lfq init` (or equivalent) creates the default chord-wave for a repo. All existing waves automatically become members by populating its area. The default chord-wave's tend flow runs against everything.

3. **Restructuring through tend.** The default chord-wave's first tend cycle reads all existing waves, compares against the redesign doc, and proposes restructuring. The human reviews. This is the migration path — no manual wave surgery.

4. **Chord nesting in UI.** Concerto shows nested chord-waves. The default chord-wave at top, project chord-waves inside, waves at the leaves.

## Done when

- Adding a chord-wave as a member of another chord-wave works
- Cycle detection rejects invalid membership
- Default chord-wave can be created and absorbs existing waves
- The default chord-wave's first tend cycle proposes restructuring
- Human can approve/reject the restructuring proposal
