# Open questions — work with children, and where plans live

Only what is still open as of 2026-11-18 (kickoff). Words and earlier rounds:
`scratch/said.md`. Work with children is planned in `scratch/plan.md`; the
rest is not chosen.

## Made at kickoff with nobody present, for review

- The checkout is retired at close, as today, not at merge. A merged parent's
  conversation stays in its checkout; after close it reopens in the
  repository's main checkout.
- A child copies its parent's Project when filed; nothing climbs at read time.
- `lf task complete <parent>` never stops and takes no new flag: merged
  children become done, never-started ones are abandoned, and a child with
  anything unmerged stays open with its link and is named (concept review;
  replaces the kickoff's `--keep` / `--abandon`).
- A conversation belongs to one Task and conducts that Task's children: two
  relations. `session_tasks()` is not widened (concept review).
- A conductor is told when a child stops at an interactive step, fails,
  merges or closes, not at each step.
- A closed parent's conversation still hears about a child that is open.
- No `merged` / `closed` booleans: `pr.phase` and `state` already say it.
- Removing several PRs in a row on one Task is a follow-up Task, not this PR.
  It is live behaviour (v0.13.16, `docs/ops.md:38`).
- Wave and repository conversations do not hear about Tasks in this change.

## Made while building slices 1 and 2, for review

- Only a done or abandoned Task's conversation falls back to the main
  checkout. An open Task with no checkout still refuses with "has no
  checkout; place it with `lf checkout`".
- `lf resume <Task>` still chooses among the Task's unfinished conversations
  only; a completed one reopens by Session ID.
- A child's `lf task status` prints `parent: LOO-12` (answers the third
  question below for the detail view; lists are not built).
- A child line's standing is the Task's state plus the step it stands at:
  `moving`, `idle at human-review`, `unstarted`. The docs example changed
  from "running at implement" to match; a running child's current step is
  not on `TaskRow`.
- `--no-wave` and an omitted `--wave` are the same value, so a follow-up
  filed with `--no-wave` still joins its parent's Wave. `--parent none` is
  how to file outside it.
- A follow-up filed in another repository than its parent keeps the link and
  takes no Project.

## Raised by writing the docs (2026-11-18)

- "Parent" already means the stack parent across `docs/work.md`,
  `docs/agent-api.md` and `docs/ops.md` ("forks from its parent's branch").
  The docs now say "a parent says where a Task came from, a stack says which
  branch it starts from". Is the filing link called parent / follow-up of /
  something else, and does the stack keep the word?
- A follow-up that needs its parent's unmerged code must also pass
  `--stack-on`. Filed from a Task whose PR has not merged, should a child
  stack on it by default? The docs say no (plan: neither implies the other).
- A child whose parent closed is listed top-level. Does anything on that
  child still show where it came from in the human output, or only in
  `--json`?

## One type of work

1. (planned: see above and plan decision 7)
2. (planned: a sibling; `--parent` says otherwise)
3. The fusion of Wave and Task: behaviour first with storage later / one type
   in the store from the start?
4. Is "work" the name for everything, with Task and Wave kept as words for
   what a piece of work is doing?
5. Children in the sidebar: folded behind a count until one needs you / always
   listed / listed while the parent's conversation is open?
6. (answered: both providers resume from another directory after the first
   is deleted; probes in `scratch/said.md`)
7. How one tree maps onto a tracker with Projects, issues and sub-issues, and
   onto LOO-938's Wave and Project records.

8a. A folder (a short-lived group): a tracker Project of its own / a group
    only its owner sees, its Tasks showing in the parent's Project / a Task
    with subtasks? Is it a third kind, or work with children that has no PR,
    plus a choice of how the team sees it?
8b. May a parent have no PR and no checkout of its own, and where does its
    conversation run then?

## Plans and docs

8. Who writes the shared plan most of the time: the person / the agent with
   review / both in one doc?
9. A doc store as a repository setting like the tracker / docs as links of any
   kind on a piece of work?
10. With no outside doc tool, the plan file lives: in the repository with a PR
    per change / with the planning records outside the working tree?
11. A follow-up gets the plan as it was reviewed / the latest?
12. Does a running Task see amendments made after it started?
13. Do comments and edits people make in the doc reach the conversation that
    owns the work?

## Dependencies (its own design)

14. One link, "follow-up of", with waiting and starting branch as settings on
    it / separate relations?
15. The Task waited on is abandoned or redone: the parent's conversation is
    woken and asks / waiting Tasks start from main / they stay waiting and say
    why?
16. May a Task wait on several?

## Shipping

17. `lf ship`, `-b` and the Auto-Ship approval: which one stays, and what does
    `ship` become shorthand for? Not read in the code yet.
18. A follow-up runs to merge because of how it was filed / its parent's
    conversation answers its stops?
19. What shows that something merged without being opened?

## Doc edits reaching children (its own Task)

20. What closes a batch of doc edits: a quiet spell / someone saying "send
    this" / nothing, each child reads the latest at its next step?
21. Does every child get the batch, or does the parent's conversation read it
    and steer the children it touches?

## Not checked

`scratch/explore.html` round six (the whole window as sequences) has not been
looked at rendered, and predates the one-type direction.
