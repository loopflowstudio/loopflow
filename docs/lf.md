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

Selected Wave goals are supplied once as complete `GOAL.md` documents. Repeated
requests for the same document do not repeat its contents; distinct memory files
remain separate even when their text matches. IDE launches with Wave documents
use the assembled prompt so those references reach the provider.

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
lf --task EXP-12 skill task/session # another conversation about that Task
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
| Wave | `lf --wave growth wave/operate` | `lf session ensure -w growth` |
| Task | `lf task/operate EXP-12` | `lf session ensure --task EXP-12` |

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
lf pr publish                      # push a ready PR
lf submit                          # prepare for a reviewer's merge click
lf arm                             # prepare and request auto-merge; return
lf land                            # record delivery and return
lf land --wait-and-fix              # wait for merge and repair failing CI
lf sync --plan                     # preview integration with main or stack parent
```

A Task has zero or one PR. Choose one delivery operation for the desired
endpoint. Submit, arm and land own preparation and integration; publish does not
sync. PR operations also work on ordinary branches without creating a Task.
Arm and bare land return after recording delivery. `--wait-and-fix` observes and repairs
its PR every 15 seconds for up to 30 minutes, including with Desktop closed;
timeout or interruption leaves the intent intact. Repeating land for a merged
Task PR succeeds and reports whether Task completion remains.
`lf pr reconcile` checks recorded landings once. In a Task checkout it can also
recover an existing PR whose GitHub identity is missing, without publishing.
Ambiguous provider evidence remains unresolved. `lf ci watch` repairs actionable
CI failures while it runs.

```bash
lf task follow-up EXP-12 --title 'Verify the installed command' --notes 'Follow-up to EXP-12: after the release is installed, run the accepted scenario and record its version and result; investigate a failure.' --due 2026-10-09
lf task follow-up EXP-12 --finish 'The installed check has its own Task'
lf task complete EXP-12
```

After verified merge, file accepted remaining obligations as linked follow-up
Tasks, or record `lf task follow-up EXP-12 --none 'No accepted work remains'`.
Use `--existing ISSUE` to link work already filed and `--wave WAVE` to choose its
owner. Repeat filings before `--finish`; retries retain the original child
identity and Project even after chapter rotation. Inspect receipts and retry
identical input after an uncertain response. The source completes after filing,
without waiting for those children to finish. Preserve accepted later checks in
PR copy before landing clears scratch.

`ship` runs gate, waited landing, then follow-through. A bare delivery, manual
GitHub merge or stopped finishing Flow leaves **Merged · Follow-through pending**.
The next Task/Wave operation checks for live work, then runs `finish-delivery`
if needed. It does not rerun gate or re-arm the merged PR.

Due follow-ups return on the owning Wave's next operation, even if unstarted.
Their briefs carry release prerequisites, expected evidence and any timezone.
A due date is not an alarm or proof of success. Only a concrete check already
authorized in the brief can start unattended. Filing installs no schedule;
without an installed Wave schedule, there is no automatic check between passes.

`lf task complete EXP-12` is an alias for `lf task move EXP-12 end`. Both require
merge and a durable none/filed disposition when the Task has a PR. A PR-less
Task can finish with research files or commits; they do not create a PR
requirement. Old Session turns and unknown process exits cannot veto completion.
Live controls and execution history remain intact; unsafe or occupied checkouts
are retained. An open or closed-unmerged PR still requires delivery or explicit
abandonment. A different PR needs another Task, stacked if dependent:

```bash
lf checkout EXP-13 --stack-on EXP-12 --design scratch/child-design.md
lf --task EXP-13 pr publish
```

A published parent PR, including a draft, is enough to start and publish the
child. Prepare a child-specific design; checkout transfers it without overwriting
newer child work. `lf sync` follows parent updates and its squash merge while
preserving the child's identity, PR, Session and design.

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
watcher running during `land --wait-and-fix`: waited landing admits the same repair.
After bare land returns, failures wait for a watcher or another waited landing.
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
launches check destination access with the inherited restrictions. A foreground
credential lease cannot authorize a detached remote process after it expires.
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
lf --machine mini session list
lf --machine mini --task LOO-123 implement
lf --machine mini machine add builder
```

`--machine <label-or-id>` runs the entire command on that machine in its saved
repository. Task, worktree and Wave selectors resolve there. `--secret NAME` and
`--forward-agent` require `--machine`; the saved repository is set by `machine add`.

Save an SSH destination once, then use its label. List, rename and remove saved
connections with `lf machine list`, `lf machine rename mini builder` and
`lf machine remove builder`. Removing a connection leaves remote work running.
Interactive add offers to install a missing `lf`; an existing installation stays
untouched. Status reports connection failures and recovery commands without prompting.
Connections are reused for 60 idle seconds through Loopflow's private SSH socket.
See [machine connections](architecture/machines.md) for repository paths and version checks.
