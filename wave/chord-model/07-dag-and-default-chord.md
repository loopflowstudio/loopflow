# 07: DAG Enforcement and Default Chord

**Finish line:** Chords can contain other chords (forming a DAG). Acyclicity is enforced. The default chord concept is validated and ready to absorb existing waves.

## Context

The redesign chord coordinates four waves. But what about the existing five waves (foundation, trust, context, concerto, scale)? And what about Cadenza as a second project?

Two concepts mature here:

1. **Chord DAG.** Chords containing chords. The `is_default` chord at the top. A project chord (redesign) as a member. Acyclicity validation prevents loops.

2. **Default chord.** Every project gets one. It's the thing that tends all waves. When the redesign chord has proven tend works, the default chord absorbs the existing five waves and restructures them through tend cycles — not manual reshuffling.

## What to build

1. **DAG validation.** On `add_chord_member`, if the new member is a chord, walk the membership graph and reject cycles. Store chord-contains-chord relationships in the same membership table.

2. **Default chord creation.** `lfq init` (or equivalent) creates the default chord for a repo. All existing waves automatically become members. The default chord's tend flow runs against everything.

3. **Restructuring through tend.** The default chord's first tend cycle reads all existing waves, compares against the redesign doc, and proposes restructuring. The human reviews. This is the migration path — no manual wave surgery.

4. **Chord nesting in UI.** Concerto shows nested chords. The default chord at top, project chords inside, waves at the leaves.

## Done when

- Adding a chord as a member of another chord works
- Cycle detection rejects invalid membership
- Default chord can be created and absorbs existing waves
- The default chord's first tend cycle proposes restructuring
- Human can approve/reject the restructuring proposal
