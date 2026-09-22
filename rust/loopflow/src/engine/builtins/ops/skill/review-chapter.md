---
requires: .lf/chapters/ archive when one exists, wave/<name>/ directories, PM snapshots
produces: a chapter report in final Run output; tracked archive when run in a Task worktree
---
Close the repository's chapter: gather every Wave's chapter report and judge the Wave portfolio on dated evidence.

A chapter is a dated planning interval over the existing Wave → Project → Task
model, not a new Work kind or a code branch. This review closes the interval's
evidence record so the next `start-chapter` can plan without inheriting old
priorities. It runs headlessly to completion and never calls `lf ask`.

## Boundaries

Review does not alter KRs, charters, Tasks, or code — no Wave charter, Linear
object, Work state, branch, or product code changes. When running in a Task
worktree, this repository Run alone writes the tracked chapter report and the
copies of child reports under `.lf/chapters/`. Bound Wave and Project Runs use
their Wave checkout and return reports in final output; they do not write
chapter files. Without a Task worktree, emit the full report in final output
and state that the archive was not persisted.

## Establish the interval

The newest `.lf/chapters/<chapter-id>/` directory containing `start.md` is the
interval eligible for review: its start time opens the chapter, now closes it,
and its ledger records the Waves, Projects, exact KR claims, and open Tasks the
chapter started with.

If no start record exists, say the interval is unknown and perform a baseline
review: evaluate every currently visible KR, label the interval and historical
coverage unknown, and write the dated baseline report to a new
`.lf/chapters/<UTC timestamp>-<this Run id prefix>/review.md` when in a Task
worktree. Leave out `start.md`: a baseline is not a chapter start and cannot
call itself complete.

## Workflow

1. **Read the evidence sources.** The chapter start ledger, the current Wave
   roster (every `wave/<name>/GOAL.md`), dated PM snapshots
   (`lf pm show --wave <name> --json`), Work status
   (`lf status <wave> --json`, `lf roadmap --json`), and the code, PR, and Run
   evidence reachable through the repository's configured paths. If PM
   refresh fails or its timestamp is too old for a KR's window, retain the
   observation as stale and mark the affected coverage or verdict unknown.
2. **Launch one bounded Run per Wave** in the union of the start ledger and the
   current roster: `lf -b --wave <name> wave/review-chapter "<chapter interval and
   this Wave's exact starting Project/KR ledger>"`. Pass the scoped ledger in
   the launch prompt because the child has a different checkout. Capture its
   final report, its included Project reports, and all Run ids; write each
   scoped report into the chapter archive from this Task worktree. A retired
   or currently unresolvable Wave is reviewed from its recorded start evidence and marked
   with the missing live source. A failed child Run stays explicit: its scope
   is incomplete coverage, never silently omitted or re-derived as if the
   child had reported. If the Wave union itself cannot be fully enumerated,
   mark coverage incomplete and name the missing source.
3. **Verify the union.** Check the collected Wave reports against the start
   ledger and current roster — every expected Wave accounted for, every child
   coverage statement carried through. Parent aggregation cannot upgrade an
   incomplete child.
4. **Judge the portfolio.** With the complete Wave ledger, judge whether the
   chapter's Wave map advanced the repository: mandates that advanced or
   stalled, boundaries that chafed or overlapped, work that landed outside
   every mandate, and Waves whose reports could not support a judgment.

## Verdict rules

Wave reports roll up Project KR verdicts; carry them through unchanged. Each
KR row keeps one verdict: holds, does not hold, or unknown. Missing, stale, or
ambiguous evidence is unknown — never a silent pass or fail. A checked box,
PR, or implementation receipt alone cannot prove an outcome or its endurance
window; neither can a single successful run.

## Report

Emit the readable Markdown below as the final Run output. When in a Task
worktree, also write it to `.lf/chapters/<chapter-id>/review.md`. If that
file already exists, append a dated factual correction or new observation;
preserve the earlier verdict and evidence:

```markdown
# Repository chapter report — <interval, or "interval unknown (baseline)">

## Coverage
Coverage: complete | Coverage: incomplete — <missing scope and source>
<Wave roster reviewed; child Run ids and report paths>

## Wave ledger
### <wave>
<mandate; coverage; KR verdict rollup; shipped behavior; surprises; open work; evidence gaps>

## Portfolio judgment
<did these Waves advance the repository; boundary tensions; orphaned work>

## Evidence gaps
<what the next chapter should instrument so its review isn't guessing>
```

Every level states `Coverage: complete` or `Coverage: incomplete`; incomplete
coverage names the missing scope and source and forfeits any claim to a
complete report. This report is the evidence packet the next `start-chapter`
conversation carries in.

## What to avoid

**Sampling.** Reviewing the two loudest Waves and calling it a portfolio
judgment. Enumerate the union first; then review everything or mark what's
missing.

**Treating unknown as failure or success.** Unknown means the evidence isn't
there. Record the gap; don't guess the verdict.

**Fixing while reviewing.** Stale KRs and dead Tasks get dispositions in the
next `start-chapter`, not edits here.
