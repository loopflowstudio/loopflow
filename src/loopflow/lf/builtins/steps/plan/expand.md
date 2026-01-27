---
requires: none
produces: scratch/expansion-opportunities.md
---
Find the highest-leverage expansions in your area of responsibility.

## Scope

The included context defines what you own. Focus your analysis there. If given `src/cli/`, find expansion opportunities in the CLI—don't propose features for unrelated areas. Your job is to identify what this area could become, not what everything could become.

## Goal

Identify where the system has latent capabilities waiting to be unlocked. Expansion opportunities live in the gap between what the system can do and what it could easily do. When infrastructure exists but isn't fully utilized, when patterns are established but not extended, when adjacent features become trivial—that's where expansion has leverage.

The question isn't "what features would be nice?" but "what capabilities are one step away from what already exists?"

## Workflow

1. Understand what exists: README, main entry points, core abstractions
2. Map the infrastructure: what's built that could support more?
3. Look for latent capabilities:
   - Patterns established in one place but not extended elsewhere
   - Infrastructure that handles 80% of a larger problem
   - APIs that could serve additional use cases
   - Data that's collected but underutilized
4. Identify 2-3 expansions where existing work makes the next step easy
5. Write `scratch/expansion-opportunities.md`

## Signs of expansion opportunity

**Partial patterns.** A convention exists in some places but not others. Extending it is easier than inventing something new.

**Underutilized infrastructure.** Code handles a general case but only one specific case uses it. The generality is already paid for.

**Adjacent features.** The system does X, and Y is trivially close. Auth exists—password reset is nearby. Caching exists—cache invalidation UI is nearby.

**Collected but unused.** Data gathered for one purpose could serve another. Logs that could become analytics. Errors that could become diagnostics.

**User requests.** What do users ask for that's surprisingly close to what exists?

## What to skip

- Features requiring new infrastructure (that's a larger project)
- Expansions that fight the architecture (that's reduce territory)
- Polish and quality improvements (that's polish territory)
- Speculative features without clear demand

## Output

Write `scratch/expansion-opportunities.md`:

```markdown
# Expansion Opportunities

## Current capabilities

<What this area does well. The foundation expansions build on.>

## Opportunity 1: <name>

**Latent capability**: <What existing infrastructure enables>
**Gap**: <What's missing to unlock it>
**Expansion**: <What becomes possible>
**Leverage**: <Why this is easier than it looks>

## Opportunity 2: <name>
...

## Not yet ready

<Expansions that would be valuable but require more foundation first>
```

Focus on leverage—expansions where existing work makes the next step easy.
