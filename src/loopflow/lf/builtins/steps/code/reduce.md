---
requires: diff vs main
produces: simpler code
---
Leave the codebase simpler than you found it. Delete what isn't needed. Flatten unnecessary abstractions.

## What to look for

- **Reshape data structures.** Can a different representation eliminate special cases?
- **Rearrange APIs.** Can the interface change so callers don't need conditionals?
- **Delete dead code.** Unused functions, unreachable branches, obsolete options.
- **Collapse duplication.** Same pattern twice? Inline it or pick one location.

## Guardrails

- Stay in scope—if a file wasn't in or used by the diff, don't touch it
- Reshape, don't layer—restructuring is good, adding adapters is not
