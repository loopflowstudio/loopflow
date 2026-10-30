# Delivery

A Task binds durable planning to one managed checkout and one current PR, when
published. Checkout placement owns branch and base independently of publication.
Publishing after a PR settles retains it as read-only history and selects a new
current PR. Recommend follow-up Tasks for distinct outcomes; continued work in
the same checkout needs no special command.
Git owns commits and branches. GitHub owns PR heads, checks, and merge. Local
state records enough evidence to continue delivery safely.

```bash
lf checkout INF-123
lf --task INF-123 implement
lf commit -m "parser: accept nested groups"
lf pr publish --title "Parser: accept nested groups"
lf land --wait-and-fix
lf task complete INF-123
```

## Delivery flow

```text
Local Task plan (optional Linear sync)
    |
    v
Task Work ----> managed worktree ----> commits
    |                                     |
    |                                     v
    +--------------------------------> GitHub PR
                                          |
                                 checks / repair / merge
                                          |
                             optional ordinary Task creation
                                          |
                                    complete Task
```

| Object | Authority |
| --- | --- |
| Task directive and Project membership | Local planning; when connected, the change sent last wins |
| managed checkout placement and optional PR state | Task delivery records plus resolved Git repository |
| commits, branch ancestry, sync state | Git |
| PR head, required checks, merge | GitHub |
| landing checks and repair admission | exact recorded PR head plus landing generation |

Task types live under [`work/task/`](../../rust/loopflow/src/work/task/). Operational Git,
PR, CI, and landing workflows live under [`ops/`](../../rust/loopflow/src/ops/).
The exact landing fence is modeled in
[`pr_landing.rs`](../../rust/loopflow/src/pr_landing.rs).

## Create or reuse the worktree

`lf checkout` resolves a saved Task inside one Project and
creates or reuses its managed worktree without creating a PR. Saved planning
works without Linear; an unknown provider alias needs initial acquisition.
It starts no execution. `lf task run ISSUE` uses the same substrate and additionally
runs a fresh Graph there. The repository identity—not the caller's
current directory spelling—selects the Git directory and sibling worktree
namespace.

Only Task Work owns a delivery worktree. Project and Wave processes coordinate;
they do not edit product files in substitute worktrees.

A dependent change that must begin before its parent merges uses another Task:

```bash
lf checkout INF-124 --stack-on INF-123 --design scratch/child-design.md
lf task run INF-124
```

The parent must have a published PR; a draft suffices. The child can start and
publish before that PR merges. It records its fork point and targets the parent's
PR branch. Prepare a self-contained child-specific design and transfer it with
`--design`; its receipt retains source Task, commit and content hash. Identical
retries do not reset the child; changed content at an occupied destination is a
handoff conflict, with both versions retained.
The child inherits existing scratch files. Parent updates merge normally;
creating a child does not delete inherited files or add a cleanup commit.
After the parent merges, `lf sync` merges current main using the recorded fork
as the comparison base. Child edits and original commit identities survive squash
landing without replay.
The child retains its Task, PR, branch and Session identity and its own design.

## Commit and publish

```bash
lf commit -m "parser: accept nested groups" # local checkpoint
lf pr open                             # prepare a draft and open its page
lf pr publish                          # ready for review
lf arm                                 # request auto-merge and return
lf land                                # hand off delivery and return
lf pr reconcile                        # check recorded landings once
lf land --wait-and-fix                 # land, repairing failed CI until merge
```

`publish` creates or refreshes the current PR. After settlement, continued work
starts a new PR and retains the previous delivery. The publishing intent is
durable before the remote call, so retries resolve the same PR. Already-delivered
history is removed from the new range; unrelated upstream changes still need sync.

`open` and `publish` resolve the comparison base from HEAD's current
merge-base with the selected upstream. A reset, rebase or rewritten main cannot
make an old base veto publication. All branch commits remain intact, including
inherited dependencies; the PR shows GitHub's actual range. Publication does not
trim commits based on a saved Task boundary. Unrelated histories and empty ranges
still cannot form a PR. Root Tasks save the resolved base; stacked Tasks retain
their integration fork until explicit sync so squash-parent comparison preserves
child edits.

`submit`, `arm` and `land` clear added scratch files during preparation,
merge current main, verify once,
and push the exact head. Branch commits and merge resolutions retain their identities.
GitHub applies the repository’s allowed merge method or merge queue policy. `arm` and `land` request
GitHub auto-merge, record the landing, and return. Success means handoff;
`lf pr reconcile` checks recorded repository landings once and repairs nothing;
the waiting landing (`--wait-and-fix`, `lf ship`) does. `submit` performs the
same preparation but leaves the exact-head merge to a person. These delivery
commands inspect Task delivery state when present; they do not require a running
Task Graph or certify that a particular Graph ran.

Scratch cleanup selects landing candidates for this repository's
[hosted CI](../../.github/CI.md); PR readiness alone does not select CI.

Final preparation keeps the existing PR title and body when its published head
matches the local head. Explicit copy and valid gate output take precedence;
unpublished changes still generate fresh copy. Task merge-disposition text is
updated after that selection. Preparing an unchanged published PR needs no agent
just to rewrite its description.

Repeating `land` on a clean, already armed exact head resumes the
existing request, including standalone PRs. It preserves the commit, merge
queue position, and CI. Explicit standalone title/body edits update only those
fields; omitted copy is preserved. Dirty source or a new local commit still
prepares and publishes a replacement head. Task completion remains separate:
verified merge is required.

`lf pr open` is the presenting verb; it opens the review surface after
publishing. Headless Task Graphs use publish or land.

## Serialize the exact Git races

Loopflow uses advisory OS file locks beneath the repository's absolute Git
directory:

```text
<absolute-git-dir>/loopflow/rebase-owner.json
<absolute-git-dir>/lf-pr-mutation.lock
<absolute-git-dir>/lf-pr-landing.lock
<absolute-git-dir>/lf-ci-watch.lock
```

The open file descriptor is authority. JSON is a readable receipt. Process
death releases the kernel lock even if metadata remains.

### Sync operation

Provider launches receive no durable Git writer token. Independent agents may
coexist in the shared worktree. A sync locks `rebase-owner.json` from its merge
through publication; fetching and planning happen before, unfenced. New agent
launches refuse only while that lock is held, and the refusal names the holder,
its target and how long it has held the lock:

```text
refusing to mutate Git: lf sync pid 4121 has held the sync lock for 14 min (feature onto origin/main); retry when it stops, or kill pid 4121 to continue
```

A live sync keeps the lock however long it runs; there is no Loopflow timeout.
The push passes `http.lowSpeedLimit`/`http.lowSpeedTime` so Git aborts a
stalled HTTP transfer. The existing receipt filename stays stable so older
executables share the same lock.

| Concurrent work | Result |
| --- | --- |
| agent + independent agent | allowed; use distinct output paths |
| read/build/test + agent | allowed |
| live sync (merge or push) + new agent launch | blocked, naming the holder |
| sync stopped on conflicts or errors + agent launch | allowed; continuation adopts the existing merge |
| sync whose conflict resolver is running + agent launch | allowed; the sync retakes the lock to verify and push |
| stale sync record without a kernel lock | adopted or removed through the sync path |

The lock does not make a provider the worktree owner or serialize ordinary
edits, conversation recording, tests, or planning writes. Verification checks
only what the push publishes: no active merge, the expected branch, the pinned
target and original head as ancestors, and no unmerged paths. Another writer's
tracked edits beside the resolution leave the sync verified; a path that writer
stages with `git add` while a resolver works lands in the merge commit. That
follows from allowing concurrent writes and loses nothing.

A supervisor-started merge can be handed to an agent in the same checkout.
`lf sync --continue --adopt` claims a raw merge after resolution. For a stopped
Loopflow sync, ordinary `--continue` retains the saved branch and pinned target.
Launching the agent neither adopts the operation nor publishes the result.

### PR mutation

`lf-pr-mutation.lock` covers only Task PR/head transitions: publication,
repair, range healing, merge request, and settlement. A
second mutation fails fast while that exact section is held.

### Landing checks

`lf-pr-landing.lock` follows one check's actual operation lifetime. A
contending check returns immediately while an observation or repair is still
running, even when its async waiter has been canceled. The file contains no
state; the existing landing generation still fences database writes. Each
check observes GitHub before deciding what to do and releases its claim when it
returns. Joining an active landing updates the requested head and disposition
while retaining the running check's checkout. Resuming a blocked landing can select the
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
read required checks on H1 once
          |
   +------+------+----------+
   |      |      |          |
 pending pass  merged      fail
   |      |      |          |
 return return settle   record the incident; waited landing or a
                        release reserves one repair under G
```

`lf pr reconcile` repeats this from fresh evidence, records a failure and
returns. Only a waiting landing (`lf land --wait-and-fix`, the ship process of a
shipping Task) and a release's own landing start a repair through this check.
The landing lock, landing generation and incident reservation are the claim, so
none of them repeats a fix. A returned bare landing is repaired by nothing.

A repair is an ordinary detached `lf -b skill ci-fix` run in the landing's
checkout; the incident's reservation names its owner Process. Any later check
settles an exited run from its Session's final answer: `published`, `blocked`
with a reason, or no conclusion. A run that ends without publishing ends that
attempt; the next attempt starts after a backoff, with the earlier summaries.

### Recovery

There is no schedule. Any lf read of this Machine's Tasks or processes
(`lf task status`, `lf task list`, `lf ps`, `lf wave status`, Desktop's planning
reads) first settles AgentProcesses whose lf died and starts the ship process of
each shipping Task checked out here again when it has exited. A Machine nothing
reads recovers on its next read.

A check never transfers green checks from one head to another. A failure is
confirmed by a second observation before repair. One incident (head, failed check set and provider check URLs) owns one repair
Session. Admission reserves that Session and launcher Process atomically before
starting its detached worker. A dead launch may retry once using the same
conversation and native history; a completed repair retains its outcome. When
the same incident still fails, its next attempt (`<identity>#n`) starts after a
backoff of 1, 2, 4… minutes, at most 30, and its prompt carries the earlier
attempts' summaries. Nothing hands the landing back to a person.
Required integration is actionable even with green checks. Pending and missing
checks keep waiting without synthetic failures or elapsed-time reruns. A failed
read is never evidence of CI failure or merge. GitHub remains the final merge authority.

An auto-merge request targeting a merge queue keeps waiting when its base
advances, including before queue entry. Its original-head CI still receives
repair, and a real conflict still requires integration. Once GitHub queues the
PR, landing and release wait for the queue's integrated-commit proof instead
of interpreting the original head's mergeability or checks. Only an
authoritative GitHub merge finishes the landing.

When a PR loses its merge request, landing and release read its latest
merge-request event. A person's dequeue or disabled auto-merge stops them,
naming who and when; `lf arm` resumes. Every other removal (a queue eviction,
a repair's own disarm, no event) is a CI incident, and the repair re-arms the PR.

Landing and release read PR state, head, merge commit, auto-merge request, and
queue membership in one GitHub response. A merge therefore takes precedence
over its removed request without combining an earlier open state with a later
request read. Missing or partial responses cannot settle a landing.

`PrLanding` owns the generation. `LandingSupervisor` names the process,
placement, and heartbeat of the check currently holding the claim. Incidents
retain response provenance and timing across generations.

Required gates and repair details come from one paginated GitHub check set for
the observed PR head. Every page must still name that head. A moved head leaves
checks unknown until the caller reobserves; an unreadable page cannot supply a
partial success. Repeated jobs retain their newest result within each workflow
and event, while legacy status contexts keep their own identities.

An aggregate required check such as `tests-result` has no check run until every
job it `needs` finishes. The checkout's `.github/workflows` names each required
check's producing job and its transitive `needs`; a failed job in that set fails
the gate at once, matrix runs included (`rust-test (library, 3, 5)`). Optional
checks outside it keep their meaning. A required check with no producer in the
checkout is judged when its own run reports. The watcher hands any failed check
to this one judgment.

A blocked landing stays observable: later checks still settle its merge,
checks that stop failing clear the block, and a waited landing keeps checking.
Use `lf history show SESSION --final` to inspect a repair's conclusion.

Repairs return `published` or `blocked` with a summary in their final answer.
A blocked result ends that attempt; the next one starts after its backoff.
Provider exit code zero alone does not mean the repair succeeded.

A PR closed without merging ends its landing unsettled. Merge evidence is
recorded before Task settlement; a failed local or Linear settlement keeps the
landing pending and the next check retries it.

`ship` runs gate and `land --wait-and-fix`. Waited landing uses the existing
observation and CI repair path every 15 seconds until verified merge, closure
or interruption, releasing its lock between reads. A blocked check or failed
read is printed once and checked again. Repair
admission exempts the calling command and recorded ancestors waiting for it;
unrelated live or unresolved Processes still prevent
editing. Unanswered turns alone are history, not execution. The same incident
reservation deduplicates a concurrent watcher. Elapsed time never terminates
observation, repair admission or a live repair. A PR closed without merging
returns held (exit 3); interruption retains intent and returns stopped (exit 130).
Neither ordinary reconciliation nor a manual GitHub merge invents a verdict.

Independently accepted later work is an ordinary Task plus a linking comment.
Keep a saved Task when its comment fails; retry only the comment. Synchronization
uses the same Task and comment writers as other Work. Historical filing events
stay intact: private recovery converts reserved children and notes, retaining
unknown provider creations and relations with their original IDs and inputs.
These receipts carry no completion or process authority.

`lf task complete` settles status without moving the Task's graph. Reaching
`end` persists a completion request in the same transaction as arrival, then
tries completion. Failure retains both arrival and the reason; operators retry
completion alone. Accepted provider status changes clear superseded requests
without touching graph position or Process history. Reads never execute the trigger. A Task with a PR needs merge
evidence; its follow-up Tasks need not be complete.
A PR-less Task may finish with retained files and commits, without a landing
ceremony. Completion is idempotent, including when the enclosing Graph later
arrives at end; planning writeback remains retryable.

After a stopped Graph, inspect associated live work and retry only the unfinished
operation. A completed Task can receive comments without reopening execution.
There is no special finishing Graph or new-work exception for completed Tasks.
Do not replay gate or re-arm a merged PR to finish a comment or completion request.
Dated follow-ups return on the owning Wave's next pass, including unstarted ones;
unattended execution needs a concrete check already authorized in the brief.
Filing installs no schedule. Without an installed Wave schedule there is no
automatic wake, and time passing proves no accepted outcome.

Task decisions never settle Session turns or process exits. Checkout cleanup and
process control keep their own evidence and authority. Historical uncertainty
cannot veto completion or cancellation; it can require retaining the checkout.

## Failure and recovery

- An interrupted provider turn does not discard the worktree or PR identity.
- A failed check is GitHub evidence, not a completed local transition.
- A crashed sync keeps Git's sequencer state; explicit recovery adopts it
  with fresh operation identity.
- A crashed PR mutation is retried by resolving current Git and GitHub truth
  inside the same narrow lock.
- A Task moved to another Linear Project fails closed before automated commit,
  push, publication, merge request, or completion.

## Boundary contracts

- One Task has one active remote branch. A checkout tracking it identifies the
  Task; the stored worktree path is placement.
- A Task shows one current PR and retains earlier deliveries as read-only history.
  Only a settled PR can be superseded. New publication clears earlier completion
  intent and reopens a completed Task. An earlier merge cannot finish the new PR.
- Simultaneously open dependent work belongs to another stacked Task.
- Git and GitHub remain authority for their own objects.
- Locks serialize exact local races, not all activity.
- Conversation identity does not grant Git or PR mutation authority.
- Repair and merge decisions are fenced by exact PR head evidence.

## Next

[Planning →](planning.md) separates the Task objective from exact Graph positions.
[Machines and processes →](machines.md) owns the machine and process boundaries around
delivery.


## Scheduled Task admission

Task owns its CI-repair hold and unchanged-failure retry count. A repository
check never launches, continues or chooses a Graph. Session input reservations
share a short checkout admission lock; automatic repair admission re-reads Hold
under that boundary. Unknown process identity and live unrelated work defer
admission.

The repository lock skips overlapping observations. Per-Task and per-PR claims
still fence direct callers. Delivery observations have no elapsed-time cutoff;
concurrent checks skip the landing while its current observation owns the lock.
The detached transport owner starts a separate process group. Startup waits for
its acknowledgement without an elapsed-time cutoff. No provider turn runs inside
the tick.

Desktop consumes the Rust automation projection: per-Task CI repair holds and
unsettled deliveries. Scheduling the check is the person's own crontab line.
