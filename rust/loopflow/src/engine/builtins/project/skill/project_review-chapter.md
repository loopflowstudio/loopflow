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
Otherwise say the interval is unknown and reconstruct the Project's KR-era
baseline. Review current, closed, removed, and rewritten KRs from their first
supported observation through closure or now. Pre-KR repository history is
context, not retroactive promise evidence. Missing lineage makes historical
coverage incomplete even when every current KR is present; a baseline cannot
call itself complete.

## Boundaries

Review is read-only and does not alter KRs, charters, Tasks, or code — no
Linear object, Work state, Task worktree, branch, product code, or chapter file
changes. Return the complete report to the parent in final output; the
repository Run owns the tracked archive.

## Workflow

1. **Enumerate the KR rows.** The starting ledger in the launch prompt defines
   the minimum KR set: one row per recorded KR. Add one row for any current KR
   (`lf pm show --wave <name> --project <slug> --json`) absent from that ledger so
   drift is visible. For a baseline, also add every closed, removed, or earlier
   wording preserved in dated PM snapshots or revisions, prior Run context,
   archived records, or repository history after KRs began. Record each row's
   exact claim, status (`current`, `closed`, or `replaced`), and supported
   `established`/first-observed and `closed`/last-observed dates. Unknown dates
   stay unknown; do not infer them from unrelated pre-KR commits. Report the
   expected and actual KR row counts. If the
   full KR set cannot be enumerated, mark coverage incomplete and name the
   missing source; never claim a complete report over a partial KR set. A
   failed or stale PM refresh is an evidence gap, not an empty KR list.
2. **Judge each KR row.** Record the **exact claim**, verbatim as the ledger
   held it; its **active interval** — exact dates when known, otherwise honest
   first/last-observed bounds; the **observed evidence and its time window** — dated references
   from `lf status`, `lf roadmap`, metric contracts and their live readings,
   git history, CI results, Run records, and shipped behavior exercised
   through the real configured path; and exactly one verdict: holds, does not
   hold, or unknown. Missing, stale, or ambiguous evidence is unknown — never
   a silent pass or fail.

   A checked box, PR, or implementation receipt alone cannot prove an outcome
   or its endurance window; neither can a single successful run. Exercise the
   real configured path when it is available; a clean archive, fixture, or
   isolated substitute does not erase a counterexample from the ordinary path.

   For duration, universal, and conjunctive claims:

   - `holds` requires the complete denominator and window named by the claim;
   - `does not hold` requires one dated counterexample inside that scope, even
     when the rest of the window is missing;
   - `unknown` means neither complete proof nor a valid counterexample exists.

   Judge the Project definition separately with the same three verdicts. It is
   useful Wave evidence but is not a KR and must not be added to KR totals.
3. **Build the objective evidence packet.** Keep observations separate from
   interpretation: dated behavior, metric readings and denominators,
   counterexamples, Task/PR receipts, and missing sources. A Task or PR may
   establish that a change shipped; it does not establish that the user
   outcome occurred. Record shipped behavior, surprises, open work (Tasks
   still in flight, with their state — untouched by this review), and the
   evidence gaps the next chapter should close.
4. **Write the Project synthesis from that packet.** Lead with the person or
   agent the Project meant to help and the real use of Loopflow it meant to
   improve. Then state what that user can rely on now, what they still cannot,
   and the two to four facts that determine the conclusion. Explain whether
   the chosen KRs were credible evidence of the intended improvement or merely
   measured the mechanism. If the Project definition never made the user
   connection explicit, say so; do not invent one after the fact. Keep this
   synthesis concise enough to understand without reading the KR ledger.
5. **State the next-chapter implication without applying it.** Classify the
   Project as likely `carry`, `rewrite`, `defer`, `complete`, or `retire`, with
   the evidence that makes that disposition plausible. Unknown KRs do not
   automatically carry. Name goals the chapter revealed were not actual
   priorities — for example, no aligned active work, no instrumentation despite
   repeated promises, or current work consistently serving a different
   outcome. Also name active Tasks whose real outcome does not fit this
   Project; preserve their provider links and current status.

## Report

Emit this readable Markdown as the final Run output; the repository parent
archives it:

```markdown
# Project chapter report: <project> — <interval, or "interval unknown (baseline)">

Run id: <this LF_RUN_ID>

## Project synthesis
**Intended user improvement**: <who should experience what change; say "not explicit" when the definition only names a mechanism>
**Current user experience**: <what that user can and cannot rely on now>
**Chapter conclusion**: <did the bet improve the product, stall, or prove mis-shaped; distinguish interpretation from fact>
**Decisive facts**: <two to four dated observations from the evidence packet>
**Measurement fit**: <why the KRs did or did not prove the intended improvement>
**Next-chapter implication**: likely carry | rewrite | defer | complete | retire — <evidence, not a mutation>

## Objective evidence packet
Coverage: complete | Coverage: incomplete — <missing scope and source>
Expected KR rows: <n from ledger> · Actual KR rows: <m reported, incl. drift rows>
Verdict totals: <holds> holds · <does not hold> does not hold · <unknown> unknown

## Definition
**Verdict**: holds | does not hold | unknown
**Evidence**: <current outcome evidence; do not count this row as a KR>

## KR ledger
### <KR claim, verbatim>
**Status and active interval**: current | closed | replaced · <established or first observed> → <closed, replaced, last observed, or now>
**Verdict**: holds | does not hold | unknown
**Evidence**: <what was observed, where, over what dated time window>

## Shipped behavior
<what a user or agent can now do that they couldn't at chapter start>

## Task and PR receipts
<what landed in service of the Project; contribution evidence, not outcome proof>

## Priority and ownership findings
<goals not supported by actual priority; active work that belongs elsewhere or lacks a clear owner>

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

**Making the reader reconstruct the bet.** The ledger is the audit trail, not
the summary. A reader should understand the intended and current user
experience from the Project synthesis before opening individual KR rows.

**Sampling.** Judging three memorable KRs and skipping the awkward ones. Every
ledger row gets a verdict; the row counts make a silent skip visible.

**Reviewing only survivors.** A closed or rewritten KR is still a promise from
the KR era. Keep it as its own dated row; do not overwrite it with today's
wording or drop it because the provider no longer returns it.

**Fixing while reviewing.** A KR that should be rewritten is a finding for the
next chapter start, not an edit here.
