# Open questions — work with children, and where plans live

Only what is still open as of 2026-11-18 (kickoff). Words and earlier rounds:
`scratch/said.md`. Work with children is planned in `scratch/plan.md`; the
rest is not chosen.

## Made at kickoff with nobody present, for review

- The checkout is retired at close, as today, not at merge. A merged parent's
  conversation stays in its checkout; after close it reopens in the
  repository's main checkout.
- A child copies its parent's Project when filed; nothing climbs at read time.
- `lf task complete <parent>` closes children with nothing unmerged and stops
  on the others, taking `--keep <ID>` or `--abandon <ID>`.
- No `merged` / `closed` booleans: `pr.phase` and `state` already say it.
- Removing several PRs in a row on one Task is a follow-up Task, not this PR.
  It is live behaviour (v0.13.16, `docs/ops.md:38`).
- Wave and repository conversations do not hear about Tasks in this change.

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
