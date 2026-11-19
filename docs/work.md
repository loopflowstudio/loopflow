# Work

You've run your first Task. This page covers running several Tasks in parallel,
stacking dependent changes, steering work in flight, sharing plans with your
team (via Git, Linear, or Asana), and organizing ongoing areas of work into
Waves.

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
and (when in a Wave) the Wave's goal, memory, and KRs.

**Instant checkouts.** A new Task branches from `origin/<default>` as of the
last background fetch (its first `lf sync` catches up). Loopflow keeps one
warm worktree at `<repo>.ready` beside your repo so creation is instant, then
replenishes it in the background. `lf wt prune` leaves it alone, and deleting
it is safe.

**Stacking Tasks.** Pass `--stack-on LOO-12` when starting a dependent Task, or
run `lf sync LOO-12` inside an existing Task's worktree to stack it on `LOO-12`.
`lf sync` carries the child through the parent's updates and squash merge;
`lf sync main` detaches it back onto the default branch.

**Task conversations.** `lf task run`, `lf ship`, and `lf task checkout`
prepare the Task's primary conversation and the local server starts it in the
background within seconds (with no background conversation cap). An in-place
`lf -i task run` uses your current terminal instead. Pass `--open` to attach
when launch returns: a direct CLI call attaches in the current terminal, while
an agent tool call opens a sibling tab ([Workspaces](workspaces.md#choose-where-it-appears)).
If opening fails, the Graph keeps running — fix the host issue and run
`lf open --task LOO-12`.

Task conversations stay alive while the Graph runs and come back automatically
after a kill, crash, or reboot ([Sessions](sessions.md#after-a-kill-a-crash-or-a-reboot)).
A conversation you explicitly quit stays quit until you `lf resume --task LOO-12`.

**Investigating a failure.** Run `lf incident` inside any Task checkout (even
after the Task is done) to run an independent investigation with that Task's
context without changing its Graph position or completion state.

## Steer work in flight

```bash
lf steer LOO-12 "keep the retry budget at 3"   # delivered at the next step or turn
lf task status LOO-12 --json                   # thread: your steers, teammates' comments, agent reports
lf task interrupt LOO-12                       # stop its running Graph
lf task unassign LOO-12                        # release ownership; stops ship and CI repair
```

Steers save locally first, then reach every Session working on the Task at its
next provider boundary (before a Graph step or after a turn). An idle Task
conversation is woken with it. Comments from teammates in Linear or Asana
arrive the same way.

`lf task claim LOO-12` takes ownership without starting a run; `lf task run`
claims and runs. To jump into the conversation directly, run
`lf resume --task LOO-12` ([Sessions](sessions.md)).

## See where everything stands

```bash
lf wave show                                   # each Wave's plan and Tasks, sorted by what needs attention
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

Every read queries this machine's local store, so it answers instantly and
works offline.

`--subscribe --json` also works on `lf task status <id>`, `lf wave show`,
`lf wave status`, `lf session list`, `lf session history`, `lf repo tracker`,
and `lf desktop` reads. Each line is `{"answers", "unavailable", "body"}`,
where `body` matches `--json`. Write `{"id": 1, "args": {"query": "parser"}}`
to stdin to change parameters or `{"id": 2}` to request a refresh; close stdin
to stop. `lf repo tracker --subscribe` also keeps planning synced while it runs.

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

Adopting preserves the branch, uncommitted and untracked files, and the
Session's provider history and live terminal attachment.

- Plain `lf task create` always files new work, even inside an existing Task
  checkout or Session. Pass `--worktree` or `--session` explicitly to adopt.
- `--worktree` without `--run` adopts the checkout only; adding `--run` also
  starts the Task's conversation and Graph.
- Main and Wave checkouts stay shared: binding a Session there transfers the
  conversation to the Task without moving its directory until you run
  `lf task checkout`.
- Binding an unfinished interactive Session to a Task that already has a
  worktree switches that Session's working directory to the Task's worktree
  (existing files stay where they were). Closed or headless Sessions cannot
  become primary.
- In Loopflow Desktop, use **Create Task from this conversation**, **Bind to
  Task**, or **Make primary Task conversation** from the Task menu or command
  palette.

## Finish

```bash
lf --task LOO-12 land --wait-and-fix   # merge its PR, repairing CI until it lands
lf task complete LOO-12                # mark the Task done
```

A Task whose Graph reaches `end` requests completion automatically (waiting
for its PR to merge if it has one). If you land a PR outside its Graph, mark
the Task done with `lf task complete`. See [Pull requests](ops.md#pull-requests)
for publishing, submitting, and CI repair.

## Share the plan with your team

```bash
lf repo tracker                    # active tracker and any pending sync items
lf task claim LOO-12               # claim ownership; previous owner's Sessions are notified
lf task unassign LOO-12            # clear ownership; history stays intact
```

### Git ref (default, zero accounts)

By default, Tasks, threads, and status sync through a Git ref on `origin`
(`refs/loopflow/planning/shared/main`) — never a branch. Anyone who clones the
repo reads and writes the same plan with no external account. Every command
saves locally first and syncs asynchronously, so nothing blocks on the network.

Only planning state travels: worktrees, Sessions, Graphs, and processes stay on
the machine that ran them. Claims follow your Git `user.email`. A repo with no
`origin` keeps its plan locally and publishes automatically once you add one.

Use a different shared ref or a personal-only ref in `.lf/config.yaml`:

```yaml
# .lf/config.yaml
tracker:
  git:
    shared: team          # refs/loopflow/planning/shared/team
    # personal: true      # refs/loopflow/planning/users/<key>; see `lf repo tracker key`
```

### Linear or Asana (two-way sync)

Coworkers do not need to install `lf` — they keep working in Linear or Asana,
and changes stay synchronized both ways:

```bash
lf account connect linear                       # or: lf account connect asana
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

| You in Loopflow… | Coworkers see… |
|---|---|
| Create, edit, steer, complete, reorder, or delete | The same change |
| Start work on the Task | Issue moves to In Progress |
| Open, land, or abandon its PR | Linked PR and its state |

In Linear, a Wave maps to an Initiative and its current Project maps to a
Linear Project. In Asana, it maps to a team project.

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
  Git tracker conflicts work the same way.

A Wave not yet linked to a Linear Initiative keeps its Tasks local until you
run `lf wave plan connect <wave>`.

## Waves

A Wave groups related Tasks under a lasting objective (`GOAL.md`), curated team
memory (`MEMORY.md`), KRs, and shared defaults. Every Task in the Wave starts
with that context.

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

`MEMORY.md` beside it holds durable context: decisions, dead ends, and facts
downstream Tasks need. Agents update it as work ships, and `realign` keeps
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
`lf.config.local.yaml` (edited by `lf set`, shared across this clone's
worktrees) overrides both. Use `lf unset <key>` to remove a personal override.
See [Configuration](config.md) for the complete precedence rules.

### Operate a Wave

```bash
lf --wave billing wave-operate "invoices first"   # one finite pass over its Tasks
lf resume --wave billing                          # open the Wave's ongoing conversation
lf wave status billing                            # plan, Tasks, KRs, and runtime state
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

### KRs, nested Waves, and Discord

**KRs and metric targets.** A Wave's current Project holds its Tasks, KRs, and
targets:

```bash
lf wave plan billing --json
lf wave update-project --wave billing --file project.json
```

```json
{"metric_targets":[],"krs":[{"text":"A customer's invoice matches metered usage to the cent.","holds":false}]}
```

Updating a Project replaces its KRs and targets in place; started Tasks keep
their worktrees and PRs.

**Nested Waves.** Create `.lf/wave/infrastructure/release/GOAL.md` and address
it as `infrastructure/release`. Agents read the parent's top-level Markdown,
then the child's (siblings stay out). A root `MEMORY.md` applies even with no
Wave selected.

**Rename or remove a Wave.**

```bash
lf wave rename billing payments          # moves files on main; keeps Tasks, ID, and planning
lf wave remove payments --preview        # preview what happens to Tasks and Sessions
lf wave remove payments                  # moves open Tasks to the parent Wave or repository
lf wave remove payments --delete-backlog # delete unstarted Tasks instead; started ones always move
```

Removal retires the Wave and its children, folds a child's `MEMORY.md` into its
parent, and keeps completed history readable via `lf wave show payments`.

**Discord bridge.** Talk to a Wave from a Discord channel:

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

**Graph selection.** A Task keeps the Graph it took up on its first run. Before
then, precedence is: explicit `lf task run LOO-12 GRAPH` → personal
`wave.graph` → Wave `config.yaml` `graph:` → repo `.lf/config.yaml` `graph:` →
builtin `code`. `lf task run LOO-12 --from demo` starts at any step; starting
from an interactive step places the Task there without advancing past it.

**Membership and lifecycle.** `lf task move LOO-12 --wave billing` (or
`--no-wave`) moves a Task while keeping its Session, checkout, PR, and Graph.
`lf task complete` updates status only; `lf task reopen` keeps the same PR;
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
`lf wave sync` refreshes from the tracker first. If Linear updates an issue or
Project without bumping `updatedAt`, Loopflow adopts the fresh reading and
keeps your local edits queued. Moving a Project to another Initiative in Linear
is held until moved back, because the Initiative determines which Wave owns it.

Every flag: [`lf task`](lf-reference.md#lf-task), [`lf wave`](lf-reference.md#lf-wave).

Next: [Sessions](sessions.md) — persistent conversations, multi-view attach,
and crash recovery.
