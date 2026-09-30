---
requires: PR or branch diff, source code, and available design context
produces: self-contained HTML walkthrough of the change
---
Create an HTML walkthrough that helps a design collaborator review the code that matters in a PR.

Assume the reader helped design the change and knows its goals. Restate those
goals clearly, then spend their attention on how the implementation realizes
them, where it differs, and what still needs judgment. The reader should finish
able to explain the core model, follow the important behavior into code, and
decide what deserves a closer look.

## Workflow

1. **Establish the review scope.** Use the requested PR or the current branch's
   PR when available. Read its diff against the actual base, the relevant source
   around changed lines, and available design or discussion. Without a published
   PR, use the local branch comparison and label it. Record the base and head
   revisions; distinguish unpublished working changes from the published diff.
   If a base or source is unavailable, name the gap instead of inventing it.
   Prior design context helps explain intent but is not required to begin.

2. **Choose a reading route.** Organize by the concepts and behaviors the reader
   needs to understand, not file order. Lead with the goal and delivered scope.
   Select the core data models, public APIs, ownership or state boundaries, and
   the implementation points that decide important user behavior. Include
   unchanged code when it explains the connection. Skip routine plumbing unless
   it carries a consequential constraint. Scale the walkthrough to the change;
   a small PR may need only a few stops. For prose or configuration changes,
   show the actual instructions or declarations that control behavior rather
   than inventing models and APIs.

3. **Connect intent to source.** At each stop, explain the design choice, show
   a short real excerpt, and explain its consequence. Identify the file, symbol,
   revision, and line range. Link to revision-pinned source when a remote exists;
   keep excerpts readable offline. Label local-only excerpts accurately. Preserve
   source text, mark omissions, and distinguish explanatory pseudocode from real
   code. Never let an ellipsis hide the condition being discussed. Use before/after
   excerpts when the change itself is easier to see that way.

   Trace at least one representative user action through the entry point, model
   or state changes, decisive branch, and visible result. Include the important
   failure or boundary case. For changes without a direct UI, use the caller's
   observable behavior. Explain who owns durable state or side effects where
   relevant. A diagram earns space when it clarifies these relationships.

4. **Surface review decisions and evidence.** Distinguish accepted intent,
   implementation choices, and unresolved questions. Highlight departures from
   the design, surprising tradeoffs, and plausible failure modes with their source
   anchors. Tie tests or observed behavior to the claims they support; distinguish
   inspected tests, recorded passes, newly run checks, and unverified behavior.
   If source contradicts the design, explain the discrepancy. Do not manufacture
   findings to fill a section, imply that passing tests proves every goal, or
   turn the walkthrough into an unsolicited rewrite or merge decision.

5. **Build and inspect the HTML.** Write to the requested path, defaulting to
   `scratch/pr-review.html`. Produce one self-contained file with inline styles
   and any small scripts; no network fonts, CDN libraries, or build step. Escape
   source and diff text so opening the document displays code without executing
   it. Use a deliberate editorial layout: clear typography, a short reading route
   with anchor navigation, concise explanations beside readable code, and generous
   spacing. Keep the central excerpts visible; use native disclosure controls for
   supporting detail. Support narrow screens, keyboard navigation, and printing.
   Do not replace the walkthrough with a dashboard of counts or a wall of diff.

   Render through an available browser capture tool (in Loopflow,
   `lf screenshot scratch/pr-review.html -o scratch/pr-review.png`). Inspect the
   result at desktop and narrow widths; check code overflow, contrast, navigation,
   and disclosure controls. Fix defects before delivery. If rendering is
   unavailable, report that limit explicitly. Recheck excerpts against the named
   revisions and ensure every material claim has source or evidence behind it.

## Delivery

Return the HTML file link and a brief description of the reading route. Surface
any material evidence limits. Keep generation local; publishing, PR comments,
approval, and code changes are separate actions. On a repeat invocation, refresh
the same walkthrough for the current scope and remove stale explanations. If
the artifact already matches the requested revisions and contract, reuse it.

A goal summary without meaningful code, an exhaustive file inventory, or a
polished page whose claims cannot be traced to source does not satisfy this skill.
