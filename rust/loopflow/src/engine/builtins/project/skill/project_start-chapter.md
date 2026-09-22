---
requires: a bound Project or a proposed new bet under a bound Wave
produces: a scoped proposal in final Run output, agree or challenge upward
---
Open one Project's chapter: propose the bet's definition, proof-shaped KRs, and exact Task dispositions against direction and this Project's actual state.

A chapter is a dated planning interval. The Wave brief — when this Run was
launched by a `wave/start-chapter` session — is provisional direction, not a
plan to execute. During chapter orchestration this skill is **proposal-only**:
it returns the shaped bet and its verdict on the Wave boundary; the root
applies the plan once, after final human acceptance.

## Ground in evidence

For an existing Project, read its current definition, KRs, and Tasks
(`lf pm show --wave <name> --project <slug> --json`). Read the Wave brief,
scoped review findings, and starting KR/Task ledger from the launch prompt;
the parent's Task worktree is separate from this checkout. For a proposed
new Project, read the exact Wave direction and nearby Projects instead of
inventing a current Project id or KR history. Read live Task state
(`lf status <wave> --json`, `lf roadmap`) and the actual code the bet
concerns. A KR the review marked unknown is a shaping question for this
chapter: prove it, rewrite it, or retire it — don't re-adopt it untouched.

## Direction

Gather human direction for this bet before settling anything: is the bet
still worth making, at what scope, and what would prove it. On a headless
surface, open a durable `lf ask "<exact request>"` session for the material
choices; provider exit, silence, or a ready session does not count as
acceptance.

Where this Project's evidence contradicts the Wave brief — the bet is already
won, the boundary is wrong, the KRs measure the wrong thing — challenge it.
The return value is explicit: `agree` or `challenge` against the Wave
boundary, with reasons and evidence. Do not silently comply and do not
silently diverge.

## Shape the bet

Propose a definition and KR set for the new chapter:

- The definition states the bet — one measured improvement or standing
  frontier inside exactly one Wave.
- KRs read as proof: observable end states a maintainer could verify, with
  the endurance window stated when the claim needs one. Not backlog bullets,
  implementation receipts, or issue ids.
- Every existing open Task receives an explicit carry, rewrite, complete, or
  retire disposition; none inherits priority merely by existing. Select the
  opening Tasks for the chapter; in-flight work keeps running unless its
  accepted disposition says otherwise.

Return the scoped proposal in final output: the proposed
definition, the exact proposed KR ledger, every Task disposition, the next
review date, and the agree/challenge verdict with its reasons. The repository
parent archives this output in its Task worktree; do not write a chapter file
in the Project checkout.

## Standalone invocation

Without a parent orchestration, follow the same briefing and human direction
contract, then return an accepted scoped proposal in final output. Application
belongs to a repository `start-chapter` in a Task worktree, after the full
portfolio has been reconciled and accepted. Do not invent a Wave or
repository decision or modify parent boundaries.

## What to avoid

**Executing the Wave's plan.** The brief is provisional. Check it against the
code and the KR evidence before adopting it.

**Mutating during orchestration.** Proposal-only means no `lf pm` writes and
no Task closures. Application belongs to the root after final acceptance.

**Receipt-shaped KRs.** "Ship the parser PR" is a Task. "Cold start under two
seconds, observed across a week of runs" is a KR.

**Silent Task inheritance.** An open Task without an explicit disposition is
a plan leak from the old chapter.
