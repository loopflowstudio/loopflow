# Open questions — one tree of Waves

As of 2026-11-20. The plan: `scratch/plan.md`. What is locked, leaning and
unclear, with the words: `scratch/wave-tree.md`, `scratch/said.md`. The
earlier list, for PR #1986, is in this branch's history.

## Decide the shape

1. (answered: the Task record)
2. Linear stays flat: shared Waves under a Project are issues in it, no
   sub-issues. Any case that still wants Linear to show nesting?
3. A new Wave's default: shared under a shared Wave with no worker, personal
   under one someone is working on / always personal / always shared under a
   connected Wave.
4. A connected Wave's title, status and assignee belong to the tracker;
   everything else to Loopflow.
5. One tracker, one chat and one doc store per repository / any mix.

## Storage

6. Waves leave the repository: where goal and memory live, and whether an
   agent reads them as files placed in the worktree or only through `lf`.
7. With Linear, a shared Wave's goal lives on the Linear Project / on the
   plan, linked.
8. How a person's own Waves reach their other Machines: hub / a ref of
   their own.
9. With no ref to push to: a carrier repository / the hub as home.

## Behaviour

10. PRs and the Linear issue: every PR but the last links without closing /
    the first merge closes the issue and the Wave lives on / each PR that
    matters gets its own issue.
11. The issue is cancelled or reassigned while a worker runs: tell the
    thread and keep going / pause / depends on who did it.
12. A personal Wave that needs a person: reaches only you / posts in the
    nearest channel above / a setting.
13. Memory from a personal Wave closing under a shared one: into shared
    memory / into your own at that level / the closing step proposes.
14. Connecting a group that already has shared Waves to a Project: list and
    ask / file them / make the existing ones personal.
15. Publishing a Wave that sits under personal ones: publish those too /
    move it up / ask.
16. A steer to a Wave whose home Machine is off: queued / refused / offered
    a move.
17. A comment from someone else on your Wave's issue: the agent acts / it is
    shown and you decide.
18. A thread hears the level under it / everything under it.
19. Closing a Wave: what happens to connections and the Waves under it.
20. A worktree closes when its PR merges / stays while the Wave is open.
21. A cron's worktree: fresh each run / kept.

## Names

22. The word for a Wave someone is working on that is meant to end: Task /
    Wave.
23. Share and Surface as the two outward verbs, or `publish` for both.

## Not checked

- Linear accepting PR wording that links without closing ("Part of").
- Slack delivering each event to only one of several connections of one app.
- `cloudflared` named tunnels meeting Google's domain check.
- Whether Linear caps sub-issue depth (moot if question 2 stands).
- Liveness of the team and initiative links listed for deletion.
- `scratch/wave-tree-design.html` and `scratch/wave-tree-dynamics.html` have
  not been looked at rendered.

## From Slice 1 concept review (2026-11-22)

- Whether cascading child abandonment on `lf task complete <parent>` should later become a setting (like Linear's `autoCloseChildIssues`), once the Slice 1 test drive shows how it feels in practice.
