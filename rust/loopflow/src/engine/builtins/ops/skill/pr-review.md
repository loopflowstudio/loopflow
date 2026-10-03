---
requires: PR or branch diff, source code, and available design context
produces: self-contained HTML walkthrough of user behavior, key data models, and APIs
---
Create a code-centered HTML walkthrough that helps a design collaborator review a PR's behavior, data models, and APIs.

Assume the reader helped design the change and knows its goals, but does not
know the implementation. Show the user's actions, the few concepts representing
them, and the APIs that change their state. Treat implementation as evidence,
not a commitment: expose where the code fits the intended experience and where
a product choice remains. Let real snippets carry the review; use prose to
connect them and explain consequential choices.

## Workflow

1. **Establish the scope.** Use the requested PR or the current branch's PR
   when available. Read its diff against the actual base, relevant surrounding
   source, and available design or discussion. Without a published PR, use the
   local branch comparison and label it. Record base and head revisions;
   distinguish unpublished working changes from the published diff. Name missing
   source or context instead of inventing it. Prior design context is useful,
   not required.

2. **Map the experience to the code.** Open with a short goal and delivered
   scope, then a compact map that fits on one desktop screen:

   | User action and visible result | Concept / key type | State-changing API |
   | --- | --- | --- |
   | A concrete interaction | The record or value representing it | The operation responsible |

   Use actual names and link to the walkthrough's excerpts. Choose the few
   models and APIs that explain the product contract; omit incidental helpers
   and exhaustive file inventories. Show what owns state, what is derived,
   and which distinctions the caller must understand. Make changes from the
   base explicit. Existing mechanisms can supply context without being claimed
   as this PR's work.

3. **Build the review around snippets.** Organize a few stops around the key
   concepts and interactions. Each stop uses this shape:

   - **Behavior:** one sentence or a concrete action → result example.
   - **Model:** a short real type, schema, or declaration showing the relevant
     fields, relationships, or states.
   - **API:** the public signature and decisive implementation branch showing
     how that state changes and what the caller receives.
   - **Review note:** a brief annotation for ownership, a surprising tradeoff,
     a mismatch, or a product decision that the snippets do not explain alone.

   Adapt the shape to the change. Show a shared model once and link back to it;
   do not force a new type or a finding into every stop. For prose or config
   changes, show the instructions or declarations that control behavior. Prefer
   before/after snippets when they reveal a changed contract more clearly.

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

   Give the model and API excerpts the visual emphasis: readable code, compact
   source captions, short annotations, and anchor navigation through the concept
   map. Use side-by-side models or before/after APIs when comparison helps; stack
   them on narrow screens. Keep central code out of disclosure controls and use
   disclosures for secondary implementation and evidence. Support keyboard
   navigation and printing. A diagram earns space only when it clarifies state
   ownership or a transition better than the excerpts.

   Render through an available browser capture tool (in Loopflow,
   `lf screenshot scratch/pr-review.html -o scratch/pr-review.png`). Inspect
   desktop and narrow widths, including the code sections, navigation, contrast,
   overflow, and disclosures. Fix defects before delivery. If rendering is
   unavailable, state the limit. Recheck excerpts against the named revisions.

   Read only the behavior labels, models, and API snippets: can the collaborator
   explain the normal path, failure, and recovery? Then read the annotations:
   does each add a connection, constraint, evidence limit, or decision the code
   cannot communicate alone? Remove the rest.

## Delivery

Return the HTML link and a brief description of the reading route. Surface
material evidence limits. Keep generation local; publishing, PR comments,
approval, and code changes are separate actions. On repeat invocation, refresh
the same walkthrough and remove stale explanations. Reuse it when it already
matches the requested revisions and this contract.

A narrative summary with incidental snippets, a type inventory without user
behavior, or a polished page whose claims cannot be traced to source does not
satisfy this skill.
