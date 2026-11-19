# Work

You've handed off a Task: `lf task create`, then `lf -b task run`. This page
picks up from there: steering it while it runs, seeing where everything
stands, sharing the plan with teammates, and grouping longer objectives into
Waves.

## Hand off more than one

```bash
pbpaste | lf task create                     # first line is the title, the rest its notes
lf task run LOO-12                           # returns once its Graph is running
lf task run LOO-12 --open                    # continue its Graph and open its conversation
lf task run LOO-13 --stack-on LOO-12         # builds on LOO-12's branch before it lands
lf capture-tasks                             # talk it through; the Session files the Tasks
```

A Task adopts existing work or gets its own worktree beside your repository, its own branch and at
most one pull request. A stacked Task forks from its parent's branch on origin,
pushing it when absent or when an unpublished parent is ahead; `lf sync`
carries the child through the parent's updates and squash merge. To stack a
Task that already exists, run `lf sync LOO-12` in its worktree; `lf sync main`
takes it back off.

`lf task run`, `lf ship`, and an explicit `lf task checkout` start the Task's
conversation in the background when an agent is configured and installed.
An in-place `lf -i task run` uses its current terminal instead.
`lf resume` in its worktree reattaches to it. Previously opened conversations
stay as they are. Every newly checked-out Task can start its conversation;
there is no background conversation cap.
A running Graph keeps its conversation alive; abandoning the Task stops its
unviewed conversation. A conversation you are viewing stays open.
Preparing a conversation, including Desktop's preload, does not launch an agent.

Add `--open` to open the Task's primary conversation when launch returns,
including when the Task is waiting at an interactive step. It reuses the Task's
primary Session, including one already started in the background. A bare run
opens at Graph registration; `-b` and `-i` still wait for their foreground run
before opening.

Opening follows [`lf open`'s host selection](workspaces.md#choose-where-it-appears):
a direct CLI invocation attaches in the current terminal; an agent tool call
opens a sibling tab. When the Graph runs on another Machine, the conversation
opens locally where you invoked the command. `--explain` and `--quote` open
nothing. Task runs reject `--context`; use `--explain` to inspect their selection.
With `--json`, stdout holds only the `task.run` value; the open receipt goes to
stderr.

A failed launch opens nothing. An opening failure does not stop the Graph.
Fix the reported host or attachment problem, then use
`lf open --task LOO-12` to open the conversation without advancing the Graph.

Every agent that works on a Task starts from the same place: the Task's notes
and thread, its checkout and `scratch/`, and, in a Wave, the Wave's goal
and memory.

Run `lf incident` in a Task checkout to investigate a failure, including after
the Task is done. A named Graph keeps the checkout's Task context and records
its own run; it leaves the Task's Graph position and completion unchanged.

### Recover a creation whose reply was lost

```bash
lf task create --request-id terminal-recovery-42 --no-wave --title "Keep the terminal intact after sleep" --json
```

Save a unique request ID before sending a creation you may need to recover.
Inspect `lf server receipt terminal-recovery-42 --json` after a lost reply.
This read needs no running server and starts no work. A missing receipt is `null`;
an unavailable lookup is an error, not absence. Repeating the exact creation on
the same Machine and repository with that ID returns the original creation or
continues its safe unfinished work rather than filing a second Task.
Changing the payload while reusing the ID returns `conflict`. A new creation
needs a new ID. Without `--request-id`, the CLI generates one for that invocation;
rerunning the command without it is a new submission.

The JSON creation result contains `task`, `graph`, `session_id` and `milestone`,
including without `--run`. Scripts read the identifier at `.task.identifier`.
Without `--run` the milestone is `filed`. With `--run` the answer arrives once
the Task, its claim, its Graph and its conversation are recorded, before any
checkout or network call: the milestone is `admitted`. The conversation then
does the rest, one line per step:

```
Checkout ready  0.2 s
Graph code started
```

Repeating the request ID during setup returns the same Task and Session and
starts no second Graph. If a setup step fails, the conversation shows the error
and the Task's position reads `Setup stopped: checkout failed` (or `graph`) in
`lf task status` and Desktop; `lf resume --task <ID>` reopens it and runs setup
again. Tracker
sync and the assignee check run after the answer; a failure there is a Task
event, not a creation failure.

A new Task's branch starts from `origin/<default>` as of the last background
fetch, not a fetch at creation; its first `lf sync` catches up. To make the
checkout instant, Loopflow keeps one prepared checkout per repository at
`<repo>.ready` beside your Task worktrees and replaces it after each Task takes
it. `lf wt prune` leaves it alone. Deleting it is safe: the next Task makes its
checkout the slower way and a new one is prepared.

A receipt for a removed Task does not recreate it. An earlier release may have
left a `run_requested` receipt: its launch is unconfirmed, so inspect
`lf task status <ID>` before starting anything.

Desktop saves and recovers these submissions automatically, including after it
quits. Showing the resulting Task clears its saved submission; promotion from a
conversation follows the same path. See [Recover a submitted creation](server.md#recover-a-submitted-creation).

## Keep work already in progress

```bash
lf task create --worktree . --title "Finish the prototype"
lf session list --json                         # find the Loopflow Session ID
lf task create --session SESSION --title "Finish the prototype"
lf session bind SESSION --task LOO-12           # select this Task’s conversation
```

Promotion preserves the branch, dirty and untracked files, and the Session's
provider history and live attachment. The selected Session becomes the Task's
primary conversation; `lf open --task LOO-12` and `lf resume --task LOO-12` find it.
Pass `--session` explicitly to keep a conversation. An agent reads its own
Session ID from its lf context block.
A Graph belongs to the Task it was started for. Graphs that ran in an adopted
checkout before the Task adopted it stay unattached; `lf task status` lists the
Graphs launched for the Task from then on. While an earlier Graph still runs
in that checkout, status shows the Task moving and reports its execution;
`lf task run` starts no other Graph until it finishes.
Without `--run`, `lf task create --worktree` adopts only the checkout; it selects
no conversation. `--run` then uses ordinary Task conversation admission.
Opening a conversation more recently never selects it. Explicit-source retries
keep the same Task; conflicting Task ownership fails without moving either
person's work.

Plain `lf task create` files new work, including from an owned checkout or
Session. `--run` starts the created Task; it does not opt into adoption.
Explicit sources work with or without `--run`. Creation alone starts no Graph
and approves no Auto-Ship. Binding also requires an explicit Session ID.

Main and Wave checkouts remain shared. A selected Session there can become a
Task conversation without moving its cwd; an explicit later `lf task checkout`
gives the Task a separate home. Selecting a repository or Wave primary Session
transfers that conversation to the Task; its former scope can open a new one.
A remote command uses the selected Machine's existing paths and Sessions; it does not transfer local live work.

Desktop offers **Create Task from this conversation** in the Session’s binding
action. **Bind to Task** makes it the selected Task’s primary conversation.
Already-assigned Sessions offer **Make primary Task conversation** in the palette
and Task menu. The current terminal and composer stay in place.

When an existing Task already has a worktree, `lf session bind <id> --task <task>`
keeps that worktree and immediately sets the Session's working directory there.
Already-running clients may keep their original directory; subsequent connections
use the selected one. Files remain in their original checkout. If an open Task has no checkout, binding adopts the Session’s unowned
linked checkout. Done Tasks can still select an unfinished conversation without
reopening work. With no Task checkout, the conversation stays in place and no
checkout is adopted. Closed or headless Sessions cannot become primary.

## Steer it while it runs

```bash
lf steer LOO-12 "keep the retry budget at 3"   # reaches its agents at their next step or turn
lf task status LOO-12 --json                        # the thread: your steers, teammates', the agents' reports
lf task interrupt LOO-12                          # stop its running Graph
lf task unassign LOO-12                           # nobody owns it: ship and CI repair stop; live processes finish
```

A steer saves locally first, then reaches every Session working on the Task at
its next supported boundary: before a Graph step or after an agent turn. An
idle Task conversation is woken with it. Direction from teammates, in Loopflow
or as comments in your tracker, arrives the same way.

`lf task claim LOO-12` takes it back without starting anything; `lf task run`
claims and runs. To reopen a conversation, use `lf resume` ([Sessions](sessions.md)).

## See where it stands

```bash
lf wave show                       # each Wave's plan and Tasks, sorted by what they need
lf task status LOO-12              # state, graph step, PR, and every Session and Graph on it
lf task changes LOO-12        # what it changed so far
lf task wait LOO-13                # wait until the Task is done or abandoned
```

```bash
lf task list                                  # open Tasks in this repository
lf task list loo-60                           # identifier or title contains "loo-60"
lf task list --wave infra --state idle
lf task list --closed --wave infra --since 7d   # closed in the last week
lf task list --subscribe                          # a JSON line each time the list changes
```

`lf task list` is the flat inventory. Every view reads this machine's store, so
it answers instantly and offline.

`--subscribe` also works on `lf task status <id>`, `lf wave show`, `lf wave status`,
`lf session list`, `lf session history`, `lf repo tracker` and the
`lf desktop` reads, always with `--json`. Each line is
`{"answers", "unavailable", "body"}`, where `body` is
what `--json` prints. Write `{"id": 1, "args": {"query": "parser"}}` to stdin to
change what it reads, or `{"id": 2}` to ask for a fresh reading; the next line
answers that id. It stops when stdin closes. `lf repo tracker --subscribe` also
keeps exchanging with the hub while it runs.

A Graph that stopped or crashed stays as history; nothing resumes it. Read
`lf task status`, then launch what remains with `lf task run`. The Task keeps
its worktree, branch and PR throughout.

## Finish

```bash
lf --task LOO-12 land --wait-and-fix   # merge its PR, repairing CI until it lands
lf task complete LOO-12                 # then mark the Task done
```

A Task whose graph reaches `end` requests completion itself; with a PR,
completion waits for the merge. Landing outside its graph leaves the Task open
until you complete it. Publishing, landing and CI repair are covered in
[Pull requests](ops.md#pull-requests).

## Share the plan

```bash
lf repo tracker                    # how this repository's plan travels, and what's unsent
lf task claim LOO-12               # take it; the earlier owner's Sessions are told
lf task unassign LOO-12            # nobody owns it; Linear shows no assignee; history intact
```

Without Linear or Asana, the plan travels with the repository. Clone it on a
second Machine and the plan is there, with nothing to set up:

```text
laptop$ lf task create 'plan smoke'
plan smoke · 3e1c9a
laptop$ lf repo tracker
Plan: Git origin, 1 unsent.
  3e1c9a
laptop$ lf repo tracker
Plan: Git origin.
```

On the mini, `lf task list` shows the Task within a couple of minutes; the
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
(`refs/loopflow/plan/<machine id>`) within a minute of a change, a few seconds
after one made in Desktop, and fetches the others' once a minute;
`lf task sync <issue>` does both now. The refs stay out of
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
Plan: Git origin, unreachable since Nov 18 14:02, 3 unsent.
  3e1c9a
  5b20c4
  wave billing
  fatal: unable to access 'https://github.com/acme/product/': Could not resolve host: github.com
```

They go when the remote answers again.

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
Hub mini, unreachable since Nov 18 14:02, 2 unsent.
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

### Linear or Asana

```bash
lf account connect linear                  # or asana
lf wave plan connect --all --team-key LOO       # Linear: one Team per repository; Tasks become LOO-12
```

```yaml
# .lf/config.yaml, for Asana
tracker:
  asana:
    team: "<team-gid-or-url>"      # the Asana team holding this repository's project
```

Each Task is one item in the tracker, and the tracker is where coworkers work:

| A coworker, in the tracker… | Loopflow… |
|---|---|
| creates a task | adds it to `lf task list` |
| edits the title or description | updates the Task and tells a running agent |
| comments | delivers it to the Task's agents as direction |
| completes, cancels or reopens it | moves the Task to match |

| You, in Loopflow… | Coworkers see… |
|---|---|
| create, edit, steer, complete, reorder or delete | the same change |
| start work on the Task | it moves to in progress |
| open, land or abandon its PR | a link to the PR and its state |

Linear and Asana behave the same way. In Linear, a Wave is an Initiative and
each of its Projects is a Linear Project; in Asana, it is a team project.

Every change saves on this machine first and joins one queue of actions for
the tracker, sent in the order you made them. Each `lf` command sends a short
batch from the queue, so a long backlog drains over several commands without
using up the tracker's request allowance. Offline, commands still save and the
queue waits:

```text
$ lf repo tracker
Tracker: linear. 3 open · 1 lost.
  LOO-412            title · queued · Offline
  LOO-412            comment · queued · Offline
  LOO-398            complete · sent, waiting for linear to confirm
  LOO-377  9f2c41d0  title · conflicted · lost: "Offline rename" · linear: "Renamed in Linear"
Answer a lost line: lf repo tracker answer <id> --keep-theirs, or --use-mine.
```

When a send failed, the line ends with why: offline, signed out, or the
tracker's own error.

A change waiting to confirm was sent, but its answer never arrived.
Loopflow reads the tracker before sending anything else for that Task, at most
once every ten minutes, so the change lands once. `lf task sync LOO-398` looks
now.

When a coworker changed the same field in the tracker before your edit
arrived, the tracker's value wins and your edit is lost, not open: nothing is
left to send. The line shows both values, on the Task or Project, until you
answer it. `lf repo tracker answer 9f2c41d0 --keep-theirs` closes it;
`--use-mine` saves yours again and sends it; so does editing that field.
Desktop's Keep and Use mine buttons on the line do the same, and
`lf task sync LOO-377 --keep-theirs` answers every line a Task lost. It also closes on its own when a
coworker changes that field again in the tracker.

A Wave not yet connected to an Initiative has nothing to exchange. Its Tasks
stay local and `lf wave sync` shows them without asking Linear, until
`lf wave plan connect billing` connects one:

```text
$ lf wave sync billing
billing: no Linear Initiative; showing local planning (lf wave plan connect billing)
```

## Waves

A Wave keeps a longer objective moving across many Tasks: a goal, the memory
agents curate as they work, and its Tasks, grouped in Projects. Every Task in
it starts with that context.

### Give it a goal and memory

```bash
lf wave create billing                    # register it, commit .lf/wave/billing/ on main
$EDITOR .lf/wave/billing/GOAL.md       # who benefits, and what improves
lf task create --wave billing --title "Meter usage events"
```

A clone's committed `.lf/wave/` directories register when the repository joins
the Desktop portfolio, and on `lf wave create` or `lf wave plan connect --all`; Desktop
and `lf wave list` read the same rows. A removed Wave's directory stays removed.

```markdown
<!-- .lf/wave/billing/GOAL.md -->
## Objective

Replace legacy billing with metered usage, so customers pay for what they run.

## Process

Make mechanical changes directly; write a scratch design first when the blast
radius crosses storage, auth, or public APIs.
```

`MEMORY.md` beside it is durable working context: decisions, dead ends, what a
downstream Task should know. Agents fold what shipped into it; `realign`
reconciles plan, code and memory. Both are ordinary repository files, reviewed
like code. With two contributors, keep memory as short, independent sections so
pull requests merge.

`lf context --wave billing` shows each file's share of the context budget.

### Rename or remove it

```bash
lf wave rename billing payments          # same Wave, Tasks and planning; files move on main
lf wave remove payments --preview        # what happens to its Tasks and Sessions
lf wave remove payments                  # open Tasks move to the parent Wave or the repository
lf wave remove payments --delete-backlog # delete unstarted Tasks instead; started ones always move
```

Removal retires the Wave and its child Waves: completed Tasks and
conversations stay readable with `lf wave show payments` until a new Wave takes
the name. A child's `MEMORY.md` folds into its parent's. Running it again
finishes an interrupted removal; adding the name again starts a fresh Wave.
Repeat a failed rename with the same old and new names to finish its pending
files and Initiative title. Retries keep later edits uncommitted. An unfinished
request retains the original Wave even if its name has been reused; address the
replacement by its ID for a new removal or rename.

### Run a pass, or keep a conversation

```bash
lf --wave billing wave-operate "invoices first"   # one finite pass over its Tasks
lf resume --wave billing                          # the Wave's ongoing conversation
lf wave status billing                            # Projects, Tasks and execution evidence
```

`wave-operate` gives every started Task you own a disposition: moving, acted on,
waiting on a named person, unassigned, or unknown. It leaves running Graphs alone,
reruns what remains of a stopped one, and lists unstarted backlog for you to
pick. It never edits the plan or files Tasks. Teammates' Tasks are listed and
left alone.

The Wave's conversation carries the same pass each time you return. Between
turns, running Graphs keep going on their own. For a schedule, run an ordinary
command from your own scheduler:

```bash
# 0 9 * * *  cd ~/src/product && lf -b --wave billing wave-operate
```

On macOS, a scheduled command that runs an agent or reads Keychain credentials
belongs in a LaunchAgent in your login session (`StartCalendarInterval` in
`~/Library/LaunchAgents`), because cron runs outside the login session and
cannot reach the Keychain. Crontab works on Linux and for jobs that need no
Keychain.

### Set it up once

```yaml
# .lf/wave/billing/config.yaml: committed, inherited by the Wave's Tasks
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
lf --wave billing set machine mini                        # your Machine for its Tasks
lf --wave billing set agent codex                         # your agent here; the team's stays claude
lf wave status billing                                    # each setting and its source
```

`config.yaml` has the shape of `.lf/config.yaml` and holds only what the Wave
changes; Wave values override repository values, and personal config overrides
both. Repository settings (`planning`, `release`,
`merge_method`, `land`, `pr`) are an error there. It also carries the Wave's
`id`, `linear.initiative` and `chat`. A Task with no Wave uses the repository's
settings.

`lf set` edits your private `lf.config.local.yaml`, shared across this clone's
Task worktrees. `--wave` selects a Wave within that file; `--global` edits
`~/.lf/config.yaml`. Independent clones have independent private preferences.
The same values can be edited by hand or through Desktop. `lf unset <key>` removes
your declaration and reveals the next applicable value.

Machine choices are personal file settings too. Credentials,
observed availability and existing work placement remain runtime state. Agent choices
beat all `default_agent` fallbacks; within each class, personal settings beat shared
settings. [Configuration](config.md) owns the complete source/scope/target rules and
remote preference behavior. `lf wave status` explains each value's source.

A Wave without `config.yaml` reads these keys from `GOAL.md` frontmatter, which
released lf still reads (LOO-640 removes that).

### Projects

```text
$ lf wave create billing
$ lf task create --wave billing --title "Meter usage events"
Meter usage events · LOO-12
$ lf wave status billing
…
  Project   billing (4f1c…) · started
```

A new Wave has no Project and sends nothing to the tracker. Its first Task
makes one, named for the Wave. A Wave can hold several Projects, each with a
name, a summary and a status; `lf wave status` and `lf wave plan billing --json`
list them in the Wave's order, ended ones included.

Projects start and end in the tracker: rename one, start another or mark one
completed in Linear, and `lf wave sync` shows the same here. Without a
tracker, a Wave keeps the one Project its first Task made.

A new Task joins the Wave's newest started Project, or its newest Project
that has not ended when none is started. When every Project has ended, the
Task makes a new one. With more than one open, the command says which it
chose:

```text
$ lf task create --wave billing --title "Refund a mischarged invoice"
Refund a mischarged invoice · LOO-14 in Project Invoicing
```

Move a Task to another Project in the tracker. A Task in any Project of its
Wave runs, and a started Task keeps its worktree and PR wherever it is.

### Nested Waves

```text
.lf/wave/infrastructure/GOAL.md
.lf/wave/infrastructure/release/GOAL.md
```

Address the child as `infrastructure/release`. Its agents read the parent's
top-level Markdown, then the child's; siblings stay out. A root `MEMORY.md`
comes first and applies with no Wave selected.

### Talk to it from Discord

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

One foreground bridge per channel: each new message becomes a Wave-attributed
conversation, and its final answer is posted back. Restarting skips history
it missed.

## Reference

**Task identifiers.** Ticket aliases (`LOO-12`, any case), full IDs, or unique
ID prefixes of four or more hex digits (`abcd`, `lf-abcd`, `task_abcd`). An
ambiguous prefix lists its matches.

**Graph selection.** A Task keeps the graph it took up. Before its first run:
the one named on `lf task run LOO-12 GRAPH`, then its Wave's default, then
`graph:` in `.lf/config.yaml`, then builtin `code`. `lf task run LOO-12 --from
demo` starts at any step; starting from an interactive step only puts the Task
there. See [Orchestration](orchestration.md#where-a-person-takes-part).

**Membership.** `lf task create` makes a standalone Task unless you pass
`--wave`. `lf task move LOO-12 --wave billing` (or `--no-wave`) changes it later
and keeps the Session, checkout, PR and graph.

**Completion.** `lf task complete` changes status only, independent of graph
position; retry it alone if it fails. `lf task reopen` keeps the same PR.
`lf task abandon` cancels it, closes its PRs and deletes its branches.

**Sync.** `lf task sync LOO-12` exchanges that Task with its tracker now. On
Linear or Asana it looks for every change sent without an answer and resends
what the tracker lacks; between syncs, every `lf` command sends one short batch.
It prints what is still open (a change queued or sent) and what was lost to the
tracker's value, until you answer it with `--keep-theirs` or `--use-mine`.
Without a tracker it pushes and fetches the repository's plan and exchanges
with the hub, then prints what each still has unsent. `lf repo tracker` lists the same
lines for the whole repository, and `lf repo tracker answer <id>` answers one.
`lf wave sync` refreshes from the tracker first; a Wave with no Initiative skips
the refresh and still succeeds.

```bash
lf task sync LOO-403
# LOO-403: title · conflicted · lost: "Offline rename" · linear: "Renamed in Linear"
```

Linear can change an issue or a Project without moving its `updatedAt` (it
re-renders branch names, for one). When its copy disagrees with lf's saved copy
at the same `updatedAt`, the sync keeps what it just read and carries on, for
Tasks and Projects alike; your saved local edits stay queued. A Project's
Teams follow Linear the same way. Moving a Project to another Initiative is
held instead, because the Initiative decides which Wave plans it: move it back
in Linear, then run `lf wave sync`. When an error has a fix, it names the
command: `lf task sync <issue>` for a Task, `lf wave sync` for a Project.

Every flag: [`lf task`](lf-reference.md#lf-task), [`lf wave`](lf-reference.md#lf-wave).

Next: [Sessions](sessions.md), every conversation those Tasks ran.
