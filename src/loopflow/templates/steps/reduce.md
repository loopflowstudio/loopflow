---
requires: diff vs main
produces: simpler code
---
Simplify code touched by this branch while preserving user behavior.

## Goal

Less code that does the same thing. Delete what isn't needed. Flatten unnecessary abstractions. The bar is: if a user ran the same workflows before and after, they wouldn't notice a difference—except maybe things are faster or error messages are clearer.

## What to simplify

Focus only on code this branch modified.

**Dead code.** Unused functions, unreachable branches, commented-out code.

**Over-abstraction.** Layers that don't earn their keep. Inheritance that could be composition. Generics that are only ever used with one type.

**Duplication.** Copy-pasted logic that could be a function. But don't create abstractions for two similar lines.

## What to preserve

**User-visible behavior.** Same inputs, same outputs. Same error messages. Same side effects.

**Performance characteristics.** Don't make things slower to make them prettier.

**Test coverage.** Tests should still pass. If a test fails, the simplification went too far.
