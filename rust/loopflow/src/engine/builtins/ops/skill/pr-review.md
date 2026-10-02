---
requires: PR or branch diff, source code, and available design context
produces: self-contained HTML walkthrough of the change
---
Create an HTML walkthrough that helps a design collaborator review the code that matters in a PR.

Assume the reader helped design the change and knows its goals, but does not
know the implementation. Name the observable behaviors and show the snippets
that produce them. The reader should finish able to explain what happens,
find the code responsible, and decide what still needs judgment.

## Workflow

1. **Establish the review scope.** Use the requested PR or the current branch's
   PR when available. Read its diff against the actual base, the relevant source
   around changed lines, and available design or discussion. Without a published
   PR, use the local branch comparison and label it. Record the base and head
   revisions; distinguish unpublished working changes from the published diff.
   If a base or source is unavailable, name the gap instead of inventing it.
   Prior design context helps explain intent but is not required to begin.

2. **Choose a reading route.** Lead with the goal and delivered scope, then
   organize stops around observable behavior. Name each stop with what happens:
   “Moving a group keeps its Tasks attached” or “A failed save preserves the
   draft.” Use the reader's product vocabulary. Introduce data models, APIs and
   infrastructure only where they explain that behavior; their structure should
   not determine the narrative. Include unchanged code when it explains the
   connection. Scale the walkthrough to the change; a small PR may need only a
   few stops. For prose or configuration changes, show the instructions or
   declarations that control behavior.

3. **Connect behavior to source.** At each stop, first state what happens in
   plain language. Use a concrete action and result, or a small before/after
   example, when the behavior needs explanation. Then show a short real excerpt
   and explain how its decisive condition produces that result. Keep function
   names and call chains in source captions or supporting detail until the
   reader understands the behavior. Identify the file, symbol,
   revision, and line range. Link to revision-pinned source when a remote exists;
   keep excerpts readable offline. Label local-only excerpts accurately. Preserve
   source text, mark omissions, and distinguish explanatory pseudocode from real
   code. Never let an ellipsis hide the condition being discussed. Use before/after
   excerpts when the change itself is easier to see that way.

   Trace at least one representative action from the reader's starting situation
   to its visible result, with source anchors for the entry point, state change
   and decisive branch. A sequence of function calls alone is not that trace.
   Include the important failure or boundary case. Label hypothetical examples
   and test scenarios so they cannot be mistaken for requested product changes.
   For changes without a direct UI, use the caller's observable behavior. Explain
   state ownership or side effects where they affect the result. A diagram earns
   space when it clarifies the behavior.

4. **Surface review decisions and evidence.** Distinguish accepted intent,
   implementation choices, and unresolved questions. State what this diff changes
   versus behavior already present in the base. When related work supplies a
   mechanism, name that dependency and explain only this change's contribution;
   separate planned work from implemented behavior. Highlight departures from
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
   `lf home screenshot scratch/pr-review.html -o scratch/pr-review.png`). Inspect the
   result at desktop and narrow widths; check code overflow, contrast, navigation,
   and disclosure controls. Fix defects before delivery. If rendering is
   unavailable, report that limit explicitly. Recheck excerpts against the named
   revisions and ensure every material claim has source or evidence behind it.
   Read the headings and explanations without the code: the behavior should
   still make sense. Then check that each central snippet explains that behavior,
   rather than merely showing nearby infrastructure.

## Delivery

Return the HTML file link and a brief description of the reading route. Surface
any material evidence limits. Keep generation local; publishing, PR comments,
approval, and code changes are separate actions. On a repeat invocation, refresh
the same walkthrough for the current scope and remove stale explanations. If
the artifact already matches the requested revisions and contract, reuse it.

A goal summary without meaningful code, an exhaustive file inventory, or a
polished page whose claims cannot be traced to source does not satisfy this skill.
