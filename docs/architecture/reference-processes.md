# Locks, processes and Machines

Part of the [Architecture Reference](../architecture-reference.md).

## OS locks and allowed contention

Loopflow uses advisory OS file locks for exact local critical sections. The
open file descriptor holds authority; the JSON file is a readable receipt.
Process death releases the kernel lock even if the receipt remains, so the next
operation can clean or explicitly adopt stale metadata.

### Git mutation and sync locks

For a Git worktree, `absolute_git_dir` selects the real Git directory, including
the linked-worktree case. Sync coordination lives beneath it:

```text
<absolute-git-dir>/loopflow/rebase-owner.json
```

Agent conversations receive no worktree writer token. Commit, PR mutation, restart
checkpointing, and land take short OS-held locks only around their exact Git
mutation. Independent agents may edit and run concurrently; the shared
worktree remains the durable blackboard.

A sync takes an exclusive lock on `rebase-owner.json` only while its Git
merge, abort or commit steps run, and while it verifies and pushes the result.
New agent launches refuse while that sync lock is live. A merge stopped on
conflicts releases the lock before its `sync-conflicts` resolver launches, so a
stuck resolver fences no one; the resolver's `lf sync --continue` adopts the
stopped operation's record like any other continuation. A raw sync without a
readable record can be adopted only through `--adopt`, which mints a new id.

Unowned and stopped Git operations do not block agent launch. A stopped operation's
receipt survives so continuation can adopt its pinned target. Raw merges use
`lf sync --continue --adopt`; launching an agent grants no operation ownership.

Therefore:

- agent + agent is allowed;
- reader/build/test + agent is allowed;
- live sync + new independent agent launch is blocked;
- a running agent does not reserve the worktree against sync;
- a stopped or resolving sync admits agents, and one continuation adopts it;
- stale JSON with no OS lock is not a live owner;
- raw Git outside Loopflow is not compelled by these advisory locks.

### Short mutation and machine locks

`<absolute-git-dir>/lf-pr-mutation.lock` serializes only Task PR/head mutation
sections; a second such operation fails fast while the first guard is alive.
Wave locator locks serialize relocation filesystem ownership. A
`<store>.migration.lock` serializes backup plus schema application. The current
promotion operation holds `$HOME/.lf/promotion.lock` exclusively for its full
upgrade transaction. These locks do not turn a conversation ID into authority.

## Process ownership and control

Causal ancestry and conversation identity are not process ownership. Local child
handles permit the spawning process to control its child. Cross-process control
requires exact PID/start identity and the appropriate native scope, claim and
AgentProcess record. Revalidate that evidence before every signal.

A headless AgentProcess lives only while an LfProcess attached to it does. It
runs in its own process group with a watchdog that holds the read end of a
lifeline; each attached LfProcess holds a write end until it ends. Return, a
signal, a panic and SIGKILL all close it, and the watchdog then terminates the
group. Only Codex names its lifeline, so only a Codex takeover can hold the same
lifeline first and leave the AgentProcess alive; takeover coverage for other
providers and foreground terminals remains open. A saved endpoint alone is not
liveness; a missing endpoint alone is not death. An LfProcess that replaces a
dead one ends the AgentProcess it left by recorded PID and start time, then
resumes the saved native thread on a new AgentProcess; the turn that was in
flight is not recovered. Nothing authorizes terminating a shared AgentProcess to
recover one thread.

Pi's native bash executor creates detached tool groups. Its launch extension
wraps the built-in executor with a per-command FIFO writer held by Pi. Before
user commands run, `lf __context-block --moment tool-group` verifies that the current detached group
is a direct child of the live provider, then starts the same group watchdog.
Normal completion, abort and provider death close the writer. Custom extension
tools retain their own execution ownership; there is no tool PID registry or
postmortem discovery of signal targets.

One exception to local-handle control exists. The scheduled repository check and
`lf mon prune` terminate an AgentProcess that outlived its attachment: the recorded PID and
start time still name a live provider server leading its own process group, and
every Session naming it is headless with an attached LfProcess that is provably dead.
Unknown evidence, a live attachment, an interactive Session or a reused PID leaves it
running. The Session records the exit and why, and its AgentProcess identity is cleared.
Task or Wave membership still grants no authority to stop anything.

## Machines and process topology

```text
Loopflow.app / shell / external harness
                 |
                 v
                lf ---------------- Linear / GitHub / provider auth
                 |
       planning SQLite + repository/Git
                 |
                 v
          Task CLI ----- exact invocation claim
                                              |
                                              v
                                      provider harness
                                              |
                                              v
                                      LfSession native history
```

Wave operations are finite conversations. Tasks drive their selected invocation; finite
`lf pr reconcile` checks observe delivery. Crossing Machines is an explicit `lf --machine` hop
whose target proves its Machine identity.

### Multi-Machine ownership and execution

`MachineId` is stable identity; its SSH route is replaceable. Which Machine runs a
person's Wave work is that person's choice, never repository content:

```bash
lf machine use mini infrastructure            # this Wave's new Tasks and its Session
lf machine use laptop infrastructure --session  # only the Wave Session
lf --repo loopflow machine use mini --tasks    # only the repository's new Tasks
lf machine list                               # each Machine with the defaults naming it
```

The setting lives in Machine-local SQLite `scope_defaults`, keyed by Wave or repository, and is copied to the
person's other added Machines over `lf --machine`; an unreachable Machine gets a
warning and a later rerun. Resolution for an unowned Task walks its Wave, its parent
Waves, its repository, then this Machine, so single-Machine use needs no setup. A scope's Session Machine is a separate setting with the same
resolution (`--session`; `--tasks` sets only the Tasks'). Setting it moves the
Wave primary Session to the new Machine with its native provider history. `lf resume --wave <w>`, bare resumes that
resolve to a Wave or the repository, and Desktop's Wave Session route to the Session
Machine. A Task's Machine is captured in its claim: changing the Wave Machine moves
no existing Task, and `lf task move <task> --machine <machine>` moves one: its Graph stops,
its worktree is committed and pushed, and its Workflow position and primary Session go with it.

```bash
lf machine add jack@mini.local --label mini
lf task claim LOO-523
lf task move LOO-523 --machine mini
lf task unassign LOO-523
```

A Task is owned by (person, Machine) or unowned. Its `task_ownership` log holds
`claim`, `handoff` and `unclaim` entries that are never updated or deleted; the
later entry wins in provider order (Linear comment time, or the Git planning
journal order, where each local append sorts after every entry its Machine has seen).
A handoff moves the claim to its target Machine and keeps the person; it needs
only the target's identity, not a connection.

Forwarding stays within one person's Machines. A Task the caller owns on another
of their Machines reruns there; an unowned Task gets the caller's claim on their
Wave Machine; a Task someone else owns gets the caller's claim on their own Wave
Machine, and the later claim wins. Nothing routes to another person's Machine, and
nobody hands off or unclaims another person's claim; cross-person handoff is
deferred. The Machine in a claim comment is informational only.
`lf task claim|handoff|unassign`, `lf task run` and `lf task checkout` route to
that writer over `lf --machine` and re-check inside one IMMEDIATE transaction.
`task_execution_route` returns the claim's Machine (source `owner`) or the Wave
Machine (`owning_machine:wave:<name>`, `owning_machine:this_machine`). `lf task location --peers` observes other
Machines' checkouts and routes nothing.

Each local entry is delivered as a marked Task comment ("Jack Heart claimed this
on mini" plus a `loopflow-ownership` HTML comment carrying the entry) through
ordinary comment delivery to Linear or the Git planning journal; inbound comments
are parsed into the log. With Linear, a claim sets the assignee to that Machine's
Linear viewer and moves backlog, triage or `unstarted` to `started`; unclaim clears
the assignee; handoff leaves it. Assignee changes write no entries; the owning
Machine re-asserts its assignee while its claim is unsuperseded.

An inbound entry never ends this Machine's own open claim; only its own unclaim or
handoff does. When another Machine claims later, every other Machine shows the
later claim while the earlier claimant still owns it locally, reports
`superseded_by` and steers its live Sessions. Nothing is cancelled. If the log
stops showing this Machine as owner for any other reason, such as a stale
Machine's unclaim, the owner claims again.

The target uses its own store, repository, provider homes, OS locks and payload
directory. Reads remain Machine-local. Foreground SSH may explicitly forward
selected account authority; detached processes use installed credentials.

Wave selection always resolves `(canonical repository, slug)` to one UUID.
Bare-slug diagnostics fail when more than one repository owns the slug; no
read or mutation chooses one by order. A scoped lookup repairs an equivalent
legacy path spelling to the canonical repository in one transaction.
`lf wave rename <uuid>` is the only semantic locator mutation: it fences
the Wave chord, moves authored files, commits the new locator
transactionally, and leaves PM and Work rows joined to the
unchanged UUID. A target-local `.lf/tmp/wave-relocations/<uuid>.json` receipt
bridges the filesystem/SQLite commit boundary; retrying after a committed crash
finishes verified source cleanup, then removes the receipt. Repository moves
also require compatible configured PM Teams so relocation cannot impersonate
the separate `lf reteam` operation.

## Promotion and long-running old processes

1. Verify and install immutable versioned artifacts.
2. Copy the selected planning store, apply the candidate schema to that
   isolated copy, and prove the candidate can read it.
3. Atomically repoint the launcher used by future top-level processes.
4. Let already-running processes continue with their selected executable.
5. Restart only the Machine services and app surfaces actually being replaced.
6. Recover or roll back from the persisted artifact-selection receipt.
7. Garbage-collect old artifacts separately from activation.

Any build may select a store it reads; promotion advances the store only with
migrations origin/main carries. A release and a CI build of a main commit both
qualify (`lf self install --main`). An unreleased build's receipt, `lf --version`
and `lf doctor` name it, e.g. `build e9b2c2eac on v0.14.16`, or `schema work …`
when it carries an unmerged migration.

The installation promotion lock serializes artifact selection and service
replacement. It does not discover, drain, stop, or settle conversations and it is not
held by ordinary harnesses. A store snapshot lets preview prove a candidate
schema without mutating the selected store.

An already-running `lf` retains its executable and selected store path.
An execution-schema cutover must account for those writers before activation. On the first published-to-development switch, the process may keep
writing successfully to the prior production store after new commands select
the cloned development store; those writes become invisible to the new
selection. A later development-to-development promotion may reuse and migrate
the selected store, so an old writer may instead fail against changed schema.
Promotion pauses and replaces the known services it owns but does not discover
every shell or provider process. The clone proves candidate readability; it
does not provide cross-store write continuity or old-schema compatibility.
