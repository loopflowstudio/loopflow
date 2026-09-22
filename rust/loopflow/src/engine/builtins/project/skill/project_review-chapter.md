---
requires: one bound Project and its starting KR ledger in the launch prompt
produces: a Project chapter report in final Run output
---
Close one Project's chapter: give every KR in the chapter ledger exactly one verdict backed by dated evidence.

A chapter is a dated planning interval, not a new Work kind or a code branch.
This review runs headlessly to completion and never calls `lf ask`. The parent
passes this Project's exact starting KR ledger and interval in the launch
prompt; its Task worktree is separate from this Project's checkout. For a
standalone invocation, use a locally available start record if one exists.
Otherwise say the interval is unknown and perform a baseline review of every
currently visible KR; a baseline cannot call itself complete.

## Boundaries

Review is read-only and does not alter KRs, charters, Tasks, or code — no
Linear object, Work state, Task worktree, branch, product code, or chapter file
changes. Return the complete report to the parent in final output; the
repository Run owns the tracked archive.

## Workflow

1. **Enumerate the KR rows.** The starting ledger in the launch prompt defines
   the minimum KR set: one row per recorded KR. Add one row for any current KR
   (`lf pm show --wave <name> --project <slug> --json`) absent from that ledger so
   drift is visible. Report the expected and actual KR row counts. If the
   full KR set cannot be enumerated, mark coverage incomplete and name the
   missing source; never claim a complete report over a partial KR set. A
   failed or stale PM refresh is an evidence gap, not an empty KR list.
2. **Judge each KR row.** Record the **exact claim**, verbatim as the ledger
   held it; the **observed evidence and its time window** — dated references
   from `lf status`, `lf roadmap`, metric contracts and their live readings,
   git history, CI results, Run records, and shipped behavior exercised
   through the real configured path; and exactly one verdict: holds, does not
   hold, or unknown. Missing, stale, or ambiguous evidence is unknown — never
   a silent pass or fail.

   A checked box, PR, or implementation receipt alone cannot prove an outcome
   or its endurance window; neither can a single successful run. A KR
   claiming duration ("for a week straight") is unknown unless that window
   itself was observed.
3. **Record the rest of the chapter.** Shipped behavior, surprises, open work
   (Tasks still in flight, with their state — untouched by this review), and
   the evidence gaps the next chapter should close.

## Report

Emit this readable Markdown as the final Run output; the repository parent
archives it:

```markdown
# Project chapter report: <project> — <interval, or "interval unknown (baseline)">

## Coverage
Coverage: complete | Coverage: incomplete — <missing scope and source>
Expected KR rows: <n from ledger> · Actual KR rows: <m reported, incl. drift rows>

## KR ledger
### <KR claim, verbatim>
**Verdict**: holds | does not hold | unknown
**Evidence**: <what was observed, where, over what dated time window>

## Shipped behavior
<what a user or agent can now do that they couldn't at chapter start>

## Surprises
<what the chapter didn't predict>

## Open work
<in-flight Tasks and their state — untouched by this review>

## Evidence gaps
<KRs that stayed unknown and what would have proven them>
```

## What to avoid

**Receipts as proof.** "The PR merged" answers what shipped, not whether the
claimed behavior holds now or held for its window.

**Sampling.** Judging three memorable KRs and skipping the awkward ones. Every
ledger row gets a verdict; the row counts make a silent skip visible.

**Fixing while reviewing.** A KR that should be rewritten is a finding for the
next chapter start, not an edit here.
