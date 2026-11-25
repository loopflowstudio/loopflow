# Work

You've handed off a Task: `lf task create`, then `lf -b task run`. This page
picks up from there: handing off several at once and stacking them, steering
one while it runs, seeing where everything stands, sharing the plan with
teammates, and grouping longer objectives into Waves.

## Run Tasks in parallel

```bash
pbpaste | lf task create                     # first line is the title, the rest its notes
lf task run LOO-12                           # starts its Graph and returns immediately
lf task run LOO-12 --open                    # starts its Graph and opens its conversation
lf task run LOO-13 --stack-on LOO-12         # forks from LOO-12's branch before it lands
lf capture-tasks                             # talk through an idea; the agent files the Tasks
```

Each Task gets its own sibling worktree (`../<repo>.<task>`), its own branch,
and at most one open pull request. Every agent working on a Task starts from
the same shared context: the Task's title, notes, thread, checkout, `scratch/`,
and (when in a Wave) the Wave's goal and memory.

A new Task branches from `origin/<default>` as of the
last background fetch (its first `lf sync` catches up). Loopflow keeps one
warm worktree at `<repo>.ready` beside your repo so creation is instant, then
replenishes it in the background. `lf wt prune` leaves it alone, and deleting
it is safe.

To stack a dependent Task, pass `--stack-on LOO-12` when starting it, or
run `lf sync LOO-12` inside an existing Task's worktree to stack it on `LOO-12`.
`lf sync` carries the child through the parent's updates and squash merge, and
on to the parent's next PR when the stacked one closes unmerged; `lf sync main`
detaches it back onto the default branch.

`lf task run`, `lf task auto on`, and `lf task checkout`
prepare the Task's primary conversation and the local server starts it in the
background within seconds (with no background conversation cap). An in-place
`lf -i task run` uses your current terminal instead. Pass `--open` to attach
when launch returns: a direct CLI call attaches in the current terminal, while
an agent tool call opens a sibling tab ([Workspaces](workspaces.md#choose-where-it-appears)).
If opening fails, the Graph keeps running: fix the host issue and run
`lf open --task LOO-12`.

Task conversations stay alive while the Graph runs and come back automatically
after a kill, crash, or reboot ([Sessions](sessions.md#auto-recovery-after-a-kill-crash-or-reboot)).
A conversation you explicitly quit stays quit until you `lf resume --task LOO-12`.

To investigate a failure, run `lf incident` inside any Task checkout (even
after the Task is done) to run an independent investigation with that Task's
context without changing its Graph position or completion state.

## Steer work in flight

```bash
lf steer LOO-12 "keep the retry budget at 3"   # delivered at the next step or turn
lf task status LOO-12 --json                   # thread: your steers, teammates' comments, agent reports
lf task interrupt LOO-12                       # stop its running Graph
lf task unassign LOO-12                        # release ownership; stops CI repair
```

`lf steer` says who will read it:

```console
$ lf steer LOO-12 "keep the retry budget at 3"
Saved 0199a2f1; LOO-12's worker reads it at its next boundary
```

A steer reaches every Session working on the Task at its next provider
boundary (before a Graph step or after a tool call or turn). An idle Task
conversation is woken with it. Comments from teammates in Linear or Asana
arrive the same way. When the Task runs on another Machine, the steer goes to
that Machine ([Steer and read a Task another Machine runs](dev/machines.md#steer-and-read-a-task-another-machine-runs)).

`lf steer LOO-12 --json` returns the receipt: `delivery.reached` is `owner`
(the steer is in the store of the Machine running the Task), `shared` (it is
in Linear, Asana or the plan carrier, waiting for that Machine to sync) or
`here` (only this Machine has it so far); `delivery.runs_on` names the Machine
the Task runs on, or `null`; `delivery.sentence` is the line above. The thread
itself is in `lf task status LOO-12 --json`.

`lf task claim LOO-12` takes ownership without starting a run; `lf task run`
claims and runs. To jump into the conversation directly, run
`lf resume --task LOO-12` ([Sessions](sessions.md)).

## See where everything stands

```bash
lf wave show                                   # each Wave's plan and Tasks in shared order
lf task status LOO-12                          # state, Graph step, PR, Sessions, and Graph runs
lf task changes LOO-12                         # diff summary so far
lf task wait LOO-13                            # block until done or abandoned
```

Filter the flat Task inventory:

```bash
lf task list                                   # open Tasks in this repository
lf task list loo-60                            # filter by identifier or title
lf task list --wave infra --state idle
lf task list --closed --wave infra --since 7d  # closed in the last week
lf task list --subscribe --json                # stream a JSON line on every change
```

`lf task status` says where the Task stands on its Graph in one sentence, the
same words Desktop's workflow bar shows:

```text
LOO-12  moving
  Graph code: build is running implement. You are next needed at ship-review.
```

With auto on, the second clause names auto: "Auto stands in next at
ship-review." A stopped Graph reads "Stopped at build. `lf task run` starts it
again." `--json` carries the parts: `graph.steps` (each top-level step's
`name` and `interactive`, true when a person takes part anywhere inside the
step), `graph.current_step` (its index, null before the start and at the end)
and `graph.sentence`. Before a Task starts, Desktop draws the same steps from
[`lf graph list --json`](orchestration.md#describe-a-graph).

Every read queries this machine's local store, so it answers instantly and
works offline.

`--subscribe --json` also works on `lf task status <id>`, `lf wave show`,
`lf wave status`, `lf session list`, `lf session history`, `lf repo tracker`,
and `lf desktop` reads. Each line is `{"answers", "unavailable", "body"}`,
where `body` matches `--json`. Write `{"id": 1, "args": {"query": "parser"}}`
to stdin to change parameters or `{"id": 2}` to request a refresh; close stdin
to stop.

A Graph that stopped or crashed stays as history; nothing resumes it. Inspect
`lf task status LOO-12`, fix the cause, and run `lf task run LOO-12` to
continue from the step it stopped on. The Task keeps its worktree, branch, and
PR throughout.

## Adopt work already in progress

Already have a dirty checkout or an active conversation? Turn it into a Task
without losing anything:

```bash
lf task create --worktree . --title "Finish the prototype"
lf session list --json                         # find an existing Session ID
lf task create --session SESSION --title "Finish the prototype"
lf session bind SESSION --task LOO-12          # make SESSION this Task's primary conversation
```

Binding and creation change what a conversation belongs to, never where it runs.
The conversation keeps its directory, uncommitted files, provider history, and
live terminal attachment:

```text
$ lf session bind session_94d9963b --task LOO-12
Session session_94d9963b belongs to LOO-12.
Conversation stays in /src/repo.research; LOO-12's checkout is /src/repo.loo-12.
```

In an unowned worktree like `/src/repo.research`, `lf resume` and
`lf resume -a <agent>` keep finding the bound conversation there. From anywhere
(including a shared `main` or Wave checkout), `lf resume LOO-12` opens it in
that same directory.

- Plain `lf task create` always files new top-level work, even inside an
  existing Task checkout or Session. Pass `--parent <ID>` to file a
  [child Task](#file-follow-ups-from-a-task), or `--worktree` / `--session`
  explicitly to adopt.
- `--worktree` without `--run` adopts the checkout only; adding `--run` also
  starts the Task's conversation and Graph. When choosing a conversation from
  that checkout, creation skips Sessions already assigned to another Task.
- An open Task with no checkout adopts an unowned linked worktree on
  `lf task create --session` or `lf session bind`. Main and Wave checkouts stay
  shared, and done Tasks stay done without adopting a checkout.
- When a Task already has a different checkout, `lf session bind` notes both
  paths and leaves the conversation where it is. Graph workers,
  `lf task changes`, PR delivery, and `lf task move` still act on the Task's
  checkout. Closed or headless Sessions cannot become primary.
- In Loopflow Desktop, use **Create Task from this conversation**, **Bind to
  Task**, or **Make primary Task conversation** from the Task menu or command
  palette. The conversation and its files stay in their current directory.

## File follow-ups from a Task

In LOO-12's conversation, say "file the follow-ups", or run it yourself:

```bash
lf task create --title "Retry budget per host" --parent LOO-12  # child of LOO-12
lf task create --title "Unrelated cleanup"                      # top-level work
lf task update LOO-30 --parent LOO-12                           # adopt existing Task
lf task update LOO-30 --parent none                             # clear parent
lf task status LOO-12                                           # LOO-12 and its children
```

```text
$ lf task status LOO-12
LOO-12  idle
  …
  PR: #1972 merged
  Merged · 3 children open
  child: LOO-14  Retry budget per host  moving
  child: LOO-15  Log each retry         idle at ship-review · #1981 open
  child: LOO-16  Document the budget    unstarted
```

The receipt of a child names its parent:

```text
$ lf task create --title "Retry budget per host" --parent LOO-12
Retry budget per host · LOO-14
  parent: LOO-12
```

Passing `--parent <ID>` on `lf task create` (or `lf task update <ID> --parent
<parent|none>`) links a child Task under its parent. Plain `lf task create`
always files top-level work, even inside an existing Task's conversation or
worktree. Each child is an ordinary Task with its own worktree, branch and pull
request; on creation it joins its parent's Wave unless you pass `--wave`.
Tasks can nest at any depth within a repository. Loopflow refuses cycles. A parent says who conducts the
work, a stack says which branch it starts from, and neither sets the other: add
`--stack-on LOO-12` when a child needs LOO-12's unmerged code.

LOO-12's conversation conducts its children. It is told, and woken if idle,
when a child stops at an interactive step, a child's run fails or stops, a
child's PR merges, or a child is completed or abandoned. Steps passing inside
a child's run tell it nothing. A steer goes only to the Task it names:
`lf steer LOO-14 "…"` reaches LOO-14's agents, not LOO-12's conversation.

```text
loopflow: LOO-15 "Log each retry", which this conversation conducts, stopped
at ship-review. lf task status LOO-15
```

A merge is news once Loopflow has seen it: `lf land` and auto mode see their
own, and a PR merged by hand on GitHub shows at the next `lf pr reconcile`.

`lf wave show` lists a child under its parent. Once the parent is
done or abandoned, a child still open is listed on its own; its link stays.

## Finish

```bash
lf --task LOO-12 land --wait-and-fix   # merge its PR, repairing CI until it lands
lf task complete LOO-12                # mark the Task done
```

A Task whose Graph reaches `end` requests completion automatically (waiting
for its PR to merge if it has one). If you land a PR outside its Graph, mark
the Task done with `lf task complete`. See [Pull requests](ops.md#pull-requests)
for publishing, submitting, and CI repair.

A Task with open children stays open after its PR merges. It reads
`Merged · 3 children open`, nothing asks you to complete it, and auto
leaves it open, so its conversation is still there to conduct them. A Task
with no PR whose graph reached `end` stands the same way and reads
`Finished · 1 child open`; once nothing under it is open it reads
`Finished; complete the Task`. Complete it when the follow-ups are settled:

```text
$ lf task complete LOO-12
LOO-12 done
  LOO-14 done: #1980 merged
    LOO-17 abandoned: no merged PR
  LOO-16 abandoned: no merged PR
  LOO-15 abandoned: #1981 open, at ship-review
```

Completing a parent Task settles every open descendant: children whose PR
merged are marked `done`, and all other open children (unstarted or unmerged)
are `abandoned` (closing their PRs and deleting their branches). Abandoned
Tasks cannot be reopened.

A done Task's conversation is still yours. `lf resume LOO-12` reopens it
after the checkout is gone, in the repository's main checkout, and it goes on
hearing about any child that is still open.

## Share the plan with your team

```bash
lf repo tracker                    # how this repository's plan travels, and what's unsent
lf task claim LOO-12               # claim ownership; previous owner's Sessions are notified
lf task unassign LOO-12            # clear ownership; history stays intact
```

### Through Git (default, zero accounts)

Without Linear or Asana, the plan travels with the repository. Clone it on a
second Machine and the plan is there, with nothing to set up:

```text
laptop$ lf task create 'plan smoke'
plan smoke · 3e1c9a
laptop$ lf repo tracker
Plan: Git origin.
```

The Task is on the remote in seconds and `lf task list` on the mini shows it
within a minute; the
first `lf task list` in a fresh clone reads the remote before it answers. A
teammate who clones the repository sees the same plan, and their Tasks appear
in yours: no account and no server, only push access to the repository.

Every Task, Project, Wave, comment and order you change travels: renaming a
Wave on the laptop renames it on the mini, and reordering the backlog reorders
it for everyone. Only planning travels: checkouts, Sessions, Graphs and
processes stay on the Machine that ran them. Claims follow your Git
`user.email`.

Every command saves on this Machine first, so nothing waits on the network.
Each Machine pushes its plan to a ref of its own on the remote
(`refs/loopflow/plan/<machine id>`) as part of the save, for five seconds at
most. Its server fetches the others’ refs about every 30 seconds while it owns
open Tasks in the repository and once a minute otherwise. A running Graph step
keeps checking its repository if the server exits. `lf task sync <issue>` does
both now. The refs stay out of
`git branch`, `git status` and a plain clone. A repository with no remote reads
`Plan: this Machine.` and sends nothing.

When two Machines change the same field, the later change wins on every
Machine. Nothing is marked lost and nothing asks you to choose; the earlier
value is in its Machine's ref history. When two people claim the same Task
before either has seen the other, the first claim holds: the other's run stops
and its work stays in its checkout.

Offline, commands still save, read what this Machine last saw, and the changes
wait:

```text
$ lf repo tracker
Plan: Git origin, failing since Nov 18 14:02, 3 unsent.
  3e1c9a
  5b20c4
  wave billing
  fatal: unable to access 'https://github.com/acme/product/': Could not resolve host: github.com
```

They go when the remote answers again.

A Wave has one name in a repository. `lf wave create` and
`lf task create --wave` read the plan first, so a Wave a teammate just made
is the one you join. When two Machines each made one offline, neither
overwrites the other, and the line says what is waiting:

```text
$ lf repo tracker
Plan: Git origin, 3 not taken.
  wave design from mini has the name of a Wave here; `lf wave rename design <new name>` on either Machine lets both arrive
```

```bash
lf wave rename design design-site     # on either Machine
lf task sync 3e1c9a                   # on the other: each now holds both Waves, their Projects and Tasks
```

The plan is as private as the repository. Anyone who can fetch it can read the
plan, and anyone who can push to it can change any Task.

### Seconds, not a minute: a hub

```bash
lf machine add mini        # once, if mini isn't saved yet
lf machine hub mini        # this Machine also exchanges its plan with mini, live
```

```text
$ lf repo tracker
Plan: Git origin.
Hub mini.
```

Name an always-on Machine as the hub and a change on one Machine reaches the
others in seconds; an open list on the laptop changes when you retitle a Task
on the mini. The hub is a faster way for the same plan to travel, not its home.
Git keeps carrying every change, so a hub that is off costs only the speed:

```text
$ lf repo tracker
Plan: Git origin.
Hub mini, failing since Nov 18 14:02, 2 unsent.
  3e1c9a
  wave billing
  machine "mini": unreachable: … ssh: connect to host mini port 22: Operation timed out
```

Set the hub once per Machine, not per repository; it serves every repository
on that Machine that has no Linear or Asana, and needs its own clone of each.
It is reached over SSH as you, and a teammate who can SSH to it can name it
too. A hub running a different `lf` says which side to update and how
(`ssh -- 'mini' 'lf install'`). `lf machine hub --clear` stops using it and
loses nothing; `lf machine list` marks the hub.

A repository with no remote can still use a hub. With no ref to tell its
clones they are the same repository, say so once: `lf config repo` prints the
repository id on the first Machine and `lf config repo --bind <id>` sets it in
each other clone, the hub's included.

A Machine still running an earlier `lf` keeps reading the old
`refs/loopflow/planning/*` and sees none of these changes until it updates. A
`tracker: { git: … }` key from earlier versions is read as no tracker.

### Linear or Asana (two-way sync)

Coworkers do not need to install `lf`: they keep working in Linear or Asana,
and changes stay synchronized both ways:

```bash
lf machine login linear                         # or: lf machine login asana
lf wave plan connect --all --team-key LOO       # Linear: one Team per repo; Tasks become LOO-12
```

For Asana, add your team to `.lf/config.yaml`:

```yaml
# .lf/config.yaml
tracker:
  asana:
    team: "<team-gid-or-url>"
```

| A coworker in Linear or Asana… | Loopflow… |
|---|---|
| Creates an issue | Adds it to `lf task list` |
| Edits the title or description | Updates the Task and notifies running agents |
| Leaves a comment | Delivers it to the Task's agents as a steer |
| Completes, cancels, or reopens it | Updates the Task to match |
| Makes it a sub-issue of another (Linear) | Lists it as that Task's child |

| You in Loopflow… | Coworkers see… |
|---|---|
| Create, edit, steer, complete, reorder, or delete | The same change |
| File a follow-up from a Task | A sub-issue of that Task's issue (Linear) or a subtask (Asana) |
| Start work on the Task | Issue moves to In Progress |
| Open, land, or abandon its PR | One linked PR on the issue, updated as its state changes |

In Linear, a Wave is one Linear Project and its Tasks are that Project's
issues. In Asana, it is a team project.

```bash
lf wave plan connect billing                       # make billing's Project in Linear
lf wave plan connect billing 'Refunds'             # or link the Linear Project named Refunds
lf wave plan connect billing https://linear.app/acme/project/refunds-1a2b3c4d5e6f
```

You can connect a new Wave directly to an existing Linear Project. A Linear
Project no Wave links stays out of Loopflow.

Loopflow registers Waves from an existing checkout's `.lf/wave/` files
without editing those files. When renting a Machine for a Linear Task,
Loopflow connects the Task's Wave to its linked Project on the rental.

### Offline queue and conflict resolution

Every tracker edit saves locally first and joins a per-repo outbound queue,
drained in short batches across commands so you don't burn rate limits. Offline
edits wait in the queue until you reconnect:

```text
$ lf repo tracker
Tracker: linear. 3 open · 1 lost.
  LOO-412            title · queued · Offline
  LOO-412            comment · queued · Offline
  LOO-398            complete · sent, waiting for linear to confirm
  LOO-377  9f2c41d0  title · conflicted · lost: "Offline rename" · linear: "Renamed in Linear"
Answer a lost line: lf repo tracker answer <id> --keep-theirs, or --use-mine.
```

- **Waiting to confirm:** sent, reply not yet observed. Loopflow reads the
  tracker before sending more edits for that Task (at most once every 10
  minutes; run `lf task sync LOO-398` to check now).
- **Conflicted (lost):** a coworker edited the same field before yours arrived,
  so the tracker's value won. Both values stay visible until you run
  `lf repo tracker answer 9f2c41d0 --keep-theirs` or `--use-mine` (or click
  **Keep** / **Use mine** in Desktop, or run `lf task sync LOO-377 --keep-theirs`).
- **Rate limited:** when Linear refuses a request with a 429, Loopflow holds
  back only what Linear limited. A limit on one mutation (Linear limits
  linking PRs more tightly than other writes) holds that mutation until
  Linear says it refills; new Tasks, comments and state changes keep going
  out. Only an exhausted hourly allowance holds every
  write until its reset. Turn on `logs: { linear_requests: true }` in
  `~/.lf/config.yaml` to see which operation was held:

  ```text
  $ lf history linear --since 1h
  COMMAND                   OPERATION                         SENT      HELD  COMPLEXITY
  lf                        LinkAttachment                       2         3          4
  lf task create            DeliverTaskCreation                  1         0         30
  ```

In a connected repository a new Wave's Project goes to Linear with it. If that
send fails, `lf wave status` shows `Project … (local)` and its Tasks wait in the
queue until `lf wave create <wave>` makes it. Linking another Linear Project
replaces the one Loopflow made while none of the Wave's Tasks is in Linear yet.
A Task in no Wave goes to Linear right away, as a team issue.

## Waves

A Wave groups related Tasks under a lasting objective (`GOAL.md`), curated team
memory (`MEMORY.md`), its Project, and shared defaults. Every Task in the Wave
starts with that context.

### Goal and memory

```bash
lf wave create billing                          # registers it and commits .lf/wave/billing/ on main
$EDITOR .lf/wave/billing/GOAL.md                # who benefits and what improves
lf task create --wave billing --title "Meter usage events"
```

```markdown
<!-- .lf/wave/billing/GOAL.md -->
## Objective

Replace legacy billing with metered usage so customers pay for what they run.

## Process

Make mechanical changes directly; write a scratch design first when the change
crosses storage, auth, or public APIs.
```

`MEMORY.md` beside it holds what later Tasks need: decisions, dead ends and facts.
Agents update it as work ships, and `realign` keeps
plan, code, and memory in sync. Both files are ordinary Markdown in Git,
reviewed in PRs. Check each file's context footprint with
`lf context --wave billing`.

### Defaults per Wave

```yaml
# .lf/wave/billing/config.yaml (committed, inherited by every Task in the Wave)
graph: code
agent: claude
skills:
  implement:
    agent: claude:opus
context:
  budgets:
    memory_tokens: 20000
```

```bash
lf --wave billing set machine mini              # private override: your machine for this Wave
lf --wave billing set agent codex               # private override: your agent; team's stays claude
lf wave status billing                          # shows every effective setting and where it came from
```

Wave `config.yaml` overrides `.lf/config.yaml`, and your private
`.lf/config.local.yaml` (edited by `lf set`, shared across this clone's
worktrees) overrides both. Use `lf unset <key>` to remove a personal override.
See [Configuration](config.md) for the complete precedence rules.

### Operate a Wave

```bash
lf --wave billing wave-operate "invoices first"   # one finite pass over its Tasks
lf resume --wave billing                          # open the Wave's ongoing conversation
lf wave status billing                            # the Wave's Project, Tasks and runtime state
```

`wave-operate` reviews every started Task you own (leaving running Graphs alone
and continuing stopped ones), lists unstarted backlog for you to pick, and
leaves teammates' Tasks untouched. Run it on a schedule with your OS scheduler:

```bash
# Linux crontab (or a macOS LaunchAgent when Keychain access is needed):
# 0 9 * * *  cd ~/src/product && lf -b --wave billing wave-operate
```

On macOS, scheduled commands that need Keychain credentials belong in a
LaunchAgent under `~/Library/LaunchAgents` rather than cron.

### Projects

```text
$ lf wave create billing ; lf wave create billing/refunds
$ lf task create --wave billing/refunds --title "Refund a mischarged invoice"
Refund a mischarged invoice · LOO-14
$ lf wave status billing/refunds
…
  Refund a mischarged invoice   LOO-14   unstarted
```

A Wave is one Project, made with the Wave: `lf wave create billing/refunds`
makes the Project `refunds`, and in a repository with Linear or Asana makes it
there too before it returns. Can't reach the tracker? The command fails and
says to run it again, which makes it. A Wave that arrived without one (from
an upgrade) gets its tracker Project from that command or the first Task sent
into it; `lf wave sync` and Desktop's refresh never make one. Until then `lf
wave status` and Desktop say the Project is not in the tracker yet and name
the command that makes it. Tasks reordered meanwhile keep that order in the
tracker. A Task
in a Wave is in its Project, and lists under that Wave only. A Task in no
Wave is in no Project.

The Project has a name, a summary and a status, and `lf wave status` and
`lf wave plan billing --json` show it. It starts and ends in the tracker:
rename it or mark it completed in Linear, and `lf wave sync` shows the same
here. An ended Project is still the Wave's, and new Tasks still join it.

In Linear, an issue in a linked Project appears under that Project's Wave on
other Machines. A Task with no Wave goes to Linear as a Team issue with no
Project. Other Machines syncing the Team import it as a Task with no Wave.
Issues in unlinked Linear Projects stay out of Loopflow.

`lf task move` puts a Task in the new Wave's Project. A started Task keeps its
worktree and PR wherever it is.

### Nested Waves

Create `.lf/wave/infrastructure/release/GOAL.md` and address
it as `infrastructure/release`. Agents read the parent's top-level Markdown,
then the child's (siblings stay out). A root `MEMORY.md` applies even with no
Wave selected.

### Rename or remove a Wave

```bash
lf wave rename billing payments          # moves files on main; keeps Tasks and ID; renames its Project, in Linear too
lf wave remove payments --preview        # preview what happens to Tasks and Sessions
lf wave remove payments                  # moves open Tasks to the parent Wave or repository
lf wave remove payments --delete-backlog # delete unstarted Tasks instead; started ones always move
```

Removal retires the Wave and its children, folds a child's `MEMORY.md` into its
parent, and keeps completed history readable via `lf wave show payments`.
You can then connect another Wave to the same Linear Project.

### Talk to it from Discord

Talk to a Wave from a Discord channel:

```yaml
# .lf/wave/billing/config.yaml
chat:
  provider: discord
  guild_id: "123456789012345678"
  channel_id: "234567890123456789"
```

```bash
doppler run -- lf discord serve billing
```

Each new message becomes a Wave-attributed conversation and posts its final
answer back to the channel.

## Reference

**Task identifiers.** Ticket aliases (`LOO-12`, case-insensitive), full IDs, or
unique prefixes of four or more hex digits (`abcd`, `lf-abcd`, `task_abcd`).

**Parent.** `--parent <ID>` on `lf task create` or `lf task update` names the
parent; plain `lf task create` files top-level work, and `--parent none` on
`lf task update` clears the parent. An unknown ID is an error naming it, and so
is a parent that would be its own descendant. `lf task create --json`,
`lf task status --json` and `lf task show --json` carry `parent`, the
parent's row as `lf task list --json` prints it, or `null`, so
`.parent.identifier` reads the same from all three. The last two also carry
`children`, the same rows, `[]` when there are none. Each row carries
`parent_id`, the parent's `id` or `null`, so a list can nest. A child's own
`lf task status` prints `parent: LOO-12`.

**Graph selection.** A Task keeps the graph it took up. Before its first run:
the one named on `lf task run LOO-12 GRAPH`, then its Wave's default, then
`graph:` in `.lf/config.yaml`, then builtin `code`. `lf task run LOO-12 --from
demo` starts at any step; starting from an interactive step only puts the Task
there. See [Orchestration](orchestration.md#where-a-person-takes-part).

**Membership and lifecycle.** A [follow-up](#file-follow-ups-from-a-task) is
filed in its parent's Wave and Project, and stays there if the parent later
moves. `lf task move LOO-12 --wave billing` (or `--no-wave`) moves a Task while
keeping its Session, checkout, PR, and Graph. A Task keeps one worktree and one current PR at a time, with PRs over time:
once a PR merges on a Task that stays open (conducting children, or after
`lf task reopen`), `lf task run <ID> --from <step>` (or `lf pr submit`) starts
the next pass and opens the next PR linked to the same issue.

`lf task complete` updates status only on a leaf Task; on a parent it also
finishes children with merged PRs and abandons all other open children
(closing their PRs and deleting their branches), recursing through open
descendants. `lf task complete --json` carries `children`: each child's row,
its `outcome` (`done` or `abandoned`) and its own `children`, `[]` when there
were none. `lf task reopen` keeps the same PR and restores started Tasks to
`started` (or `unstarted` if never started); abandoned Tasks cannot be
reopened.
`lf task abandon` cancels the Task, closes its PRs, and deletes its branches.

**Idempotent creation (`--request-id`).** Scripts and Desktop can pass
`--request-id <id>` to `lf task create` so a lost reply never creates a
duplicate Task. Inspect `lf server receipt <id> --json` (read-only; returns
`null` if absent), or repeat the exact command with the same ID. Creation JSON
returns `{task, graph, session_id, milestone}` (`filed` without `--run`,
`admitted` with `--run`, where setup finishes inside the Task's conversation;
if setup fails, `lf resume --task <ID>` retries it). See
[Recover a submitted creation](server.md#recover-a-submitted-creation).

**Tracker sync notes.** `lf task sync LOO-12` exchanges one Task now;
`lf wave sync billing` reads tracker changes, then gives billing’s Project
priority among pending saves. A save that stays queued shows in status with
its last error when available. A linked Linear Wave also reads its Project and issues. Without a
tracker, both commands exchange the repository’s plan through Git and its
configured hub. `lf task sync` also shows what each still has unsent. If Linear updates an issue or
Project without bumping `updatedAt`, Loopflow adopts the fresh reading and
keeps your local edits queued. An issue moved into another Wave's Linear
Project moves to that Wave.

Every flag: [`lf task`](lf-reference.md#lf-task), [`lf wave`](lf-reference.md#lf-wave).

Next: [Sessions](sessions.md): persistent conversations, multi-view attach,
and crash recovery.
