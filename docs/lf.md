# lf CLI

```bash
mkdir first-project && cd first-project
git init
lf -b : "Write a README explaining this project"
lf monitor                         # waiting, blocked, active, finished
lf account                         # live access and capacity by account
```

A local result needs a provider login and Git. It needs no Linear account or
registered Task. Connect a provider with `lf account connect codex EMAIL`, or
use an existing provider login. `lf account --cached` inspects saved evidence
without contacting providers.

## Open a Wave for work

```bash
lf wave ensure infrastructure --json
lf wave bind-project intelligence <project-uuid> --json
lf wave ensure intelligence --json
```

Ensure reuses the shared configured Project, activates it if needed, or recovers
one reserved creation. Existing Project selection is explicit by UUID. Names,
chapters and default Flows are optional. Failed reads preserve the binding;
status and roadmap never create Projects. Desktop prepares the Project on opening
and offers Retry while retaining the cached plan and independent conversation.

For coordinated chapters, plan KRs first and retain exact destination IDs:

```bash
lf repo new-chapter October --plan scratch/chapter-plan.json --dry-run --json
lf repo new-chapter October --plan scratch/chapter-plan.json --json
```

Use `lf wave new-chapter <wave> October --plan scratch/chapter-plan.json` for one
Wave. Retry the same retained input after interruption. Started work carries its
identity; unreviewed backlog stays in its prior Project. Admit new Task candidates
separately after creation. Desktop’s **Realign Projects…** previews a retained plan
file and applies the exact bytes reviewed. See [planning](architecture/planning.md)
for the input format.

## Use one Machine

Ordinary commands, Task Flows and agent tools use the installed `lf` and
`~/.lf`. Source builds forward there too. For an explicit disposable experiment:

```bash
LF_HOME="$(mktemp -d)" target/debug/lf wave list --json
```

Children stay in that Machine. Experiments start empty; Loopflow does not upgrade,
repair or preserve them. Use a fresh directory when its schema changes.

## Select an agent

```bash
lf -a codex debug
lf --agent claude:opus debug
```

`--agent` / `-a` selects the harness and optional model as `harness[:model]`.

## Run a Flow interactively

```bash
lf -i -a claude run my-flow
lf -b -a claude run my-flow # run every step headlessly
```

Each skill opens in the native agent conversation. Exit the conversation
successfully to advance to the next step; an interrupted or failed step stops
the Flow. Without either flag, an attached terminal selects interactive mode,
just like a standalone skill. Task Flow launches preserve the same choice.

## Select where work happens

```bash
lf --task EXP-12 skill design       # contribute in the Task's checkout
lf --wt csv-export : "Add CSV export"
lf --wave exports : "Review the goal" # add context in the current directory
lf -b task run EXP-12       # place the Task, run its default
lf -b task run EXP-12 pursue # take the workflow edge that runs pursue
lf task run EXP-12 end       # take an edge that runs nothing
lf task move EXP-12 demo     # put the Task at a node, running nothing
lf --task EXP-12 run incident # run a Flow without moving the Task
lf task run EXP-12 --reason "take the smaller approach"
```

`--task` and `--wt` select a location. `--wave` supplies context and identity;
it cannot override a Task's owning Wave. `task run` places the Task's
worktree, defaults to its Project's Flow, then starts
`lf --task ISSUE run FLOW`: it prints the Flow's output and returns when the
Flow ends. A Flow that fails is started again from its first step, three
times at most; one that is blocked, interrupted or waiting on a landing is
not. Background it yourself when you will not wait. It never continues
an earlier Flow. `--reason` publishes direction to the Task first. Every
Flow process for a Task is equally its work.

Flow output shows each step's position and name followed by the agent's messages.
Use `lf -v -b task run EXP-12 pursue` for context token accounting and INFO
diagnostics. Codex Session capture uses a temporary launch profile so terminal
wrapper hooks stay active. No hook setup or saved trust changes are required.

A Task takes up its Project's workflow and moves through its nodes.
Each `task run` takes one edge leaving the current node: the only one, or the
one whose Flow you name. At a node the Task waits on you in its conversation;
no command approves a node. A Flow that does not leave the node is refused
with the edges that do. A Flow that stops, or fails every attempt, leaves the Task on its edge until
you choose again or `lf task move EXP-12 <node>` puts it at a node outright. `lf task run EXP-12 code` takes up another workflow from
its start. See [workflows](authoring.md#workflows).

```bash
lf project workflow list                 # Workflow definitions and validity
lf project workflow set PROJECT code     # selection for future Tasks
lf project workflow customize feature    # print its local source path
lf task workflow show EXP-12             # captured graph, position and history
lf task workflow restart EXP-12          # move to start; retain graph and history
lf flow list                             # autonomous Flow definitions
lf flow customize pursue                 # print its local source path
```

A Flow whose driver died leaves its Processes as history. No command resumes it. To change direction or recover:

```bash
lf task interrupt EXP-12             # end the active provider turn
lf task status EXP-12                # the latest Flow and all Task work
lf flow show ID --processes --json    # one Flow's steps, by its Flow process ID
lf -b task run EXP-12       # run fresh work
```

Claude and Codex terminal launches load the complete assembled context from a
system instructions file. The first user message is a short execution request;
Wave memory, scratch, diffs, clipboard, skills and Task briefs stay out of the
command-line argument. Provider refusals remain visible in the native terminal.

Selected Wave goals are supplied once as complete `GOAL.md` documents. Repeated
requests for the same document do not repeat its contents; distinct memory files
remain separate even when their text matches.

Launching a Flow for an existing Task uses valid cached planning regardless of age.
Known invalidation, removal, terminal state or ownership changes still block.
`--reason` requires successful Linear publication before the launch. Status
retains the planning observation's original age.

## Connect planning and create work

```bash
lf list wave                       # authored goals, including unconnected Waves
lf account connect linear
lf repo connect --all --team-key EXP # connect goals and choose the Task prefix
lf task create --wave exports --title "Add CSV export"
lf checkout EXP-12                  # prepare its checkout without execution
lf task run EXP-12
lf roadmap --json                   # plans and Tasks across Waves
lf wave status exports --json       # one Wave's detailed evidence
```

Connected planning needs a Linear login and repository Team. Connection names
missing access and the command to obtain it. Authored Waves remain discoverable
before connection. Planning setup is separate from direct local execution.

After Linear accepts a Task creation or update, Loopflow confirms that issue
directly without another Wave-wide snapshot. If Linear commits but issue
confirmation fails, the error names the retained issue. Retry the same command
with the original creation options to reuse it without filing a duplicate.

## Inspect and continue

```bash
lf ps                              # one live process snapshot
lf top                             # refresh on a terminal
lf mon list --json                 # bounded Process history with a next cursor
lf mon active --watch --json        # NDJSON until stdin closes
lf mon show SESSION --final         # provider conclusion
lf usage --days 30                  # measured consumption and missing evidence
lf usage --task LOO-265 --context   # each step's input by source, flagged over budget
lf mon show SESSION --context       # one step: instructions, memory, scratch, goal, steers, carried, tools
lf session ensure                   # this repository's one ongoing conversation
lf session ensure -w growth         # a Wave's one ongoing conversation
lf session ensure --task EXP-12     # a Task's primary conversation
lf --task EXP-12 skill task-session # another conversation about that Task
lf session connect SESSION         # continue a conversation
lf session replace SESSION         # fresh conversation for the same scope
lf context --task EXP-12 --json     # effective context limits, sources and usage
lf wt timing                        # how long real `lf wt list` runs took here
```

The repository and each Wave have one ongoing conversation. `session ensure`
finds it or starts it, a Wave's with its goal and memory, and repeats return the
same Session. `replace`
stops that conversation, keeps it as history, and starts a fresh one.

A Task's primary is one of its own conversations: the one named with
`--choose SESSION`, else its only unfinished interactive conversation, else the
most recently used. A Task with none gets a new one in its checkout. The others
stay open, and listing Sessions never picks or starts one.

Use `lf resume` in the Task checkout, or `lf session connect SESSION`,
to reopen the same conversation after a failed terminal launch. Retained native
history survives retries. An active owner or a legacy provider without process
evidence still needs its supported connection or recovery path.

Session connect, rename and bind take the durable Session ID shown by
`lf session list`. Capture keys and history prefixes select retained inputs for
inspection and replay; they do not select these Session actions.

| Scope | One finite pass | Ongoing conversation |
| --- | --- | --- |
| Repository | `lf operate` | `lf session ensure` |
| Wave | `lf --wave growth wave-operate` | `lf session ensure -w growth` |
| Task | `lf task-operate EXP-12` | `lf session ensure --task EXP-12` |

Each conversation is its scope's operator and carries the matching operate
procedure. It leaves running Flows alone, reads a stopped or failed Flow before
running its remaining work fresh, and names what waits on you, including a Task
at a workflow node. Unstarted backlog stays unstarted. A conversation acts
only during a turn: between turns, running Flows and any installed background
checks continue on their own.
An open conversation keeps the instructions it launched with; `replace` it
after an upgrade.

Monitor keeps live processes, recorded outcomes and missing observations distinct.
A process has a durable `lfid` and an optional Unix `pid`. Inspect by LFID; PIDs
can be reused. `parent_process_lfid` names the recorded parent, and historical
rows without PID evidence keep `pid: null`.
Its overview explains each item's state and next action. A mechanical Process has
no provider conclusion. JSON reads emit one document; the active watch emits
newline-delimited snapshots. Progress and errors go to stderr.

`lf context` previews local launch input; [configure context budgets](config.md#context-budgets)
in existing personal, repository or Wave settings.

## Publish and finish

```bash
lf pr publish                         # push a ready PR
lf submit                          # prepare for a reviewer's merge click
lf arm                             # prepare and request auto-merge; return
lf land                            # record delivery and return
lf --task EXP-12 land               # complete after verified merge
lf sync --plan                     # preview integration with main or stack parent
lf wt create csv-export
lf check                   # inspect release eligibility
```

Choose one delivery operation for the desired endpoint. Submit, arm, and land
own preparation and integration; publish does not sync. PR operations work
on ordinary branches without creating a Task. Verified merge normally completes its Task. Arm and land return after recording delivery;
`lf pr reconcile` checks it once and settles verified merges. In a Task checkout,
it also recovers an existing PR whose GitHub identity is missing from the Task,
without publishing or rotating the branch. Multiple PRs
for the recorded branch remain unresolved. `lf ci watch`
starts a ci-fix when a recorded landing fails its required checks.

```bash
lf task follow-up EXP-12 --outcome 'Installed latency meets the budget' --evidence 'Warm p95 below 1s over 20 samples' --check-at 2026-10-09T17:00:00Z
lf land
lf task follow-up EXP-12 --clear 'Published v0.14: 20 samples, p95 0.8s'
```

Record accepted remaining work before delivery. Its outcome, evidence condition
and next check stay visible in Task status and Desktop. An overdue check calls
for evidence or a scope decision; time and green CI never establish production
success. `--next <slug>` keeps genuinely unfinished PR work open. Older keep-open
requests without a stated outcome surface for an explicit scope decision.

`lf task move EXP-12 end` records an explicit completion. Old Session turns,
reserved inputs and unknown process exits cannot veto it. Execution history and
live process controls remain intact; uncertain or occupied checkouts are retained.
Cancellation follows the same separation. Open PRs and additional committed work
still need delivery or explicit abandonment.

## Check authorized deliveries in the background

```bash
lf cron sync --repo                 # install this Machine's minute delivery check
lf task reconcile --json            # check recorded deliveries once
lf task automation --json           # schedule coverage and CI repair holds
lf cron sync --repo --disable       # remove the schedule
```

Checks continue with Desktop closed while the placed Machine's user is logged in.
They record CI failures and settle verified merges. Flow recovery belongs to its
caller: inspect execution and effect history before launching fresh work.

## Repair failed CI

```bash
lf ci watch               # watch this repository's PR checks until you stop it
lf ci watch --once        # check every open PR once and exit
lf ci watch --install     # keep it running as a launchd service
lf ci watch --uninstall
lf ci watch --status      # live or not, last poll, what it started and why
```

The watcher polls GitHub about once a minute and starts one ci-fix when a PR
with a recorded landing (`lf arm`, `lf land`) fails its required checks. A
failing PR nobody armed is reported, not repaired; so is a PR with no Task.
Loopflow Desktop runs the same command for each open repository and stops it
on quit. A second copy stands by behind a live one. Nothing depends on the
watcher running: without it, failures are recorded and wait.
An idle check launches no provider. Existing work and GitHub merge requests
continue when the schedule is disabled or a Task is held.

Set repository defaults in `.lf/config.yaml`:

```yaml
automation: {retries: 1, timeout_reruns: 1}
```

Use `lf task automate EXP-12 off` to hold future CI repair, and
`lf task automate EXP-12 on` to enable it again. These settings never resume a
Flow. One failed repair startup may retry; a completed blocked repair waits for
changed evidence.
Pending or missing CI checks allow one timeout diagnosis/rerun after 30 minutes.
Use an always-available Machine for progress through laptop logout or shutdown.

## Accounts and children

```bash
lf account connect codex work@example.com --chrome-profile Work
lf --account codex=work@ run code
lf --only-account codex=work@ run code
lf account route set codex work@ personal@
```

A preference permits fallback; a restriction limits spending. A Flow takes
one selection per provider and carries it through its child steps. Remote
launches install missing selected logins in the foreground and use the same
account on the target. Credentials remain resident there.
Missing or expired capacity remains unknown, never zero or unlimited.

## Discover commands

```bash
lf help
lf help --all
lf help feature
lf list skill
```

Use shortcuts: `lf land`, `lf ps`, and `lf mon list`.
Omit owners or abbreviate command names when the result is unique. Ambiguous
shortcuts list their matching command paths. Exact commands take precedence
over authored definitions; `lf skill NAME` and `lf flow NAME` select a kind.

Help and catalog reads launch no agent. The [command reference](lf-reference.md)
names canonical owners and arguments. [Authoring](authoring.md) explains workflows;
[configuration](config.md) covers inherited launch defaults.

## Desktop navigation

```sh
open 'loopflow://task/LOO-303'
lf roadmap --task LOO-303 --all --json
```

Task links open details without starting work, including retained and completed
Tasks. Add a percent-encoded `repo` query to narrow duplicate issue identifiers.
Press ⌘K in the desktop to search Waves, Tasks, Sessions and Flow and Workflow definitions.
Task destinations open details; selecting a Flow opens its folded graph.

### Inspect all work in a Task

```bash
lf task status LOO-358 --json
lf session list --task LOO-358 --interactive all --history
```

`lf session list` defaults to unfinished interactive conversations.
`--waiting` narrows that selection to conversations waiting on you. In Desktop,
a program's OSC 7501 report takes precedence: any blocked record means Waiting,
as does idle for an interactive Session. Working, done, error and explicit clear
suppress the quiet-time inference for that provider generation. A blocked child
still counts when its parent reports working.

Without reports, the existing provider stream supplies questions, hand-back and
the two-minute quiet rule when no tool call remains open. A long silent step can
still show up there. Plain shell panes show their own reports without creating a
Session. The focused pane's report appears in the workspace breadcrumb header
and pane strip; its message is literal text. Reports never complete work.
Desktop must be observing the terminal; detached relay observation and lf's own
terminal emission remain follow-up work.
`--all` changes repository scope, `--interactive all` includes background work,
and `--history` includes completed conversations and historical reviews.

Task status lists Sessions, Flows and Processes from the checkout and explicit binds,
including headless and completed work. A Task on a workflow also shows its
nodes, edges, position and the edges it has taken. No Flow is privileged; the execution
line observes the most recently launched one. Live or unresolved work preserves
the checkout. A stopped Flow is history and blocks nothing.

## Keep a document workspace

```bash
lf session ensure                    # persistent repository conversation
lf session ensure --wave product     # separate persistent Wave conversation
```

Inside the conversation's persistent checkout:

```bash
lf commit -m "Record accepted decisions" wave/product/MEMORY.md
lf pr publish
```

`lf wt create planning --persistent` also creates or reuses an independent document
workspace; enter its printed path before editing.

Primary conversations reuse their respective worktrees, including after replacement.
They display a workspace without gaining Task membership. Moved checkouts are
rediscovered; missing checkouts recover committed branch state. Live conversations
keep their placement until an idle driver boundary.

Persistent workspaces retain scratch locally through commit, sync, publication and
landing. Selected-path commits preserve unrelated staged edits. Publication pushes
the committed range, leaving later local edits alone. Inspect the complete range
before publishing. PR landing clears scratch in non-persistent workspaces, whether
or not the work belongs to a Task.

If a sync resolver creates a file where an untracked file was stashed, sync restores
the original and keeps the resolver's file beside it as `<name>.lf-sync-1` (or the
next unused number). Read both notes and reconcile them locally. Tracked conflicts
retain the recovery stash and report its identity; resolve them before restoring it.

Run `lf sync --plan` and `lf sync` at a deliberate maintenance boundary and after a
merged document PR. Network failures leave local work usable; conflicts stay visible
and use `lf sync --continue` or `lf sync --abort`. Persistent branches remain reusable
and survive automatic pruning. Memory updates do not require PRs or a schedule.

## Work on another machine

```bash
lf machine add mini --repo '~/src/project'
lf machine status mini
lf machine connect mini codex work@example.com
lf --machine mini session list
```

`--machine <label-or-id>` runs the entire command on that machine in its saved
repository. Task, worktree and Wave selectors resolve there.
`--forward-agent` requires `--machine`; the saved repository is set by `machine add`.

Save an SSH destination once, then use its label. List, rename and remove saved
connections with `lf machine list`, `lf machine rename mini builder` and
`lf machine remove builder`. Removing a connection leaves remote work running.
Interactive add offers to install a missing `lf`; an existing installation stays
untouched. Status reports connection failures and recovery commands without prompting.
Connections are reused for 60 idle seconds through Loopflow's private SSH socket.
See [machine connections](architecture/machines.md) for repository paths and version checks.
