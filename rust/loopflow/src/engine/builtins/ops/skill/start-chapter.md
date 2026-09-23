---
requires: the latest chapter report when one exists; Task worktree for tracked application
produces: .lf/chapters/<chapter-id>/ start record, scoped child proposals, applied accepted plan
---
Open a new chapter: brief the human, accept direction, gather scoped proposals and challenges, then apply only the plan the human finally accepts.

A chapter is a dated planning interval. Plans expire at its boundary; shipped
code, history, and in-flight work do not. The old chapter's report is
evidence; its priorities carry no authority into this one. Two explicit human
decisions gate this skill: direction before delegation, and the reconciled
plan before mutation.

## Ground in evidence

Read the latest repository chapter report (`.lf/chapters/<id>/review.md`)
when one exists; if the record is missing or stale, run `review-chapter`
bound to the same Task first and use its report. Read every
`wave/<name>/GOAL.md` and `MEMORY.md`, the PM snapshots
(`lf pm show --wave <name> --json`), and live state
(`lf ls`, `lf status <wave>`). Build a concise briefing: what held, what is
unknown, what shipped, which work remains active, and where reality has
moved. Start with what became better or worse for users, operators,
maintainers, and agents. Keep the exact KR evidence available underneath;
do not make the human reconstruct product meaning from the ledger.
Carry forward the review's proposed next-chapter implications as questions,
not inherited decisions: likely carry goals, changed assumptions, goals not
supported by actual priority, and misplaced or unowned work.

The tracked chapter archive and any Wave charter edits belong in a Task
worktree. If this Run is not in one, complete the human briefing and return
the accepted direction as a draft; continue the chapter from an existing or
new Task worktree before dispatching children or changing live planning.

## Gate 1 — accept direction before delegation

Bring the briefing, then begin with the human's experience rather than a
portfolio proposal. Ask a small number of open questions at a time and listen
before synthesizing:

- How did the last chapter feel? What created momentum, frustration, or
  surprise?
- What are they excited to make possible for users now?
- Which user pain or opportunity matters most, independent of the existing
  backlog?
- What feels complete, obsolete, urgent, valuable, or constrained?

These are direction questions, not requests to approve an agent-authored map.
Reflect the answers back in the human's language, name tensions that remain
open, and only then propose a provisional Wave map that names who each Wave
serves and what should improve in their real use of Loopflow. Discuss keep,
split, merge, retire, and new boundaries until the human explicitly says the
direction captures their intent. A list of old Projects or Tasks is not a
substitute for this conversation. No Wave or Project start Run may launch
before this record exists.

On a headless surface, open a durable `lf ask "<exact request>"` session for
this conversation and block on it. Provider exit, silence, or a ready
session does not count as acceptance; only the human completing the session
with an explicit decision does.

On acceptance, create `.lf/chapters/<UTC timestamp>-<this Run id prefix>/`
and write `draft.md`: the UTC start time and next review date for each scope,
this Run's id, the accepted repository direction and Wave boundaries with their reasons and
open tensions, source timestamps and explicit evidence gaps, and the frozen
ledger — every starting Wave, Project stable id, exact definition, exact KR
claim, and open Task.

## Dogfood the chapter process

When this live chapter exposes a defect in `start-chapter` instructions or
tooling, preserve the observation separately from the plan. Fix a reversible,
in-scope process defect in the current Task worktree, run its smallest useful
validation, and resume from the same gate. Process repair never counts as human
acceptance and never grants permission to mutate Wave, Project, Task, or Work
state early. Record larger defects as explicit follow-up work without making
the chapter wait for unrelated infrastructure.

## Delegate and reconcile

Launch bounded `wave/start-chapter` Runs sequentially. For an existing Wave,
use `lf -b --wave <name> wave/start-chapter "<chapter-id> + that Wave's accepted
brief and exact starting Project/KR ledger>"`. For a proposed new Wave with
no registered Work, use an unbound `lf -b wave/start-chapter "<proposed Wave
name, repository, and accepted boundary>"` Run; do not create its Work before
the final decision. During this orchestration child starts are proposal-only:
each inspects its real objective, code, PM snapshot, and active work, gathers
scoped human direction, proposes its Project portfolio, and returns `agree`
or `challenge` against the parent boundary with reasons and evidence.

A challenge is first-class output. Capture each child's final proposal,
included Project proposals, and all Run ids, and write their scoped
`start.md` files from this Task worktree; children do not write into their
Wave checkout. Preserve each challenge, reconcile overlaps and gaps, and
return the complete plan to the human. A failed or unanswered child
stays explicit and blocks application for its affected scope; it never
becomes tacit agreement. Record every child Run id, agreement, challenge,
failure, and unanswered question in `draft.md`.

## Gate 2 — accept the reconciled plan before mutation

Only the human's explicit final acceptance crosses the mutation boundary.
Then apply the accepted plan through the existing owners:

- Wave objective, boundary, or roster edits happen in the current Task
  worktree and follow the ordinary PR path.
- Project definitions and KRs use `lf pm project create/update/archive`.
- Task dispositions use `lf pm task create/update/done/move`.
- Durable pursuit ends through `lf work abandon` or the scoped lifecycle
  command, only where retirement was explicitly accepted.

Every prior Project and open Task receives an explicit carry, rewrite,
complete, or retire disposition; none inherits priority merely by existing.
Record each operation's success, failure, and canonical identifier in
`draft.md`. Partial application leaves the affected live source as the
authority and reports the mismatch — never rewrite the accepted plan to make
an operation look successful. Seal the interval by renaming `draft.md` to
`start.md` only after every accepted operation succeeds. A failed or
interrupted draft never displaces the last valid chapter. Corrections append
a dated correction section rather than replacing an observed decision.

## Publish the chapter boundary

The chapter is not durable merely because live planning changed or a local
`start.md` exists. After application succeeds, create one repository checkpoint
containing the preceding chapter's `review.md`, every scoped review and start
report, the accepted `start.md`, and any accepted Wave charter edits. Use `lf
commit` so the archive has one identifiable Git boundary. Do not include
scratch presentations or transient planning notes.

Obtain explicit human authority before pushing. Then publish the checkpoint
through the ordinary PR workflow the human selected; default to `lf pr submit`
when a person should land it. Future chapter reviews open their interval from
the newest merged `.lf/chapters/<chapter-id>/start.md`, using the record's UTC
start time rather than commit age. Until that commit merges, report that live PM
state is ahead of the repository archive and do not claim the new chapter is
fully published. A later correction appends evidence in a new commit; it never
rewrites the observed start decision.

## Preservation

Active Task work survives unless the final human decision explicitly moves,
completes, or retires it. Retiring a plan never deletes product code,
branches, commits, merged PRs, Run records, or the chapter archive; Git and
Linear keep the history of removed Wave files and archived planning objects.

## What to avoid

**A backlog audit wearing a chapter's clothes.** Routine reprioritization is
`wave/operate`'s job. A chapter start re-derives the map from direction, not
from the open Task list.

**Launching early.** A child dispatched before the human accepts direction
plans against direction no one gave.

**Applying early.** A mutation performed before final acceptance turns a
child challenge into a rollback problem. Children propose; the root applies
once, at the end.

**Children as executors.** A Wave start that only transcribes the parent
brief wastes the session; the brief stays provisional until scoped evidence
and human direction confirm or challenge it.
