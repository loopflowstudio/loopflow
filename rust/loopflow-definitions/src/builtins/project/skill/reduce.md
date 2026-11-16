---
requires: none
produces: scratch/simplification-opportunities.md
---
Find the highest-leverage simplifications in your area of responsibility.

Read `scratch/` first. The included context is your area: given `src/cli/`,
simplify the CLI, not the whole codebase.

Complexity usually lives in the gap between what the system is built to do and
what the product needs. Code piles up to bridge it: conditionals, adapters,
special cases. Don't ask "what's messy?" Ask "what would change if we rebuilt
around what the product wants to be?"

## Workflow

1. Learn the product from the README, CLI help and main entry points. What is it for?
2. Read the architecture: main modules, data structures, key abstractions.
3. Look for mismatches:
   - Abstractions that don't map to concepts people use
   - Features that fight the architecture
   - Data structures shaped for old requirements
   - Code that translates between what exists and what's needed
4. Pick 2–3 places where realigning would cascade into simpler code.
5. Write `scratch/simplification-opportunities.md`.

## Signs of a mismatch

**Adapters and translators.** If data is always converted the same way between
storage and use, the storage is wrong.

**Feature-shaped holes.** The same conditional in many places because one feature
doesn't fit the model.

**Naming drift.** Code uses different words than the docs, CLI or UI. The words
reveal a concept mismatch.

**Config that never changes.** Options nobody sets mean the architecture is
generic where the product is specific.

**Compatibility shims for internal code.** Shims between modules that could just
change together.

## Skip

- Performance (a different concern)
- Missing features that don't simplify what exists
- Style inconsistencies (that's polish)
- Complexity that matches real product complexity

## Output

```markdown
# Simplification Opportunities

## Product intent
<What this product wants to be, in 2-3 sentences>

## Opportunity 1: <name>
**Misalignment**: <How the architecture differs from product intent>
**Symptom**: <Where this shows up in code>
**Realignment**: <What would change to match product intent>
**Cascade**: <What else would get simpler>

## Opportunity 2: <name>
...

## Aligned areas
<Where architecture and product match — patterns to keep>
```

Realignments, not cleanup tasks.
