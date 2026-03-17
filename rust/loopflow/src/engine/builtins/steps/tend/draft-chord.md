---
requires: scratch/tend-assessment.md
produces: scratch/tend-chord.md
---
Compose a chord — a coordinated set of mutations across member waves.

## Goal

The assessment identified pressure points. This step turns them into concrete,
executable changes. A chord is multiple notes sounding together — each mutation
is a note, the chord is the composition.

Mutations should be the minimum intervention that addresses the pressure. Don't
reorganize what's working. Don't optimize what isn't the bottleneck.

Waves exist in a market for the user's attention. The chord's job is to focus
that attention on the work that matters most. In a large chord with many member
waves, most waves should be silent at any given time. Silence shrinks the
blocking queue — fewer waves competing for review means faster throughput on
the waves that are actually building. A chord that keeps all waves active
simultaneously is a chord that drowns the user.

## Mutation Levers

Each mutation pulls one of these levers on a member wave:

| Lever | When to pull | What changes |
|-------|-------------|--------------|
| **Direction** | Wave optimizing for the wrong thing | `direction:` in wave YAML |
| **Area** | Scope too broad or too narrow | `area:` in wave YAML |
| **Flow** | Process wrong for current phase | `flow:` in wave YAML |
| **Items** | Priorities stale, items missing or redundant | Item files in `wave/<name>/` |
| **Agent** | Wrong model for current work | `agent:` in wave YAML |
| **Step agents** | Different steps need different models | `step_agents:` in wave YAML |
| **Triggers** | Wrong frequency or trigger sources | `triggers:` in wave YAML |
| **Lifecycle** | Wave should pause, split, or combine | Create/delete/restructure wave dirs |
| **Rhythm** | Wrong execution pattern for current phase | `rhythm:` and `beats:` in chord YAML |
| **Silence** | Nothing compelling to build | Remove all items, wave watches its area |
| **Wake** | Something compelling emerged in a silent wave's area | Add items to a silent wave |

## Workflow

1. **Read the assessment.** `scratch/tend-assessment.md` is your primary input.
   The pressure points are your starting constraints.

2. **Draft mutations.** For each pressure point, compose one or more mutations.
   Each mutation must specify:
   - Which wave it targets
   - Which lever it pulls
   - The concrete before/after change
   - Why this addresses the pressure point
   - What could go wrong

3. **Check coherence.** Read the mutations as a set:
   - Do they conflict with each other?
   - Do they create new dependencies or ordering constraints?
   - Is the combined effect what you intend, or do interactions produce surprises?
   - Could fewer mutations achieve the same effect?

4. **Compose the chord.** Order mutations by priority. The human reviewing this
   should be able to approve the top mutations and defer the rest without
   breaking anything.

## Output

Write `scratch/tend-chord.md`:

```markdown
# Chord — <date>

## Context
<1-2 sentences linking to the assessment's key findings>

## Mutations

### 1. <title>
**Wave**: <name>
**Lever**: <direction | area | flow | items | agent | triggers | lifecycle>
**Before**: <current state>
**After**: <proposed state>
**Rationale**: <why this addresses a pressure point>
**Risk**: <what could go wrong>

### 2. <title>
...

## Coherence
<How these mutations interact. Dependencies, ordering, combined effect.>

## Deferred
<Observations from the assessment that don't warrant mutation yet, and why.>
```

## What to avoid

**Over-mutation.** A chord with 8 mutations is noise. If everything needs changing,
the problem is upstream — rethink the wave structure, don't patch it.

**Waking too many waves.** Silence is the default healthy state for most waves
in a large chord. Only wake a wave when the work is genuinely compelling —
a clear user experience improvement, not just "we could do this." Prioritize
validating existing experiences over building new ones.

**Vague mutations.** "Adjust the direction" is not a mutation. "Change direction
from `[ux]` to `[ux, clarity]`" is.

**Coupling.** Each mutation should be independently valuable. If mutation 2 only
makes sense after mutation 1, make that dependency explicit — or combine them.
