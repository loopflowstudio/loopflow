---
requires: diff vs main
produces: simpler code
---
Leave the codebase simpler than you found it. Delete what isn't needed. Flatten unnecessary abstractions.

## Goal

The best reduction isn't deleting a function—it's reshaping a structure so three special cases become one.

Simplicity compounds. Every line removed is a line that can't break, can't confuse, can't slow down the next change.

The bar: could someone reading this code for the first time understand it faster after your changes?

## Workflow

1. **Reflect on the implementation**
   What did building this reveal? Ask:
   - Did the implementation fight the existing structure?
   - Did we add workarounds that hint at a deeper problem?
   - Is there a simpler design we only see now that it's built?

   If yes: this is the reduction opportunity. Don't just clean up around the edges—reshape toward the simpler design you now see.

2. **Review the diff**
   The diff against main is in your context. Identify what was added, what was changed.

3. **Find reduction opportunities**
   For each file touched, ask:
   - What's unused now that this change landed?
   - What abstraction exists only because the old code needed it?
   - What duplication did this change create or reveal?

4. **Reduce**
   Apply changes directly. Prefer reshaping over deleting—a better structure beats surgical removal.

5. **Verify**
   Run tests. If something breaks, the reduction went too far.

## What to reduce

**Reshape data structures.** A different representation can eliminate special cases.

Example: Three optional fields that are mutually exclusive → one enum with three variants.

**Rearrange APIs.** Change the interface so callers don't need conditionals.

Example: `process(item, mode)` where every caller passes the same mode → `process(item)` with mode baked in.

**Delete dead code.** Unused functions, unreachable branches, obsolete options.

Example: A feature flag that's been `true` for six months → delete the flag and the `false` branch.

**Collapse duplication.** Same pattern twice? Inline it or pick one location.

Example: Two functions that differ by one line → one function with a parameter, or inline both if they're only called once.

**Remove backwards-compatibility shims.** Old parameter names, deprecated re-exports, migration code for formats nothing uses anymore.

Example: `def foo(x, old_name=None): x = x or old_name` → just `def foo(x):` if nothing uses `old_name`.

## Scope

**Stay in the diff.** If a file wasn't changed or used by this branch, don't touch it.

**Reshape, don't layer.** Restructuring is good. Adding adapters, wrappers, or compatibility shims is not reducing—it's adding.

**Preserve behavior.** Reduction changes structure, not functionality. If tests break, you changed behavior.

**Be aggressive.** Question whether each abstraction earns its place. Question whether each option is used. Question whether the API surface is minimal. The default is "this can probably go"—make it prove otherwise.

## Output

Simpler code that passes tests. If nothing can be reduced, say so—not every diff has reduction opportunities.
