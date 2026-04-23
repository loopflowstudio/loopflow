---
requires: none
produces: scratch/simplification-opportunities.md
---
Find the highest-leverage simplifications in your area of responsibility.

## Scope

The included context defines what you own. Focus your analysis there. If given `src/cli/`, find simplification opportunities in the CLI—don't audit the entire codebase. Your job is to improve your area, not everything.

## Goal

Identify where architecture and product intent are misaligned. Complexity usually lives in the gap between what the system is built to do and what users actually need. When those two things don't match, code accumulates to bridge the difference—conditionals, adapters, special cases.

The question isn't "what's messy?" but "what would change if we rebuilt around what the product actually wants to be?"

## Workflow

1. Understand the product: README, CLI help, main entry points. What is this for?
2. Explore the architecture: main modules, data structures, key abstractions
3. Look for misalignment:
   - Abstractions that don't map to user concepts
   - Features that fight the architecture
   - Data structures shaped for old requirements
   - Code that translates between what exists and what's needed
4. Identify 2-3 places where realignment would cascade into simplification
5. Write `scratch/simplification-opportunities.md`

## Signs of misalignment

**Adapters and translators.** Code that exists to convert between how data is stored and how it's used. If the conversion is always the same, the storage is wrong.

**Feature-shaped holes.** The same conditional appearing in multiple places because one feature doesn't fit the model.

**Naming drift.** When code uses different words than documentation, CLI, or UI. The words reveal a concept mismatch.

**Config that's always the same.** Options nobody changes suggest the architecture is generic where the product is specific.

**Backwards compatibility for internal code.** Shims between modules that could just be changed together.

## What to skip

- Performance issues (different concern)
- Missing features (that's expand, not reduce)
- Style inconsistencies (that's polish)
- Complexity that matches actual product complexity

## Output

Write `scratch/simplification-opportunities.md`:

```markdown
# Simplification Opportunities

## Product intent
<What this product wants to be, in 2-3 sentences>

## Opportunity 1: <name>
**Misalignment**: <How architecture differs from product intent>
**Symptom**: <Where this shows up in code>
**Realignment**: <What would change to match product intent>
**Cascade**: <What else would get simpler>

## Opportunity 2: <name>
...

## Aligned areas
<Parts where architecture and product match well—patterns to preserve>
```

Focus on realignment opportunities, not cleanup tasks.
