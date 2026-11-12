# Work

You've handed off a Task: `lf task create`, then `lf -b task run`. This page
picks up from there: steering it while it runs, seeing where everything
stands, sharing the plan with teammates, and grouping longer objectives into
Waves.

## Hand off more than one

```bash
pbpaste | lf task create                     # first line is the title, the rest its notes
lf task run LOO-12                           # returns once its Graph is running
lf task run LOO-12 --open                     # continue its Graph and open its conversation
lf task run LOO-13 --stack-on LOO-12         # builds on LOO-12's open PR before it merges
lf capture-tasks                             # talk it through; the Session files the Tasks
```

A Task adopts existing work or gets its own worktree beside your repository, its own branch and at
most one pull request. A stacked Task forks from its parent's PR; `lf sync`
carries the child through the parent's updates and squash merge.

`lf task run`, `lf ship`, and an explicit `lf task checkout` start the Task's
conversation in the background when an agent is configured and installed.
An in-place `lf -i task run` uses its current terminal instead.
`lf resume` in its worktree reattaches to it. Previously opened conversations
stay as they are, and starting stops at sixteen running Task conversations.
A running Graph keeps its conversation alive; abandoning the Task stops its
unviewed conversation. A conversation you are viewing stays open.
Preparing a conversation, including Desktop's preload, does not launch an agent.

Add `--open` to open the Task's primary conversation when launch returns,
including when the Task is waiting at an interactive step. It reuses the same
Session started in the background. A bare run opens at Graph registration;
`-b` and `-i` still wait for their foreground run before opening.

Opening follows [`lf open`'s host selection](workspaces.md#choose-where-it-appears):
a direct CLI invocation attaches in the current terminal; an agent tool call
opens a sibling tab. When the Graph runs on another Machine, the conversation
opens locally where you invoked the command. Preview (`--context` or `--explain`)
and `--quote` open nothing.

A failed launch opens nothing. An opening failure does not stop the Graph.
Fix the reported host or attachment problem, then use
`lf open --task LOO-12` to open the conversation without advancing the Graph.

Every agent that works on a Task starts from the same place: the Task's notes
and thread, its checkout and `scratch/`, and, in a Wave, the Wave's goal,
memory and KRs.

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
For `--run`, an interrupted launch can return `outcome_unknown` with the created
Task and `run_requested` milestone. Inspect `lf task status <ID>` before starting
anything: recovery never launches a second Graph when the first launch is
unconfirmed. A receipt for a removed Task does not recreate it.

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
Pass `--session` explicitly to keep a conversation. An agent uses its own
`LF_AGENT_CALLER.session_id` or looks it up with `lf session list --json`.
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
checkout is adopted. Closed Sessions and headless Runs cannot become primary.

## Steer it while it runs

```bash
lf task steer LOO-12 "keep the retry budget at 3"   # reaches its agents at their next step or turn
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
`lf session list`, `lf session history`, `lf repo plan`, `lf process list` and the
`lf desktop` reads, always with `--json`. Each line is
`{"answers", "unavailable", "body"}`, where `body` is
what `--json` prints. Write `{"id": 1, "args": {"query": "parser"}}` to stdin to
change what it reads, or `{"id": 2}` to ask for a fresh reading; the next line
answers that id. It stops when stdin closes. `lf repo plan --subscribe` also
keeps the repository's planning exchanged while it runs.

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
lf repo plan                 # which plan this repository reads, and what's pending
lf task claim LOO-12               # take it; the earlier owner's Sessions are told
lf task unassign LOO-12            # nobody owns it; Linear shows no assignee; history intact
```

Tasks, their threads and status sync through your Git remote by default, as a ref
(`refs/loopflow/planning/shared/main` on `origin`), never a branch. Everyone who
clones the repository reads and writes the same plan, with no account to
connect. Every command saves locally first and syncs after, so nothing waits on
the network.

Only planning travels: checkouts, Sessions, Graphs and processes stay on the
machine that ran them. Claims follow your Git `user.email`.

A different ref, or a plan only you see:

```yaml
# .lf/config.yaml
planning:
  provider: git
  shared: team        # refs/loopflow/planning/shared/team
  # personal: true    # refs/loopflow/planning/users/<key>; see `lf repo plan key`
```

Ref names are not access control; pick a remote whose readers should see the
plan. When two people edit the same field, the later edit wins and the other
value stays in `lf repo plan journal`, ready to copy back.

### Linear, Asana or Notion

```bash
lf account connect linear                  # or asana, or notion
lf wave plan connect --all --team-key LOO       # Linear: one Team per repository; Tasks become LOO-12
```

```yaml
# .lf/config.yaml, for Asana or Notion
planning:
  provider: asana                  # or notion
  team: "<team-gid-or-url>"        # Asana team, or the Notion page holding the databases
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
| create, update, steer, complete | the same change |
| start work on the Task | it moves to in progress |
| open, land or abandon its PR | a link to the PR and its state |

In Linear, a Wave is an Initiative and its planning is a Linear Project. Offline,
commands save locally and deliver on the next connected command;
`lf task sync LOO-12` retries one Task.

## Waves

A Wave keeps a longer objective moving across many Tasks: a goal, the memory
agents curate as they work, and its planning: Tasks and KRs. Every Task in it
starts with that context.

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
lf wave status billing                            # plan, Tasks, metrics and execution evidence
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
lf --wave billing set account.claude work@example.com     # the login that pays
lf --wave billing set agent codex                         # your agent here; the team's stays claude
lf set account.claude me@example.com --global             # every repository
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

Machine choices and ordered accounts are personal file settings too. Credentials,
observed availability and existing work placement remain runtime state. Agent choices
beat all `default_agent` fallbacks; within each class, personal settings beat shared
settings. [Configuration](config.md) owns the complete source/scope/target rules and
remote preference behavior. `lf wave status` explains each value's source.

A Wave without `config.yaml` reads these keys from `GOAL.md` frontmatter, which
released lf still reads (LOO-640 removes that).

### Plan it: KRs and targets

```bash
lf wave plan billing --json                     # its Tasks, KRs and metric targets
lf wave update-plan --wave billing --plan plan.json
```

```json
{"metric_targets":[],"krs":[{"text":"A customer's invoice matches metered usage to the cent.","holds":false}]}
```

The goal names who benefits; KRs prove it, observably, across a window. A
Wave's planning holds its Tasks, KRs and metric targets. `lf wave update-plan`
replaces the plan in place, and started Tasks keep their worktree and PR
wherever they are.

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

**Sync.** `lf task sync LOO-12` looks for its tracker creation now and re-sends
it with its saved ID when absent; between syncs, every `lf` command looks for an
uncertain creation once per window. `--retry-updates` retries saved field
changes after reading the tracker again. `lf wave sync` refreshes from Linear first.

Every flag: [`lf task`](lf-reference.md#lf-task), [`lf wave`](lf-reference.md#lf-wave).

Next: [Sessions](sessions.md), every conversation those Tasks ran.
