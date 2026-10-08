---
requires: PR or branch diff, source code, and available design context
produces: self-contained HTML walkthrough of user behavior, key data models, and APIs
---
Create a minimalist HTML walkthrough of what changes for users and how the key data models and APIs produce that change, with fewer, larger code excerpts.

Assume the reader helped design the change and knows its goals, but does not
know the implementation. Lead with what changes for users: what they can do,
what they see, and how failure or recovery differs. Then center the few models
and APIs that explain that behavior and matter to their judgment. Show what
changes from the base: fields, relationships, ownership,
callers, state transitions and return contracts. An unchanged type can acquire
a different role when its readers, writers or relationships change. Explain
existing behavior only where needed to understand that shift; do not give a
tour of the current system. Treat implementation as evidence,
not a commitment: expose where the code fits the intended experience and where
a product choice remains. Let real snippets carry the review; use prose to
connect them and explain consequential choices. Spend the page on a few useful
code examples, with restrained typography, generous whitespace, and little decoration.

## Workflow

1. **Establish the scope.** Use the requested PR or the current branch's PR
   when available. Read its diff against the actual base, relevant surrounding
   source, and available design or discussion. Without a published PR, use the
   local branch comparison and label it. Record base and head revisions;
   distinguish unpublished working changes from the published diff. Name missing
   source or context instead of inventing it. Prior design context is useful,
   not required.

2. **Choose the review's center and shape.** Ask what this particular reader
   needs to see to decide whether the change is done right or wrong. Open with
   that consequential user-visible shift and its delivered scope, then show
   the model and API changes that cause it. Be creative about the
   structure: a before/after model comparison, a changed relationship diagram,
   an API contract trace, or a failure-and-recovery story may carry different
   changes best. Choose deliberately for this diff and reader; do not reuse a
   fixed report outline or force novelty where it does not help judgment.

   Select the few key models and APIs, not an inventory. For each, identify
   what is added, removed, directly mutated, or related to differently. Trace
   how API inputs, outputs, effects and failure behavior shift with the model.
   An unchanged signature may now save locally instead of calling a provider;
   that is an API change worth showing. Distinguish preserved behavior from
   changes introduced here and future proposals.

   Use a short map only when it helps orientation. Write concrete actions and
   visible consequences, with links to the excerpts that explain the changes.
   A symbol alone is not an explanation. Omit incidental helpers, repeated
   background and familiar goals that do not help assess the diff.

3. **Choose fewer, larger snippets.** Organize a few stops around the user
   behaviors and the data structures and APIs that produce them. Each stop connects:

   - **Behavior first:** a concrete action and how its visible result changes
     from the base, including consequential failure or recovery differences.
   - **Data structure:** real before/after types, schema or declarations showing
     the changed fields, relationships or ownership. If the type is unchanged,
     show the changed relationship or use rather than repeating its definition.
   - **API:** the old and new contract, with public signatures and decisive
     implementation branches showing changed inputs, effects, results or
     failure behavior. Include the old path or its deletion when that explains
     the change better than the new implementation alone.
   - **Review note:** a brief annotation for ownership, a surprising tradeoff,
     a mismatch, or a product decision that the snippets do not explain alone.

   For CLI behavior, lead with a terminal transcript: the actual command and
   arguments someone types, followed by the relevant output they see. Show the
   important error and recovery command where they explain the contract. Prefer
   captured runs; identify their revision and relevant setup. If a command is
   silent, say so and show the observable result instead of inventing output.
   Mark omissions and redact secrets. Label unexecuted examples as illustrative
   or expected from source, never as observed output. Pair the transcript with
   the data structure and API excerpts that explain its result; a command list
   or test log alone is not the walkthrough.

   These are reading priorities, not four required panels. Adapt to the change.
   Show a shared data structure once and link back to it;
   do not force a new type or a finding into every stop. For prose or config
   changes, show the instructions or declarations that control behavior. Prefer
   before/after snippets when they reveal a changed contract more clearly.

   Prefer one or two substantial excerpts per stop: a complete relevant type,
   function, or coherent branch with enough surrounding code to explain it.
   Combine adjacent fragments from the same symbol. Avoid a gallery of tiny
   snippets that makes the reader reconstruct control flow across captions.
   More lines should supply context, not unrelated implementation detail.

   Keep central snippets visible and larger than their accompanying commentary.
   Usually one or two sentences per annotation suffice. Cut paragraphs that
   paraphrase readable code. Avoid a long introduction, narrative between every
   excerpt, or explanations that bury the model and API below the fold.

   Trace at least one representative action through the entry API, owned state
   change, and visible result. Include the important failure and recovery path:
   what survives, what changes, and how the caller continues. Show the decisive
   conditions, not merely a sequence of function calls. Include unchanged source
   when necessary to complete that trace. Label hypothetical examples and test
   scenarios so they cannot be mistaken for accepted product changes.

   Caption every excerpt with file, symbol, revision, and line range. Link to
   revision-pinned source when a remote exists; keep code readable offline and
   label local-only excerpts. Preserve source text, mark omissions, and label
   explanatory pseudocode separately. Never omit the condition being discussed.

4. **Expose the decisions and proof beside the code.** Distinguish accepted
   intent, implementation choices, and unresolved questions. Check whether the
   models and APIs map to concepts the user needs, whether names reveal their
   behavior, and whether an awkward interaction or unnecessary distinction leaks
   an implementation detail. Surface the most consequential mismatch with its
   source and counterexample; do not turn the walkthrough into a full redesign
   or ask the reader to adjudicate cosmetic refactors.

   Mark planned behavior separately from implemented behavior. Name dependencies
   supplied by related work. Tie evidence to the specific claim it supports;
   distinguish inspected tests, recorded passes, newly run checks, and unverified
   behavior. Keep evidence notes short and put supporting detail in disclosures.
   Passing tests do not prove every goal. Do not manufacture findings, invent
   approval, change product code, or make a merge decision.

5. **Build and inspect the HTML.** Write to the requested path, defaulting to
   `scratch/pr-review.html`. Produce one self-contained file with inline styles
   and any small scripts; no network fonts, CDN libraries, or build step. Escape
   source and diff text so code is displayed without executing it.

   Use a minimalist editorial layout: a short title, quiet navigation, clear
   typography, generous whitespace, and a restrained palette. Avoid decorative
   hero sections, badges, repeated card frames, and dashboard chrome. Give code
   most of the space, with a readable font size and compact source captions.

   Default to one wide reading column so larger excerpts remain readable. Use
   side-by-side excerpts only when direct comparison helps and neither becomes
   cramped; stack them on narrow screens. If using an opening map, let it stack
   into labeled rows on narrow screens rather than squeezing explanations into
   tiny columns. Keep central
   code out of disclosure controls and use disclosures for secondary
   implementation and evidence. Support keyboard navigation and printing.
   A diagram earns space only when it clarifies state
   ownership or a transition better than the excerpts.

   Render through whatever browser capture tool the agent has available. Inspect
   desktop and narrow widths, including the code sections, navigation, contrast,
   overflow, and disclosures. Fix defects before delivery. If rendering is
   unavailable, state the limit. Recheck excerpts against the named revisions.

   Read the opening without following links: does it put this reader's most
   consequential judgment first? Then read only the models and API snippets:
   can the collaborator explain what changed from the base, including changed
   relationships, and the normal path, failure and recovery? Would the page
   still read the same without this diff? If so, replace the inventory with
   evidence of the actual change. Read the annotations:
   does each add a connection, constraint, evidence limit, or decision the code
   cannot communicate alone? Remove the rest. Finally, can adjacent snippets be
   combined, or a visual element removed, without losing meaning? Prefer the
   version with fewer pieces and more readable code.

## Delivery

Return the HTML link and a brief description of the reading route. Surface
material evidence limits. Keep generation local; publishing, PR comments,
approval, and code changes are separate actions. On repeat invocation, refresh
the same walkthrough and remove stale explanations. Reuse it when it already
matches the requested revisions and this contract.

A narrative summary with incidental snippets, a type inventory without user
behavior, or a polished page whose claims cannot be traced to source does not
satisfy this skill.
