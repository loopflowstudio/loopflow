# Delivery

A Task binds durable planning to one managed checkout and zero or one PR.
Checkout placement owns branch and base independently of publication. Historical
multi-PR records remain read-only; new delivery never appends a successor.
Git owns commits and branches. GitHub owns PR heads, checks, and merge. Local
state records enough evidence to continue delivery safely.

```bash
lf checkout INF-123
lf --task INF-123 implement
lf commit -m "parser: accept nested groups"
lf pr publish --title "Parser: accept nested groups"
lf land --wait-and-fix
lf task follow-up INF-123 --none 'No accepted obligations remain'
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
                             file follow-ups or record none
                                          |
                                    complete Task
```

| Object | Authority |
| --- | --- |
| Task directive and Project membership | Local planning; observed conflicts adopt Linear when connected |
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
runs a fresh Flow there. The repository identity—not the caller's
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
Its first commit, `Clear inherited scratch`, removes the parent's notes. Parent
updates keep the child's entire `scratch/` tree, including deleted files; the
parent's notes remain on the parent branch. Even a child with only this cleanup
commit merges updates instead of resetting onto the parent's scratch.
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
lf ci watch                            # repair failed landings while it runs
```

`publish` creates or refreshes the Task's sole optional PR without integration.
The publishing intent is durable before the remote call, so retries resolve the
same PR. Reopening a Task or closed PR never grants a second PR slot.

`arm` and `land` merge current main, clear merge-time scratch state, verify once,
and push the exact head. Branch commits and merge resolutions retain their identities.
GitHub squash-merges the final PR tree into one commit on main. `arm` and `land` request
GitHub auto-merge, record the landing, and return. Success means handoff;
`lf task reconcile` checks recorded repository landings once;
`lf pr reconcile` uses its delivery-only path. `lf cron sync --repo` installs the
finite minute check on this Machine. Neither repairs CI; `lf ci watch` does. `submit` performs the
same preparation but leaves the exact-head merge to a person. These delivery
commands inspect Task delivery state when present; they do not require a running
Task Flow or certify that a particular Flow ran.

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
prepares and publishes a replacement head. Task completion remains separate:
verified merge and a recorded follow-through disposition are required.

`lf pr open` is the presenting verb; it opens the review surface after
publishing. Headless Task flows use publish or land.

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
 return return settle   record the incident; waited landing, the CI
                        watcher or a release reserves one repair under G
```

The next repository tick or `lf pr reconcile` repeats this from fresh evidence.
Those checks record a failure and return. `lf land --wait-and-fix`, `lf ci watch`
and a release's own landing can start a repair through this same check.

### CI watcher

```text
lf ci watch ──60 s, jitter──> REST, If-None-Match (304 costs no quota)
      |                         pulls?state=open
      |                         commits/{head}/check-runs, commits/{head}/status
      |                         rules/branches/{base} (required checks, hourly)
      v
 required check failed on a PR with a recorded landing?
      |  yes                                  | no landing, or no Task
      v                                       v
 the landing check above                   report only
 (lock, generation, confirm, reserve)
```

`lf ci watch` is one repository-wide program with one job. It reads the gate
with the same required-check projection as the landing check, then hands a
failing landing to that check, which confirms the failure against GitHub, stays
silent for a queued PR, and reserves the incident's one repair. The landing
lock, landing generation and incident reservation are the claim, so waited
landing, a watcher and a release cannot repeat a fix. Scheduled checks only
observe. The watcher fills the
incident's `provider_completed_at` from the check's `completed_at`, which makes
detection latency measurable in `lf ci`.

It backs off to five minutes after a degraded pass and waits for the reset when
fewer than 500 core requests remain. Every recorded landing is also checked
every five minutes, so a conflicting or stale head and a CI timeout are repaired
without any check failing.

The command runs three ways: in a terminal, as a launchd service
(`lf ci watch --install`), and from Loopflow Desktop, which starts it for each
open repository and stops it on quit. `<git-dir>/lf-ci-watch.lock` admits one
live watcher per repository; a second copy stands by and takes over when the
first exits. `<git-dir>/loopflow/ci-watch.json` carries its last poll, the PRs
it saw and the repairs it started, read by `lf ci watch --status`. Correctness
never depends on it: waited landing repairs its own PR. A returned bare landing
waits for another waited landing or a watcher to repair failures.

A check never transfers green checks from one head to another. A failure is
confirmed by a second observation before repair. One incident (head, failed check set and provider check URLs) owns one repair
Session. Admission reserves that Session and launcher Process atomically before
starting its detached worker. A dead launch may retry once using the same
conversation and native history; a completed blocked repair retains its outcome.
The same incident failing again waits until the head or evidence changes.
Required integration is actionable even with green checks. Pending and missing
checks retain their first-observed clock; provider attempt changes reset that
clock without resetting the timeout rerun allowance. A failed read is never evidence
of CI failure or merge. GitHub remains the final merge authority.

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

`PrLanding` owns the generation. `LandingSupervisor` names the process,
placement, and heartbeat of the check currently holding the claim. Incidents
retain response provenance and timing across generations.

Required gates and repair details come from one paginated GitHub check set for
the observed PR head. Every page must still name that head. A moved head leaves
checks unknown until the caller reobserves; an unreadable page cannot supply a
partial success. Repeated jobs retain their newest result within each workflow
and event, while legacy status contexts keep their own identities.

A blocked landing stays observable: later checks still settle its merge, and
checks that stop failing clear the block. Rerun `lf arm` or `lf land`
after resolving a blocker to resume under a fresh generation, including when
the SHA has not changed. Use `lf history show SESSION --final` to inspect a repair's
conclusion.

Repairs return `published` or `blocked` with a summary in their final answer.
A blocked result names the required action. Provider exit code zero alone does
not mean the repair succeeded.

A PR closed without merging ends its landing unsettled. Merge evidence is
recorded before Task settlement; a failed local or Linear settlement keeps the
landing pending and the next check retries it.

After verified merge the Task shows **Merged · Follow-through pending**.
`ship` runs gate, `land --wait-and-fix`, then follow-through. Waited landing uses the
existing observation and CI repair path every 15 seconds for at most 30 minutes,
releasing its lock between reads. Repair admission exempts the calling command
and its recorded ancestors, which wait for it; unrelated live or unresolved
Processes still prevent editing. Unanswered turns alone are history, not execution.
The same incident reservation deduplicates a concurrent watcher. Timeout
retains intent and returns held (exit 3), propagated through the Flow without
automatic retry;
interruption retains intent and returns stopped (exit 130).
Neither ordinary reconciliation nor a manual GitHub merge manufactures a verdict.
Keep the checkout available for the finishing step.

Follow-through reads the accepted brief, merged PR copy and delivery evidence.
It uses `lf task follow-up` to file or link actual Tasks for accepted later
obligations, or records none needed with a reason. Filing intent retains a stable
child identity, destination Project and exact payload before local creation;
uncertain responses and chapter rotation cannot select a replacement destination.
The common planning writer creates the Task; its confirmed local link permits
the filed disposition. Foreground planning sync delivers optional Linear creation
and related-issue links independently, including after source completion.
Existing unresolved keep-open records require scope conversion, never silent
completion or remote issue creation during migration.

`lf task complete` settles status without moving the Workflow. Movement to
`end` persists a completion request in the same transaction as arrival, then
tries completion. Failure retains both arrival and the reason; operators retry
completion alone. Accepted provider status changes clear superseded requests
without touching Workflow or Process history. Reads never execute the trigger. A Task with a PR needs merge
and a durable none/filed disposition; its follow-up Tasks need not be complete.
A PR-less Task may finish with retained files and commits, without a landing
ceremony. Completion is idempotent, including when the enclosing Flow later
arrives at end; planning writeback remains retryable.

A stopped finishing Flow is recovered by the next Task/Wave operation. Inspect
all associated live work, then run `finish-delivery` or repeat completion if the
disposition is already durable. Do not replay gate or re-arm a merged PR.
Provider completion does not block first filing or this recovery: an unresolved,
merged delivery retains its checkout and admits a Flow containing only
`follow-through`, regardless of the Flow's name. Normal Flow/Workflow launches
stay closed on completed Tasks. A resolved disposition admits retries of saved
filings but no new obligation or finishing Flow; recovery never reopens planning.
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
- A Task has zero or one PR. Migrated multi-PR history is read-only; no current
  operation adds a successor. An open or closed-unmerged PR cannot count as done.
- Simultaneously open dependent work belongs to another stacked Task.
- Git and GitHub remain authority for their own objects.
- Locks serialize exact local races, not all activity.
- Conversation identity does not grant Git or PR mutation authority.
- Repair and merge decisions are fenced by exact PR head evidence.

## Next

[Planning →](planning.md) separates the Task objective from exact Flow positions.
[Machines and processes →](machines.md) owns the machine and process boundaries around
delivery.


## Scheduled Task admission

Task owns its CI-repair hold and unchanged-failure retry count. A repository
check never launches, continues or chooses a Flow. Session input reservations
share a short checkout admission lock; automatic repair admission re-reads Hold
under that boundary. Unknown process identity and live unrelated work defer
admission.

The repository lock skips overlapping observations. Per-Task and per-PR claims
still fence direct callers. A pass budgets 45 seconds and each Task or external
operation 10 seconds; deferred work stays visible in check output. launchd's
calendar entries run each minute and coalesce sleep into a wake-time check.
The detached tmux launch starts a separate process group, including when it must
start the tmux server. No provider turn runs inside the tick.

Desktop consumes the Rust automation projection: installed/enabled state,
last successful and failed receipts, selection and blockers. A missing receipt
is unknown coverage. Disabling removes the job and prevents its later admissions;
it leaves running work and explicitly requested GitHub merges intact.
