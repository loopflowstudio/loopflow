# Work

You've handed off a Task: `lf task create`, then `lf -b task run`. This page
picks up from there: steering it while it runs, seeing where everything
stands, sharing the plan with teammates, and grouping longer objectives into
Waves.

## Hand off more than one

```bash
pbpaste | lf task create                     # first line is the title, the rest its notes
lf task run LOO-12                           # returns once its Graph is running
lf task run LOO-13 --stack-on LOO-12         # builds on LOO-12's open PR before it merges
lf capture-tasks                             # talk it through; the Session files the Tasks
```

Each Task gets its own worktree beside your repository, its own branch and at
most one pull request. A stacked Task forks from its parent's PR; `lf sync`
carries the child through the parent's updates and squash merge.

Every agent that works on a Task starts from the same place: the Task's notes
and comments, its checkout and `scratch/`, and, in a Wave, the Wave's goal,
memory and KRs.

## Steer it while it runs

```bash
lf comment LOO-12 "keep the retry budget at 3"   # reaches its agents at their next step or turn
lf comment LOO-12                                 # read the thread: yours, teammates', the agents'
lf interrupt LOO-12                               # stop its running Graph
lf task hold LOO-12                               # hold future runs and CI repair; live processes finish
```

A comment saves locally first, then reaches every Session working on the Task at
its next supported boundary: before a Graph step or after an agent turn. Idle
Sessions stay idle; nothing is replayed into them. Comments from teammates, in
Loopflow or in your tracker, arrive the same way.

`lf task release LOO-12` lifts the hold without starting anything. To reopen a
conversation, use `lf resume` ([Sessions](sessions.md)).

## See where it stands

```bash
lf wave show                       # each Wave's plan and Tasks, sorted by what they need
lf task status LOO-12              # state, graph step, PR, and every Session and Graph on it
lf task diff LOO-12 --files        # what it changed so far
lf task wait LOO-13                # block until its run ends, with no agent polling
```

```bash
lf task list                                  # open Tasks in this repository
lf task list loo-60                           # identifier or title contains "loo-60"
lf task list --wave infra --state idle
lf task list --done --wave infra --since 7d   # closed in the last week
lf task list --watch                          # a JSON line each time the list changes
```

`lf task list` is the flat inventory. Every view reads this machine's store, so
it answers instantly and offline.

`--watch` also works on `lf task status <id>`, `lf wave show`, `lf wave status`,
`lf session list`, `lf history`, `lf planning status`, `lf ps` and the
`lf desktop` reads, always with `--json`. Each line is
`{"answers", "unavailable", "body"}`, where `body` is
what `--json` prints. Write `{"id": 1, "args": {"query": "parser"}}` to stdin to
change what it reads, or `{"id": 2}` to ask for a fresh reading; the next line
answers that id. It stops when stdin closes. `lf planning status --watch` also
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
until you complete it. Publishing, landing and CI repair are covered in [Ops](ops.md).

## Share the plan

```bash
lf planning status                 # which plan this repository reads, and what's pending
lf task claim LOO-12               # take it; the earlier owner's Sessions are told
lf task unclaim LOO-12             # hand it back, history intact
```

Tasks, comments and status sync through your Git remote by default, as a ref
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
  # personal: true    # refs/loopflow/planning/users/<key>; see `lf planning key`
```

Ref names are not access control; pick a remote whose readers should see the
plan. When two people edit the same field, the later edit wins and the other
value stays in `lf planning journal`, ready to copy back.

### Linear, Asana or Notion

```bash
lf account connect linear                  # or asana, or notion
lf wave connect --all --team-key LOO       # Linear: one Team per repository; Tasks become LOO-12
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
| create, edit, comment, complete | the same change |
| start work on the Task | it moves to in progress |
| open, land or abandon its PR | a link to the PR and its state |

In Linear, a Wave is an Initiative and its plan is a Linear Project. Offline,
commands save locally and deliver on the next connected command;
`lf task sync LOO-12` retries one Task.

## Waves

A Wave keeps a longer objective moving across many Tasks: a goal, the memory
agents curate as they work, and a plan of Projects and KRs. Every Task in it
starts with that context.

### Give it a goal and memory

```bash
mkdir -p .lf/wave/billing
$EDITOR .lf/wave/billing/GOAL.md       # who benefits, and what improves
lf wave ensure billing                 # register it and prepare its Project
lf task create --wave billing --title "Meter usage events"
```

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

### Run a pass, or keep a conversation

```bash
lf --wave billing wave-operate "invoices first"   # one finite pass over its Tasks
lf resume --wave billing                          # the Wave's ongoing conversation
lf wave status billing                            # plan, Tasks, metrics and execution evidence
```

`wave-operate` gives every started Task you own a disposition: moving, acted on,
waiting on a named person, deferred, or unknown. It leaves running Graphs alone,
reruns what remains of a stopped one, and lists unstarted backlog for you to
pick. It never edits the plan or files Tasks. Teammates' Tasks are listed and
left alone.

The Wave's conversation carries the same pass each time you return. Between
turns, running Graphs keep going on their own. For a schedule, put an ordinary
command in your own crontab:

```bash
# 0 9 * * *  cd ~/src/product && lf -b --wave billing wave-operate
```

### Set it up once

```yaml
# .lf/wave/billing/config.yaml: committed, inherited by the Wave's Tasks
graph: code
agent: claude
skill_agents:
  implement: claude:opus
context_budgets:
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
changes; launches in the Wave layer it over the repository's and your
`~/.lf/config.yaml`. Repository settings (`planning`, `release`, `exclude`,
`merge_method`, `land`, `pr`) are an error there. It also carries the Wave's
`id`, `linear.initiative` and `chat`. A Task with no Wave uses the repository's
settings.

`lf set` never edits committed config. It stores your own value on this
Machine for the repository, a Wave (`lf --wave <wave> set`) or every repository
(`--global`); without `--wave` it sets the repository, even in a Wave's Task
worktree. Your `graph`, `agent` or `skill_agents.<skill>` wins over the
committed value for you only, and `lf wave status` reports it as `personal`.
Machines (`machine`, `machine.tasks`, `machine.session`) and account routes
(`account.<provider>`) exist only this way; a launch uses the nearest of Wave,
repository and global. `lf unset <key>` hands the choice back.

A Wave without `config.yaml` reads these keys from `GOAL.md` frontmatter, which
released lf still reads (LOO-640 removes that).

### Plan a chapter: Projects and KRs

```bash
lf wave update-plan --wave billing --plan plan.json
```

```json
{"metric_targets":[],"krs":[{"text":"A customer's invoice matches metered usage to the cent.","holds":false}]}
```

The goal names who benefits; KRs prove it, observably, across a window. A
Wave's current Project holds its Tasks, KRs and metric targets. Projects
created together share a chapter name, such as `2026-10`; completed Projects
keep their history. `lf wave new-chapter` rotates to the next one, moving
started Tasks with their worktree and PR and leaving unreviewed backlog where
it was.

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

**Sync.** `lf task sync LOO-12 --retry-creation` retries an uncertain tracker
creation with its saved ID; `--retry-updates` retries saved field changes after
reading the tracker again. `lf wave status --sync` refreshes from Linear first.

Every flag: [`lf task`](lf-reference.md#lf-task), [`lf wave`](lf-reference.md#lf-wave).

Next: [Sessions](sessions.md), every conversation those Tasks ran.
