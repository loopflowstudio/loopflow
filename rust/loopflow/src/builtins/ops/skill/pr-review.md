---
requires: PR or branch diff, source code, and any design context
produces: self-contained HTML walkthrough of user behavior, key data models, and APIs
---
Create a minimalist HTML walkthrough of what changes for users and how the key data models and APIs produce that change, with fewer, larger code excerpts.

The reader helped design the change and knows its goals, but not the
implementation. Lead with what changes for users: what they can do, what they
see, how failure and recovery differ. Then show the few models and APIs that
cause it, and what changes from the base: fields, relationships, who reads and
writes them, state transitions, return values. An unchanged type can take on a
new role when its readers, writers or relationships change. Explain existing
behavior only where the shift needs it; this is not a tour of the system. The
code is the case for the change, not a commitment: show where it fits the
intended experience and where a product choice is still open. Let real
snippets carry the page; prose connects them.

## Workflow

1. **Scope.** Use the requested PR, or the current branch's PR. Read its diff
   against the actual base, the surrounding source, and any design or
   discussion. Without a published PR, compare the local branch and say so.
   Record base and head revisions, and separate uncommitted work from the
   published diff. Name missing source or context; don't fill it in.

2. **Choose the center.** Ask what this reader needs to see to judge whether
   the change is right. Open with that user-visible shift and its scope, then
   the model and API changes that cause it. Pick the shape that fits this
   diff: a before/after model, a changed relationship diagram, an API trace, a
   failure-and-recovery story. No fixed outline, and no novelty for its own
   sake.

   Pick the few key models and APIs, not an inventory. For each, show what is
   added, removed, mutated, or related differently, and how inputs, outputs,
   effects and failures shift with it. An unchanged signature that now saves
   locally instead of calling a provider is an API change worth showing. Keep
   kept behavior, new behavior and future proposals apart. A short opening map
   is fine when it helps; a symbol alone is not an explanation.

3. **Fewer, larger snippets.** Organize a few stops around user behaviors and
   the structures and APIs behind them. Each stop connects:

   - **Behavior first:** a concrete action and how its result changes from the
     base, including failure or recovery.
   - **Data structure:** real before/after types, schema or declarations. For
     an unchanged type, show the changed relationship or use, not its
     definition again.
   - **API:** old and new signatures and the deciding branches: changed inputs,
     effects, results or failures. Show the old path or its deletion when that
     explains the change better.
   - **Review note:** a sentence or two on a tradeoff, a mismatch, or a product
     decision the code doesn't explain alone.

   For CLI behavior, lead with a terminal transcript: the command someone types
   and the output they see, plus the important error and the command that
   recovers from it. Prefer captured runs and name their revision and setup.
   If a command is silent, say so and show what changed instead. Redact
   secrets. Label unexecuted examples as illustrative; never present them as
   real output. Pair the transcript with the excerpts that explain it.

   These are reading priorities, not four required panels. Show a shared type
   once and link back. For prose or config changes, show the text that
   controls behavior. Default to inline unified diffs: red removed, green
   added, with visible −/+ markers. Keep enough context to follow the type or
   branch; one diff beats separate before/after blocks. Generate diffs from the
   pinned revisions and caption both sides with revision and line range.

   Prefer one or two substantial excerpts per stop: a whole type, function, or
   branch. Merge adjacent fragments of the same symbol; a gallery of tiny
   snippets makes the reader rebuild control flow. Keep code larger than the
   commentary and cut paragraphs that paraphrase readable code.

   Trace at least one action through the entry API, the state it changes, and
   the visible result, including the important failure path: what survives,
   what changes, how the caller continues. Show the deciding conditions, not
   just a call sequence. Label hypothetical examples and test scenarios.

   Caption every excerpt with file, symbol, revision and line range. Link to
   pinned source when a remote exists; keep code readable offline. Mark
   omissions, label pseudocode, and never omit the condition being discussed.

4. **Decisions beside the code.** Separate accepted intent, implementation
   choices and open questions. Check whether models and APIs match the
   concepts people use, whether names say what they do, and whether an awkward
   interaction leaks an implementation detail. Raise the biggest mismatch with
   its source and a counterexample; this is not a redesign, and cosmetic
   refactors don't belong.

   Mark planned behavior apart from implemented behavior and name work this
   change depends on. Tie each check to the claim it supports: tests read,
   recorded passes, checks run now, behavior not yet tried. Passing tests don't
   cover every goal. Don't manufacture findings, change product code, or make a
   merge decision.

5. **Build and inspect the HTML.** Write to the requested path, default
   `scratch/pr-review.html`: one self-contained file, inline styles and small
   scripts, no network fonts, CDN libraries or build step. Escape source and
   diff text.

   Minimalist editorial layout: short title, quiet navigation, clear type,
   generous whitespace, restrained palette. No hero sections, badges, card
   frames or dashboard chrome. Give code most of the space. Default to one wide
   column; go side by side only when comparison helps and neither side gets
   cramped, and stack on narrow screens. Keep central code out of disclosures;
   use them for secondary detail. Support keyboard navigation and printing. A
   diagram earns space only when it shows state or a transition better than
   the excerpts.

   Render it in whatever browser tool you have, at desktop and narrow widths,
   and fix overflow, contrast and navigation problems. If you can't render,
   say so. Recheck excerpts against the named revisions.

   Then reread. Does the opening put this reader's biggest judgment first? From
   the snippets alone, can the reader explain what changed, the normal path,
   and failure and recovery? Would the page read the same without this diff?
   Then it's an inventory; replace it. Cut annotations that add nothing the
   code doesn't say. Merge snippets and drop visuals when meaning survives.

## Delivery

Return the HTML path and a short description of the reading route, plus any
checks that limit a claim. Keep it local: publishing, PR comments, approval and
code changes are separate actions. On a repeat run, refresh the same page and
remove stale explanation; reuse it as is when it already matches the revisions.

A narrative with incidental snippets, a type inventory without user behavior,
or a polished page whose claims don't trace to source doesn't count.
