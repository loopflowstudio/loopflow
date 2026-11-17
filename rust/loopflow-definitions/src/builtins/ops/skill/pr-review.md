---
requires: PR or branch diff, source code, and any design context
produces: self-contained HTML walkthrough of user behavior, key data models, and APIs
---
Create a minimalist HTML walkthrough of what changes for users and how the key data models and APIs produce that change, with fewer, larger code excerpts.

The reader helped design the change and knows its goals, but not the
implementation. Lead with what changes for users: what they can do, what they
see, how failure and recovery differ. Then show the few models and APIs that
cause it. Real code carries the page; prose connects it. The code is the case
for the change, not a commitment: show where it fits the intended experience
and where a product choice is still open.

## Method

1. **Scope.** Use the requested PR, or this branch's. Read its diff against
   the actual base, the surrounding source and any design. Record base and head
   revisions and keep uncommitted work apart from the published diff. Name
   missing source or context; don't fill it in.

2. **Choose the center.** What does this reader need to see to judge whether
   the change is right? Open with that shift, then the model and API changes
   behind it. Pick the shape that fits this diff: a before and after model, a
   changed relationship, an API trace, a failure-and-recovery story. No fixed
   outline. Pick the few models and APIs that matter, not an inventory. An
   unchanged signature that now saves locally instead of calling a provider is
   an API change worth showing.

3. **Fewer, larger snippets.** Build a few stops, each a user behavior and the
   structure behind it: the concrete action and how its result changes; the
   real before and after types; the old and new signatures and the branches
   that decide; a sentence or two on a tradeoff the code doesn't explain. For
   a CLI, lead with a transcript of the command and its output, plus the
   important error and the command that recovers from it. Prefer captured
   runs; label anything you didn't run as illustrative.

   Default to inline unified diffs generated from the pinned revisions. One or
   two substantial excerpts per stop, a whole type, function or branch; a
   gallery of fragments makes the reader rebuild control flow. Caption each
   with file, symbol, revision and line range. Keep code larger than the
   commentary, and cut any paragraph that paraphrases readable code.

   Trace at least one action from the entry API through the state it changes
   to the visible result, including the failure path: what survives, what
   changes, how the caller continues.

4. **Decisions beside the code.** Keep intent people stated, implementation
   choices and open questions apart. Do the models and APIs match the concepts
   people use? Do names say what they do? Raise the biggest mismatch with its
   source and a counterexample. Mark planned behavior apart from implemented.
   Tie each check to the claim it supports. Don't manufacture findings, change
   product code or make a merge decision.

5. **Build and look at it.** Write `scratch/pr-review.html` unless another
   path was asked for: one self-contained file, inline styles, no network.
   Editorial layout: short title, quiet navigation, generous whitespace, one
   wide column, code given most of the space. No hero sections, badges or
   dashboard chrome. Render it in a browser at desktop and narrow widths and
   fix what's broken; if you can't render it, say so.

   Then reread. Does the opening put this reader's biggest judgment first? From
   the snippets alone, can they explain what changed, the normal path, and
   failure and recovery? Would the page read the same without this diff? Then
   it's an inventory; replace it.

Return the path and a short description of the reading route. Keep it local:
publishing, PR comments and approval are separate acts. On a repeat run,
refresh the same page.
