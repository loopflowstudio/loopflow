---
requires: a bound Wave or a proposed new Wave in the launch prompt
produces: a scoped proposal in final Run output, agree or challenge upward
---
Open one Wave's chapter: check the parent brief against this Wave's reality, gather scoped human direction, and propose the Project portfolio.

A chapter is a dated planning interval. The parent brief — when this Run was
launched by a repository `start-chapter` — is provisional direction, not a
plan to execute. During repository orchestration this skill is
**proposal-only**: it returns its portfolio and its verdict on the parent
boundary; the root applies the plan once, after final human acceptance.

## Ground in evidence

For an existing Wave, read its `GOAL.md` and `MEMORY.md`, PM snapshot
(`lf pm show --wave <name> --json`), live state
(`lf status <name> --json`), and actual code and in-flight work in its area.
Read the parent brief and scoped chapter report carried in the launch prompt;
the parent's Task worktree is separate from this checkout. For a proposed new
Wave, use the exact name, repository, and boundary in the parent brief and
inspect adjacent Waves and code; do not invent current GOAL, PM, or Work state.
Verify the brief against the territory. For a standalone existing Wave, use a
locally available report; if none exists, run `wave/review-chapter` and use
its final output.

## Direction

Gather scoped human direction before settling anything: which bets feel done,
which expired, what pressure the code itself is applying, what the parent
brief missed. On a headless surface, open a durable `lf ask "<exact request>"`
session for the material choices; provider exit, silence, or a ready session
does not count as acceptance.

Where this Wave's evidence contradicts the parent brief — wrong boundary,
stale purpose, a tension the brief papered over — challenge it. The return
value is explicit: `agree` or `challenge` against the parent boundary, with
reasons, open tensions, and evidence. A challenge is first-class output for
the parent to reconcile. Do not silently comply and do not silently diverge.

## Propose the portfolio

Choose the Project portfolio for the new chapter. Good Projects are
completable behavioral improvements or standing quality frontiers, each with
KRs that read as proof — observable end states, not backlog bullets or
implementation receipts. Every old Project and open Task receives an explicit
carry, rewrite, complete, or retire disposition; none inherits priority
merely by existing.

For each existing Project whose definition or KRs need real shaping, launch
`lf -b --wave <name> --project <project-id> project/start-chapter
"<chapter id, its slice of direction, review findings, and exact starting
KR/Task ledger>"`. For a new bet without a Project id, launch
`lf -b --wave <name> project/start-chapter "<new bet brief and Wave direction>"`;
it proposes a Project and does not create one before final acceptance.
Its proposal may challenge this Wave's framing the same way
this Wave challenges the parent's; preserve and reconcile before returning.
Include every complete Project proposal and Run id in final output so the
repository parent can archive them. A trivial disposition (complete a
finished Project, retire a dead one) is proposed directly.

Return the scoped proposal in final output: the proposed purpose and
boundaries, the exact proposed Project definitions and KR ledger, every
disposition, the next review date, child Run ids, and the agree/challenge
verdict with its reasons. The repository parent archives this output in its
Task worktree; do not write a chapter file in the Wave checkout.

## Standalone invocation

Without a parent orchestration, follow the same briefing and human direction
contract, then return an accepted scoped proposal in final output. Application
belongs to a repository `start-chapter` in a Task worktree, after the full
portfolio has been reconciled and accepted. Do not invent a repository
decision or modify parent boundaries.

## What to avoid

**Executing the parent's plan.** The brief is provisional. A Wave that never
checks it against its own code and human direction launders the parent's
guess into a commitment.

**Mutating during orchestration.** Proposal-only means no `lf pm` writes, no
charter edits, no Task closures. Application belongs to the root after final
acceptance.

**Portfolio by inertia.** Carrying every existing Project forward because
deleting feels destructive. Expired bets lose planning authority; the code
they shipped stays.

**KRs as task lists.** A KR states an observable condition a reviewer can
verify. Implementation steps belong in Tasks.
