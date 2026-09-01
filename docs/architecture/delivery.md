# Delivery

A Task binds durable planning to one managed worktree and one active remote
branch at a time. The current implementation retains settled PRs as a serial
delivery history.
Git owns commits and branches. GitHub owns PR heads, checks, and merge. Local
state records enough evidence to resume the workflow safely.

```bash
lf task prepare INF-123
lf --task INF-123 implement
lf commit -m "parser: accept nested groups"
lf pr publish --title "Parser: accept nested groups"
lf pr land -c
```

## Delivery flow

```text
Linear Issue
    |
    v
Task Work ----> managed worktree ----> commits
    |                                     |
    |                                     v
    +--------------------------------> GitHub PR
                                          |
                                 checks / repair / merge
                                          |
                                complete or rotate chain
```

| Object | Authority |
| --- | --- |
| Task directive and Project membership | Linear Issue |
| managed worktree placement and serial PR state | Task delivery records plus resolved Git repository |
| commits, branch ancestry, rebase state | Git |
| PR head, required checks, merge | GitHub |
| landing supervision and repair admission | exact recorded PR head plus landing generation |

Task types live under [`work/task/`](../../rust/loopflow/src/work/task/). Operational Git,
PR, CI, and landing workflows live under [`ops/`](../../rust/loopflow/src/ops/).
The exact landing fence is modeled in
[`pr_landing.rs`](../../rust/loopflow/src/pr_landing.rs).

## Create or reuse the worktree

`lf task prepare` resolves one existing Linear Issue inside one Project and
creates or reuses its managed worktree and first serial PR record. It installs
no controller. `lf task run` uses the same substrate and additionally starts
the built-in end-to-end Task flow. The repository identity—not the caller's
current directory spelling—selects the Git directory and sibling worktree
namespace.

Only Task Work owns a delivery worktree. Project and Wave processes coordinate;
they do not edit product files in substitute worktrees.

A dependent change that must begin before its parent merges uses another Task:

```bash
lf task run INF-124 --stack-on INF-123
```

The child records its fork point and targets the parent's active PR branch.
After the parent merges, it replays child-authored commits onto current main.
The parent Task does not hold two simultaneously open PRs.

## Commit and publish

```bash
lf commit -m "checkpoint: parser proof"       # local checkpoint
lf commit -m "parser: accept nested groups" -p # commit and push
lf pr publish                                  # visible, still in flight
lf pr arm                                      # request exact-head auto-merge and return
lf pr land                                     # prepared and auto-merged
```

`publish` creates or refreshes the current PR without rebasing. `arm` and
`land` integrate current main, clear merge-time scratch state, collapse
checkpoint history into one authored commit, verify once, and push the exact
head. `arm` requests GitHub auto-merge and returns. `land` watches through
merge. `submit` performs the same preparation but leaves the exact-head merge
to a human. These delivery commands inspect Task delivery state when present;
they do not require a controller or certify that a particular Flow ran.

`lf pr open` is the presenting verb; it opens the review surface after
publishing. Headless Task flows use publish, arm, or land.

## Serialize the exact Git races

Loopflow uses advisory OS file locks beneath the repository's absolute Git
directory:

```text
<absolute-git-dir>/loopflow/rebase-owner.json
<absolute-git-dir>/lf-pr-mutation.lock
```

The open file descriptor is authority. JSON is a readable receipt. Process
death releases the kernel lock even if metadata remains.

### Rebase operation

Provider launches receive no durable Git writer token. Independent agents may
coexist in the shared worktree. A rebase locks `rebase-owner.json` for the Git
sequencer lifetime; new agent launches refuse while that operation is live.

| Concurrent work | Result |
| --- | --- |
| agent + independent agent | allowed; use distinct output paths |
| read/build/test + agent | allowed |
| live rebase + new agent launch | blocked |
| rebase + its exact recovery child | allowed |
| stale rebase record without a kernel lock | adopted or removed through the rebase path |

The rebase owner authorizes only its exact sequencer and recovery child. It does
not make a provider the worktree owner or serialize ordinary edits, Run
recording, tests, or planning writes.

### PR mutation

`lf-pr-mutation.lock` covers only Task PR/head transitions: publication,
repair, range healing, merge request, settlement, and serial rotation. A
second mutation fails fast while that exact section is held.

Raw Git commands do not participate in these advisory protocols. Loopflow can
observe and diagnose their state, but cannot claim to have excluded them.

## Land an exact head

```text
observe GitHub PR head H1
          |
          v
claim landing generation G
          |
          v
wait for required checks on H1
          |
      +---+---+
      |       |
    pass     fail
      |       |
    merge   admit one repair for (G, H1)
              |
              v
           new head H2 --> fresh check evidence
```

A landing supervisor never transfers green checks from one head to another.
One failed head may admit one repair. A moved head requires a new observation
and check set. GitHub remains the final merge authority.

After merge, bare `lf pr land` settles that PR and leaves the Task open.
`lf pr land -c` completes the Task. `lf pr land --next <slug>` rotates the
serial chain to a new branch from fetched main.

## Failure and recovery

- An interrupted provider Run does not discard the worktree or PR chain.
- A failed check is GitHub evidence, not a completed local transition.
- A crashed rebase keeps Git's sequencer state; explicit recovery adopts it
  with fresh operation identity.
- A crashed PR mutation is retried by resolving current Git and GitHub truth
  inside the same narrow lock.
- A Task moved to another Linear Project fails closed before automated commit,
  push, publication, merge request, rotation, or completion.

## Boundary contracts

- One Task has one active remote branch. A checkout tracking it identifies the
  Task; the stored worktree path is placement.
- Settled PRs may remain as serial history until the one-branch Task model
  replaces rotation.
- Simultaneously open dependent work belongs to another stacked Task.
- Git and GitHub remain authority for their own objects.
- Locks serialize exact local races, not all activity.
- Run identity does not grant Git or PR mutation authority.
- Repair and merge decisions are fenced by exact PR head evidence.

## Next

[Planning →](planning.md) separates the Task objective from controller playheads.
[Homes and processes →](homes.md) owns the machine and process boundaries around
delivery.
