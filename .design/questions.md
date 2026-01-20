# Open Questions

Questions encountered during roadmap proposal that need human input.

## Prioritization

1. **Which proposal to build first?**
   - CI/lint/typecheck — foundational, unblocks quality enforcement
   - Maestro MVP — user-facing, demonstrates the vision
   - Loop metrics — enables informed decisions about daemon work

   My recommendation: CI first (tiny effort, big payoff), then Maestro (validates the product direction).

2. **Should these be approved now, or do you want to refine them first?**

## Maestro

3. **Menu bar app or dock app?**
   - Menu bar: always accessible, less intrusive
   - Dock: more visible, feels like a "real" app

4. **Multi-project support in MVP?**
   - Single project is simpler
   - But users often have multiple repos

## Infrastructure

5. **Branch protection on main?**
   - More friction for small changes
   - Prevents accidental bad merges
   - Matches "craft over vibes" philosophy

## Daemon

6. **Token/cost tracking for loops?**
   - Requires parsing API responses or separate metering
   - Useful but complex
   - Defer to post-MVP?
