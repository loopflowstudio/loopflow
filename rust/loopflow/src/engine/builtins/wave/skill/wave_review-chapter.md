---
requires: one bound Wave and its starting Project/KR ledger in the launch prompt
produces: a Wave chapter report in final Run output
---
Close one Wave's chapter: gather the complete Project ledger and judge whether those bets advanced the Wave mandate, on dated evidence.

A chapter is a dated planning interval, not a new Work kind or a code branch.
This review runs headlessly to completion and never calls `lf ask`. The parent
passes this Wave's exact starting ledger and interval in the launch prompt;
its Task worktree is separate from this Wave's checkout. For a standalone
invocation, use a locally available start record if one exists. Otherwise say
the interval is unknown and perform a baseline review of every currently
visible KR; a baseline cannot call itself complete.

## Boundaries

Review is read-only and does not alter KRs, charters, Tasks, or code — no Wave
charter, Linear object, Work state, Task worktree, branch, product code, or
chapter file changes. Return the complete report to the parent in final
output; the repository Run owns the tracked archive.

## Workflow

1. **Enumerate every Project the Wave held during the chapter.** The starting
   Project/KR ledger in the launch prompt defines the minimum set; union it
   with the current dated PM snapshot (`lf pm show --wave <name> --json`) so
   drift is visible.
   If the full Project set cannot be enumerated, mark coverage incomplete and
   name the missing source; never claim a complete report over a partial
   ledger.
2. **Gather each Project's KR report.** Launch one bounded Run per resolvable
   Project — `lf -b --wave <name> --project <project-id> project/review-chapter
   "<chapter interval and this Project's exact starting KR ledger>"` —
   capture its final report and Run id. Pass the scoped ledger in the launch
   prompt; the Project Run cannot read the parent's private context. A
   recorded Project that no longer resolves (archived, renamed away) is
   accounted for directly from the frozen start ledger and marked with the
   missing live source. A failed child Run stays explicit: its scope is
   incomplete coverage, never silently omitted.
3. **Verify the union.** Check the collected Project reports against the
   Project ledger — expected versus actual report count, every KR row
   accounted for, every child coverage statement carried through. Parent
   aggregation cannot upgrade an incomplete child. Include each complete
   Project report and Run id in final output so the repository parent can
   archive the exact KR rows in its Task worktree.
4. **Judge the mandate.** With the complete Project ledger, judge whether
   those bets advanced the mandate in `wave/<name>/GOAL.md`: which bets paid
   off, which stalled or turned out mis-shaped, what shipped behavior the Wave
   now stands on, what surprised, and what work landed outside every bet.

## Verdict rules

Each KR row keeps one verdict: holds, does not hold, or unknown. Missing,
stale, or ambiguous evidence is unknown — never a silent pass or fail. A
checked box, PR, or implementation receipt alone cannot prove an outcome or
its endurance window; neither can a single successful run.

## Report

Emit this readable Markdown as the final Run output; the repository parent
archives it:

```markdown
# Wave chapter report: <wave> — <interval, or "interval unknown (baseline)">

## Coverage
Coverage: complete | Coverage: incomplete — <missing scope and source>
<Projects reviewed incl. retired/reshaped; expected vs actual reports; child Run ids>

## Project ledger
### <project>
<definition held; coverage; KR verdicts with evidence windows; shipped behavior; surprises; open work; evidence gaps>

## Mandate judgment
<did these bets advance the Wave mandate; what the evidence supports for the next chapter>

## Evidence gaps
<what this Wave should instrument so the next review isn't guessing>
```

## What to avoid

**Reviewing only the surviving Projects.** A bet retired mid-chapter is still
in the start ledger; it gets reviewed from that record, coverage-marked, never
dropped.

**Treating unknown as failure or success.** Unknown means the evidence isn't
there. Record the gap; don't guess the verdict.

**Fixing while reviewing.** Dispositions belong to the next chapter start, not
to this report.
