# Delivery

A Task binds durable planning to one managed worktree and one active remote
branch at a time. The current implementation retains settled PRs as a serial
delivery history.
Git owns commits and branches. GitHub owns PR heads, checks, and merge. Local
state records enough evidence to resume the workflow safely.

```bash
lf checkout INF-123
lf --task INF-123 implement
lf commit -m "parser: accept nested groups"
lf pr publish --title "Parser: accept nested groups"
lf land -c
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
| commits, branch ancestry, sync state | Git |
| PR head, required checks, merge | GitHub |
| landing supervision and repair admission | exact recorded PR head plus landing generation |

Task types live under [`work/task/`](../../rust/loopflow/src/work/task/). Operational Git,
PR, CI, and landing workflows live under [`ops/`](../../rust/loopflow/src/ops/).
The exact landing fence is modeled in
[`pr_landing.rs`](../../rust/loopflow/src/pr_landing.rs).

## Create or reuse the worktree

`lf checkout` resolves one existing Linear Issue inside one Project and
creates or reuses its managed worktree and first serial PR record. It starts no
execution. `lf flow start` uses the same substrate and additionally advances the
declared Task flow. The repository identity—not the caller's
current directory spelling—selects the Git directory and sibling worktree
namespace.

Only Task Work owns a delivery worktree. Project and Wave processes coordinate;
they do not edit product files in substitute worktrees.

A dependent change that must begin before its parent merges uses another Task:

```bash
lf --task INF-124 flow start --stack-on INF-123
```

The child records its fork point and targets the parent's active PR branch.
Its first commit, `Clear inherited scratch`, removes the parent's notes. Parent
updates keep the child's entire `scratch/` tree, including deleted files; the
parent's notes remain on the parent branch. Even a child with only this cleanup
commit merges updates instead of resetting onto the parent's scratch.
After the parent merges, `lf task sync` merges current main using the recorded fork
as the comparison base. Child edits and original commit identities survive squash
landing without replay.
The parent Task does not hold two simultaneously open PRs.

## Commit and publish

```bash
lf commit -m "parser: accept nested groups" # local checkpoint
lf pr open                             # prepare a draft and open its page
lf pr publish                          # ready for review
lf land                                # request auto-merge and return
```

`publish` creates or refreshes the current PR without integration. A completed
merge, including one made outside `lf`, advances the recorded Task base to the
actual merge base when the old base is its ancestor. Publication, submit and
landing share that ancestry check; unrelated or divergent bases still fail.

`land` merges current main, clear merge-time scratch state, verify once,
and push the exact head. Branch commits and merge resolutions retain their identities.
GitHub squash-merges the final PR tree into one commit on main. `land` requests
GitHub auto-merge and returns. `submit` performs the
same preparation but leaves the exact-head merge to a person. These delivery
commands inspect Task delivery state when present; they do not require a live
Task worker or certify that a particular Flow ran.

Scratch cleanup selects landing candidates for this repository's
[hosted CI](../../TESTING.md); PR readiness alone does not select CI.

Final preparation keeps the existing PR title and body when its published head
matches the local head. Explicit copy and valid gate output take precedence;
unpublished changes still generate fresh copy. Task merge-disposition text is
updated after that selection. Preparing an unchanged published PR needs no agent
just to rewrite its description.

Repeating `land` on a clean, already armed exact head resumes the
existing request, including standalone PRs. It preserves the commit, merge
queue position, and CI. Explicit standalone title/body edits update only those
fields; omitted copy is preserved. Dirty source or a new local commit still
prepares and publishes a replacement head. Task requests must also match the
requested completion/continuation disposition.

`lf pr open` is the presenting verb; it opens the review surface after
publishing. Headless Task flows use publish or land.

## Serialize the exact Git races

Loopflow uses advisory OS file locks beneath the repository's absolute Git
directory:

```text
<absolute-git-dir>/loopflow/rebase-owner.json
<absolute-git-dir>/lf-pr-mutation.lock
<absolute-git-dir>/lf-pr-landing.lock
```

The open file descriptor is authority. JSON is a readable receipt. Process
death releases the kernel lock even if metadata remains.

### Sync operation

Provider launches receive no durable Git writer token. Independent agents may
coexist in the shared worktree. A sync locks `rebase-owner.json` for the Git
merge lifetime; new agent launches refuse while that operation is live. The
existing receipt filename stays stable so older executables share the same lock.

| Concurrent work | Result |
| --- | --- |
| agent + independent agent | allowed; use distinct output paths |
| read/build/test + agent | allowed |
| live sync + new agent launch | blocked |
| sync + its exact recovery child | allowed |
| stale sync record without a kernel lock | adopted or removed through the sync path |
| unowned or stopped merge + agent launch | allowed; continuation adopts the existing merge |

The sync owner authorizes only its exact sequencer and recovery child. It does
not make a provider the worktree owner or serialize ordinary edits, conversation
recording, tests, or planning writes.

A supervisor-started merge can be handed to an agent in the same checkout.
`lf task sync --continue --adopt` claims a raw merge after resolution. For a stopped
Loopflow sync, ordinary `--continue` retains the saved branch and pinned target.
Launching the agent neither adopts the operation nor publishes the result.

### PR mutation

`lf-pr-mutation.lock` covers only Task PR/head transitions: publication,
repair, range healing, merge request, settlement, and serial rotation. A
second mutation fails fast while that exact section is held.

### Landing supervision

`lf-pr-landing.lock` follows the supervisor's actual operation lifetime. A
replacement waits while an old observation or repair is still running, even
when its async waiter has been canceled. The file contains no state; the
existing landing generation still fences database writes. Once the old
operation returns, the replacement observes GitHub before deciding what to do.
Joining an active landing updates the requested head and disposition while
retaining the supervisor's checkout. Resuming a blocked landing can select the
caller's current checkout.

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
    merge   repair under supervisor G
              |
              v
           observe current head --> fresh check evidence
```

A landing supervisor never transfers green checks from one head to another.
A failure may need several repairs, including on the same head. Incidents
record responses; the supervisor owns execution. A moved head requires a new
observation and check set. GitHub remains the final merge authority.

An auto-merge request targeting a merge queue keeps waiting when its base
advances, including before queue entry. Its original-head CI still receives
repair, and a real conflict still requires integration. Once GitHub queues the
PR, landing and release wait for the queue's integrated-commit proof instead
of interpreting the original head's mergeability or checks. Removing the
request restores ordinary handling; only an authoritative GitHub merge
finishes the landing.

Landing and release read PR state, head, merge commit, auto-merge request, and
queue membership in one GitHub response. A merge therefore takes precedence
over its removed request without combining an earlier open state with a later
request read. Missing or partial responses cannot settle a landing.

`PrLanding` owns the supervisor generation. `LandingSupervisor` names the
process, placement, and heartbeat used both to claim and to retain that
ownership. Incidents retain response provenance and timing across generations.

Required gates and repair details come from one paginated GitHub check set for
the observed PR head. Every page must still name that head. A moved head leaves
checks unknown until the caller reobserves; an unreadable page cannot supply a
partial success. Repeated jobs retain their newest result within each workflow
and event, while legacy status contexts keep their own identities.

Rerun `lf land` after resolving a blocker to renew the exact-head request.
The release watcher retains supervisor generations and repair conclusions in
conversation history. `lf mon show SESSION --final` inspects a conclusion;
the finite landing CLI does not keep a polling process alive.

Watched repairs return `published` or `blocked` with a summary in their existing
final answer. A blocked result names the required action. The watcher observes
GitHub before returning it, so an already-merged PR still finishes successfully.
That reconciliation happens immediately after the repair returns. Pending CI
keeps its normal polling interval, and a repeated repair waits for that interval
and a fresh observation before starting.
Provider exit code zero alone does not mean the repair succeeded.

After an observed merge, the recorded bare-land disposition leaves the Task open.
`lf land -c` completes the Task. `lf land --next <slug>` rotates the
serial chain to a new branch from fetched main.

## Failure and recovery

- An interrupted provider turn does not discard the worktree or PR chain.
- A failed check is GitHub evidence, not a completed local transition.
- A crashed sync keeps Git's sequencer state; explicit recovery adopts it
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
- Conversation identity does not grant Git or PR mutation authority.
- Repair and merge decisions are fenced by exact PR head evidence.

## Next

[Planning →](planning.md) separates the Task objective from exact Flow positions.
[Homes and processes →](homes.md) owns the machine and process boundaries around
delivery.
