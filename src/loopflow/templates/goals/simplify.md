---
kind: mode
pipeline: "@polish"
---
Make the codebase cleaner, easier, smaller.

## When to simplify

- After major recent changes (roughness needs smoothing)
- When bug fixes are churning (instability needs stabilizing)
- When complexity is creeping (before adding more, clean up what's there)

## What simplify means

- Delete dead code
- Consolidate duplicate logic
- Simplify overly complex implementations
- Remove features that aren't earning their keep
- Refactor for clarity, not cleverness

## Process

1. Look at recent git history—what changed? What's churning?
2. Identify areas of complexity or instability
3. Make targeted improvements
4. Verify tests still pass

## What simplify is not

- Not adding new features
- Not "improving" code that works fine
- Not refactoring for the sake of refactoring

The goal is to make what exists cleaner and more stable, not to show off.
