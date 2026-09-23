---
requires: .lf/chapters/ archive when one exists, wave/<name>/ directories, PM snapshots
produces: a chapter report in final Run output; tracked archive when run in a Task worktree
---
Close the repository's chapter: gather every Project's KR report and judge the Wave portfolio on dated evidence.

A chapter is a dated planning interval over the existing Wave → Project → Task
model, not a new Work kind or a code branch. This review closes the interval's
evidence record so the next `start-chapter` can plan without inheriting old
priorities. It runs headlessly to completion and never calls `lf ask`.

## Boundaries

Review does not alter KRs, charters, Tasks, or code — no Wave charter, Linear
object, Work state, branch, or product code changes. When running in a Task
worktree, this repository Run alone writes the tracked chapter report and the
copies of child reports under `.lf/chapters/`. Bound Project Runs use their
Wave checkout and return reports in final output; they do not write chapter
files. Without a Task worktree, emit the full report in final output and state
that the archive was not persisted.

Determine archive authority from the active Run record, not the checkout name:
`lf runs "$LF_RUN_ID" --json` must contain a declared or inherited `task:`
subject. A sibling directory whose name merely looks Task-like is not enough.

## Establish the interval

The newest merged `.lf/chapters/<chapter-id>/start.md` is the interval eligible
for review: its recorded start time opens the chapter, now closes it,
and its ledger records the Waves, Projects, exact KR claims, and open Tasks the
chapter started with.

A local or unmerged start remains a pending publication, even when its live PM
operations succeeded. Report that mismatch without displacing the last merged
chapter boundary.

If no start record exists, say the interval is unknown and perform a baseline
review. Start at the first reconstructable KR state, not the repository's first
commit. Build a lineage ledger from dated PM snapshots or revisions, archived
or closed Projects, prior Run context, and repository records that preserve
Project content. Include KRs that were closed, removed, or rewritten, with
their exact claim and best-supported `established`, `last observed`, and
`closed or replaced` dates. Pre-KR git history is context only: it cannot be
judged against promises that did not exist.

If exact KR lineage cannot be reconstructed, include every discoverable row,
label its dates unknown or first-observed rather than invented, and mark
historical coverage incomplete. Separately state whether the current
Wave/Project/KR union was completely enumerated. Write the dated baseline
report to a new
`.lf/chapters/<UTC timestamp>-<this Run id prefix>/review.md` when in a Task
worktree. Leave out `start.md`: a baseline is not a chapter start and cannot
call itself complete.

## Workflow

1. **Read the evidence sources.** The chapter start ledger, the current Wave
   roster (every `wave/<name>/GOAL.md`), dated PM snapshots
   (`lf pm show --wave <name> --json`), Work status
   (`lf status <wave> --json`, `lf roadmap --json`), and the code, PR, and Run
   evidence reachable through the repository's configured paths. For a
   baseline, search those dated sources for the first, changed, and last
   observations of each Project and KR; do not expand the interval into the
   pre-KR repository era. If PM
   refresh fails or its timestamp is too old for a KR's window, retain the
   observation as stale and mark the affected coverage or verdict unknown.
2. **Enumerate the Wave and Project union before launching anything.** Union
   the start ledger with the current roster and PM snapshots. Freeze each
   Project's exact KR lineage and expected row count, including closed or
   replaced rows when evidence preserves them. A retired or currently
   unresolvable Wave or Project stays in the union from the start record and is
   marked with its missing live source. If the union cannot be fully
   enumerated, name the missing source before continuing.
3. **Launch Project reviews directly.** Do not add Wave aggregator Runs. For
   every resolvable Project in the union, run `lf -b --wave <name> --project
   <project-id> project/review-chapter "<chapter interval and this Project's
   exact starting KR ledger>"`. Pass the scoped ledger because the child has a
   different checkout. Run independent Projects in bounded parallel batches
   the host can sustain. Capture every final report and Run id; the repository
   Run will group them into Wave ledgers and owns all archival writes.

   Use the Run record when launch output is lost: `lf runs --parent
   "$LF_RUN_ID" --json` enumerates direct children without the recent-history
   cap, and `lf runs <run-id> --final` prints its durable final report. Runs
   without final-answer receipts are labeled and return streamed prose from
   their last completed provider turn, so isolate the report without pretending
   the narration is absent. A
   child counts only when it is terminal `completed` and has a final report. An
   unterminated record is unknown, not proof of liveness or failure; inspect
   `lf top` before deciding whether a replacement is justified, and never
   signal an unclaimed provider process. A failed or missing child remains
   explicit incomplete coverage, never silently omitted or re-derived as if it
   reported.
4. **Verify and aggregate the union.** Check Project reports against the frozen
   union: expected versus actual reports, every exact KR row accounted for,
   every child coverage statement carried through. Recompute verdict totals
   from the KR rows; do not trust a child's prose total. Holds + does not hold +
   unknown must equal the actual row count at Project, Wave, and repository
   levels. A mismatch is an evidence error to resolve or report, not a number
   to smooth over. Parent aggregation cannot upgrade incomplete child coverage.
5. **Judge each Wave, then the portfolio.** Group the verified Project reports
   by Wave and judge whether their bets advanced `wave/<name>/GOAL.md`. Then
   judge whether the Wave map advanced the repository: mandates that advanced
   or stalled, boundaries that chafed or overlapped, work that landed outside
   every mandate, and Waves whose evidence could not support a judgment.

   Preserve two explicit layers. The objective layer contains exact KR claims,
   dates, denominators, observations, counterexamples, verdicts, and Task/PR
   receipts. The interpretive layer starts at the Project: who the bet meant to
   help, what changed in their real use of Loopflow, what they can rely on now,
   and the few decisive facts supporting that conclusion. Carry the child
   Project synthesis without upgrading its evidence. Wave and repository
   summaries build from those syntheses, not directly from a pile of KR rows.
   If a Project's user benefit or the link from its KRs to that benefit was
   never explicit, report that planning weakness instead of inventing intent.
6. **Prepare explicit questions for the next chapter.** At Project, Wave, and
   portfolio level, separate four judgments: goals likely worth carrying in
   some form; learnings that should change the next plan; goals the actual Task
   mix reveals were not priorities; and active work that is misplaced,
   cross-Wave, or unowned. A failed goal may still deserve carryover, while an
   unknown goal with no aligned work may be a priority fiction. Ground these
   judgments in the objective packet and current `lf roadmap --json`; preserve
   exact Task links and status when naming work. Do not apply dispositions —
   this review hands proposals to `start-chapter`.

## Verdict rules

Wave ledgers roll up Project KR verdicts; carry them through unchanged. Each
KR row keeps one verdict: holds, does not hold, or unknown. Missing, stale, or
ambiguous evidence is unknown — never a silent pass or fail. A checked box,
PR, or implementation receipt alone cannot prove an outcome or its endurance
window; neither can a single successful run. A dated counterexample inside a
universal or conjunctive claim's window is enough for `does not hold`, even
when other parts of the window are missing. `Holds` requires the complete
denominator the claim names.

## Report

Emit the readable Markdown below as the final Run output. When in a Task
worktree, also write it to `.lf/chapters/<chapter-id>/review.md`. If that
file already exists, append a dated factual correction or new observation;
preserve the earlier verdict and evidence:

```markdown
# Repository chapter report — <interval, or "interval unknown (baseline)">

## Coverage
Coverage: complete | Coverage: incomplete — <missing scope and source>
<Wave roster; expected/actual Project and KR counts; baseline lineage coverage; Project Run ids and report paths>

## What changed for Loopflow users
<a concise interpretive account organized by Wave; name intended improvements, experienced changes, remaining user friction, and the decisive Project facts>

## Next-chapter implications
### <wave>
**Likely carry**: <goals still valuable and supported by evidence or active pressure>
**Learned**: <what the chapter changed about the model>
**Not evidenced as a priority**: <goals the Task mix and instrumentation do not support>
**Misplaced or unowned work**: <active work that does not fit its current Project or Wave>

### Portfolio gaps
<cross-Wave overlaps, orphaned work, catch-all Projects, and exact current Task references>

## Wave ledger
### <wave>
<Wave synthesis: intended user improvement, current experience, decisive Project conclusions, and evidence limits>

#### <project>
<child Project synthesis unchanged; do not make the reader reconstruct it from KR rows>

## Objective evidence ledger
<Project report paths and Run ids; exact KR rows, dated facts, verdict totals, Task/PR receipts, open work, and evidence gaps>

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

**Leading with the audit trail.** Counts and KR rows protect the review from
storytelling, but they are not the reader's entry point. Establish facts
first, then lead the report with the Project and user consequences they
support.

**Fixing while reviewing.** Stale KRs and dead Tasks get dispositions in the
next `start-chapter`, not edits here.
