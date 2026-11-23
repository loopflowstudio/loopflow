# One tree of Waves, connected outward

Design in progress, 2026-11-20. Exact words: `scratch/said.md`. Tracker
facts: `scratch/hierarchy-research.md`. Two UX drafts:
`scratch/wave-tree-design.html`, `scratch/wave-tree-ux.md`. `scratch/plan.md`
is the narrower plan PR #1986 built (a Task with a parent).

Sorted by how firmly each was said. "Locked" was stated plainly or more than
once. "Leaning" was said with "probably", "I think", "I wonder" or as a
question. "Proposed" was never said by anyone but an agent.

## Locked

1. One kind of work, nestable without limit, called a Wave.
2. On a Wave each person has at most one session, one worktree and one
   current PR, with PRs over time, and at most one active runner on their
   Machine in that worktree. Cron jobs run in a separate worktree of their
   own. A person need not have any of these on a given shared Wave. When
   each opens and closes is not designed.
3. Cheap to create, close and reopen.
4. The thread in use does not change as work grows; nobody has to open the
   Waves under it.
5. Fan-out must not require a public Wave at every level.
6. Agents are not shared between people.
7. What a Wave lacks it finds by going up (chat, where to file, memory,
   settings). With none up to the repository, the action fails.
8. No automatic import. The CLI and API are good enough that an agent builds
   and maintains the setup when asked.
9. Loopflow can create a Project or Initiative outside, and mostly should not.
10. `lf wave publish` turns a personal Wave into a shared one, and publishes
    only that one.
11. A shared Wave can contain personal Waves; a personal Wave cannot contain a
    shared one.
12. Design for Linear first; Asana is retrofitted.
13. Projects end.
14. KRs are removed for now and may return.
15. Separate designs: dependencies; a doc store beside the tracker with no
    CRDT; plan edits reaching the work under them in batches.
16. Privacy of personal Waves does not matter; the size of what is shared does.
17a. For now a Wave is not connected to a Linear team or initiative. The
    tracker connections are a Project and an issue.
17b. Crons are per Machine. A shared Wave has no cron of its own until there
    is a hosted server for crons to run on.
17c. For each person a Wave has a home Machine: their one worker and one
    chat session on that Wave are on it. One remote branch and one PR per
    person per Wave.
17d. A new Wave takes its home Machine from the Wave above, else the Machine
    it was made on. Steering reaches a Wave on another of your Machines.
17e. Steering across people goes through the outside tools, such as a Linear
    comment, not through Loopflow directly. Possibly all steering should go
    that way; for now your own steers travel by Loopflow's own path.

## Locked when said, since questioned

17. Loopflow owns its own tree; it need not follow any outside hierarchy, and
    changing it changes nothing outside. Each Wave may be connected to a
    tracker item, a chat, docs. ("the only system that really works")
    Later: "this model is maybe a little unconstrained … ends up being kind of
    confusing". Two constraints were proposed in reply and are unanswered
    (Unclear 1 and 2).

## Leaning

18. Task and Wave are not separate kinds.
19. Waves are not repository content; they are published live like the
    tracker.
20. A person's store is the shared store plus their own Waves.
21. A Git Wave store always, with Linear and Asana as optional syncs.
22. A hub serving one person is in the open-source version; a hub several
    people share is probably a hosted service.
23. Hearing from Slack, Linear and Docs live is wanted for a five-person
    team; `cloudflared` can give a hub its public address.
24. A Wave that truly ends has no reason to keep its memory forever.
25. A small group of follow-ups maps to a Linear parent issue with sub-issues
    (said before the tree model).

## Unclear

1. One tracker, one chat and one doc store per repository, or any mix per
   Wave.
2. Whether shared means in the tracker (one line), or shared and connected
   stay independent (two lines).
3. One tracker connection per Wave, or an issue and a project on the same one.
4. A new Wave under a connected one: starts unconnected, or gets a tracker
   item by default.
5. The word for a Wave with a runner that is meant to end: Task, or Wave.
6. Publishing a Wave that sits under personal ones: publish those too, move
   it up, or ask.
7. Making a shared Wave personal again when others have Waves under it.
8. Where goal and memory live and how an agent reads and writes them.
9. Which facts about a connected Wave the tracker owns.
10. Tracker items nobody has taken: read on demand, or mirrored.
11. With no ref to push to: a carrier repository, or the hub as home.
12. How a person's own Waves reach their other Machines.
13. A team on the open-source version and Slack.
14. A thread hears the level under it, or everything under it.
15. Closing a Wave: what happens to memory, connections and the Waves under
    it. PR #1986 built one answer for the last.
16. Whether the root sits above repositories.
17. Whether a Project needs a target date, and who raises one past it.
18. Whether #1986 lands as built, with the fusion as later work.

## Proposed, unanswered

- A connected Wave's title, status and assignee belong to the tracker;
  everything else to Loopflow.
- Goal and memory as docs connected to the Wave; with Linear, the goal on the
  Linear project.
- Share (to Loopflow teammates) and Surface (to an outside tool) as the two
  outward verbs (`scratch/wave-tree-ux.md`).
- On a shared Wave each person has their own thread.
- A chat connected to a personal Wave is read-only; agents draft.
- Polling as the catch-up when a hub was off.
