---
requires: none
produces: scratch/expansion-opportunities.md
---
Where could this codebase go next? What's the natural evolution?

## Goal

Look at what exists and ask: what would multiply the value? Not feature requests or wish lists—structural opportunities that would make the system more capable.

This is strategic thinking, not roadmap planning. Identify directions worth exploring, not tasks to execute.

## Workflow

1. Explore the codebase: what does it do, who uses it, what's the core value
2. Look at how it's being used (README examples, test cases, CLI help)
3. Identify 2-3 expansion directions that extend the core value
4. Write `scratch/expansion-opportunities.md`

## What makes a good expansion

**Multiplicative, not additive.** A new capability that makes existing features more powerful, not just one more thing.

**Follows the grain.** Extensions that feel like natural next steps given the architecture, not bolted-on features that fight the design.

**Opens new uses.** Changes that would let users do things they couldn't before, not just do existing things slightly better.

**Tractable.** Ambitious but achievable. If it requires rearchitecting everything, it's not an expansion—it's a rewrite.

## What to avoid

- Feature parity with other tools (do what this tool does well, not what others do)
- Abstraction layers for hypothetical flexibility
- Integration with every possible service/format
- Backwards compatibility mechanisms

## Output

Write `scratch/expansion-opportunities.md`:

```markdown
# Expansion Opportunities

## Current state
<What the codebase does well, who it's for>

## Direction 1: <name>
**What**: <The capability>
**Why**: <What it unlocks>
**Builds on**: <What existing code/concepts it extends>
**Open questions**: <What would need to be figured out>

## Direction 2: <name>
...

## Not yet
<Ideas that seem promising but aren't ripe—wrong time, missing prerequisites, unclear value>
```

Write for a human who needs to decide what to build next.
