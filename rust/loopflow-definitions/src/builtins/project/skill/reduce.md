---
requires: none
produces: scratch/simplification-opportunities.md and an unstarted Task per opportunity
---
Find the highest-leverage simplifications in the area you are given.

Read `scratch/` first. Your area is whatever the request names: a
subsystem, a surface, a directory, recent work. In a Wave with no request, it
is what the Wave's goal says it owns. Otherwise it is the whole codebase.

"We did a lot of work on accounts lately" names an area, not a diff. Find the
PRs that did the work, see which part of the product or infrastructure they
kept touching, and widen to the next cohesive whole. Work on that whole: the
PRs may have been patches on a larger problem. Open the output by naming the
area you took.

Complexity usually lives in the gap between what the system is built to do and
what the product needs. Code piles up to bridge it: conditionals, adapters,
special cases. Don't ask "what's messy?" Ask "what would change if we rebuilt
around what the product wants to be?"

## Workflow

Start at the core and radiate outward. The core is the few data structures and
APIs everything else is built on, in the product and in its infrastructure.

1. **Say what the product values.** From the README, docs, positioning and
   recent decisions, write the handful of behaviors this product exists for,
   and what is questionable even though it works. Everything after is judged
   against this list, so show it to the person first when one is here, with
   the few questions only they can answer: which of these do you use?
2. **Read the core.** The records, the models, the public APIs. Are they as
   simple and clear as they could be, and do they say what the product wants
   to be? A core that is slightly wrong is paid for in every layer above it.
3. **Follow it outward.** Where the layers above bend around the core (signs
   below), the fix is usually in the core, not the layer.
4. Pick 2–3 places where realigning would cascade into simpler code. Each is
   one big piece of work, not a list of small ones.
5. Write `scratch/simplification-opportunities.md`.

"Is this called?" is too weak a test. Code can be called, tested and
documented and still produce behavior nobody cares about. The test is whether
it serves something on the valued list.

A reduction ends smaller. Say what each opportunity removes, roughly how much,
and what breaks if it goes. Removing a special case by adding a second
mechanism is growth.

This is the pass for getting the shape right. Once it is, `deep-clean` goes through
what remains function by function. In that order: cleaning code a redesign
will replace is wasted.

## Signs of a mismatch

**Two ways of doing one job.** Two transports, two command paths, two places
that rebuild the same result. One should lose.

**Adapters and translators.** If data is always converted the same way between
storage and use, the storage is wrong.

**Feature-shaped holes.** The same conditional in many places because one feature
doesn't fit the model.

**Naming drift.** Code uses different words than the docs, CLI or UI. The words
reveal a concept mismatch.

**Config that never changes.** Options nobody sets mean the architecture is
generic where the product is specific. A parameter every caller leaves alone
is a behavior, not a parameter.

**Compatibility for nobody.** Shims between modules that could change
together, readers for data nobody has, deprecations with one user.

## Skip

- Performance (a different concern)
- Missing features that don't simplify what exists
- Style inconsistencies (that's polish)
- Complexity that matches real product complexity

## Output

```markdown
# Simplification Opportunities

## Area
<What you looked at, and why that>

## What the product values
<The behaviors it exists for, and what is questionable even though it works>

## Opportunity 1: <name>
**Misalignment**: <How the architecture differs from product intent>
**Symptom**: <Where this shows up in code>
**Realignment**: <What would change to match product intent>
**Removes**: <What goes, roughly how much, and what breaks if it does>
**Cascade**: <What else would get simpler>

## Opportunity 2: <name>
...

## Aligned areas
<Where architecture and product match — patterns to keep>
```

Realignments, not cleanup tasks.

Then file each opportunity as a Task, following `lf help capture-tasks`: the
misalignment, the symptom, what it removes and the cascade are its brief, and
its "Done when" says the code ends smaller. Two or three Tasks
that each change the shape of something; never a list of small ones. They wait
unstarted until someone picks them.
