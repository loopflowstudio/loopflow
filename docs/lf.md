# lf CLI

```bash
mkdir first-project && cd first-project
git init
lf --mode batch : "Write a README explaining this project"
lf monitor                         # waiting, blocked, active, finished
lf account                         # live access and capacity by account
```

A local result needs a provider login and Git. It needs no Linear account or
registered Task. Connect a provider with `lf account connect codex EMAIL`, or
use an existing provider login. `lf account --cached` inspects saved evidence
without contacting providers.

## Use one Home

Ordinary commands, Task workers and agent tools use the installed `lf` and
`~/.lf`. Source builds forward there too. For an explicit disposable experiment:

```bash
LF_HOME="$(mktemp -d)" target/debug/lf wave list --json
```

Children stay in that Home. Experiments start empty; Loopflow does not upgrade,
repair or preserve them. Use a fresh directory when its schema changes.

## Select where work happens

```bash
lf --task EXP-12 skill design       # contribute in the Task's checkout
lf --wt csv-export : "Add CSV export"
lf --wave exports : "Review the goal" # add context in the current directory
lf --task EXP-12 flow start          # start or continue its managed Flow
lf --task EXP-12 flow start incident # choose a template for new work
```

`--task` and `--wt` select a location. `--wave` supplies context and identity;
it cannot override a Task's owning Wave. A named direct Flow creates an
independent FlowSession. `flow start` preserves the Task's selected Flow and
saved progress. `flow resume ID --retry` retries a saved boundary; `task restart`
explicitly replaces the Task's workflow.

Selected Wave goals are supplied once as complete `GOAL.md` documents. Repeated
requests for the same document do not repeat its contents; distinct memory files
remain separate even when their text matches. IDE launches with Wave documents
use the assembled prompt so those references reach the provider.

## Connect planning and create work

```bash
lf list wave                       # authored goals, including unconnected Waves
lf account connect linear
lf repo connect --all --team-key EXP # connect goals and choose the Task prefix
lf task create --wave exports --title "Add CSV export"
lf task checkout EXP-12                  # prepare its checkout without execution
lf --task EXP-12 flow start
lf roadmap --json                   # plans and Tasks across Waves
lf wave status exports --json       # one Wave's detailed evidence
```

Connected planning needs a Linear login and repository Team. Connection names
missing access and the command to obtain it. Authored Waves remain discoverable
before connection. Planning setup is separate from direct local execution.

## Inspect and continue

```bash
lf monitor ps                              # one live process snapshot
lf monitor top                             # refresh on a terminal
lf monitor list --json                 # bounded Exec history with a next cursor
lf monitor active --watch --json        # NDJSON until stdin closes
lf monitor show SESSION --final         # provider conclusion
lf monitor usage --days 30                  # measured consumption and missing evidence
lf monitor usage --task LOO-265 --context   # each step's input by source, flagged over budget
lf monitor show SESSION --context       # one step: instructions, memory, scratch, goal, steers, carried, tools
lf session ensure                   # this repository's one ongoing conversation
lf session ensure -w growth         # a Wave's one ongoing conversation
lf session connect SESSION         # continue a conversation
lf session replace SESSION         # fresh conversation for the same scope
lf session complete SESSION         # return review feedback
lf context --task EXP-12 --json     # effective context limits, sources and usage
```

The repository and each Wave have one ongoing conversation. `session ensure`
finds it or starts it, a Wave's with its goal and memory, and repeats return the
same Session. `replace`
stops that conversation, keeps it as history, and starts a fresh one.

Monitor keeps live processes, recorded outcomes and missing observations distinct.
Its overview explains each item's state and next action. A mechanical Exec has
no provider conclusion. JSON reads emit one document; the active watch emits
newline-delimited snapshots. Progress and errors go to stderr.

`lf context` previews local launch input; [configure context budgets](config.md#context-budgets)
in existing personal, repository or Wave settings.

## Publish and finish

```bash
lf pr publish                         # push a ready PR
lf pr submit                          # prepare for a reviewer's merge click
lf pr arm                             # prepare and request auto-merge; return
lf pr land                            # record delivery and return
lf --task EXP-12 pr land -c            # also request completion after merge
lf sync --plan                     # preview integration with main or stack parent
lf wt create csv-export
lf repo release check                   # inspect release eligibility
```

Choose one delivery operation for the desired endpoint. Submit, arm, and land
own preparation and integration; publish does not sync. PR operations work
on ordinary branches without creating a Task. Bare `land` keeps a Task open. Arm and land return after recording delivery;
`lf pr reconcile` checks it once and settles verified merges; `lf repo ci watch`
starts a ci-fix when a recorded landing fails its required checks.

## Keep Tasks progressing in the background

```bash
lf wave cron sync --repo                 # install this Home's minute check
lf --task EXP-12 flow start                  # new launches enroll automatically
lf task automate EXP-12 off         # hold future automatic work
lf task automate EXP-12 on          # enroll or clear its retry hold
lf task automation --json           # schedule coverage and Task blockers
lf task reconcile --json            # run one check now
lf wave cron sync --repo --disable       # remove the schedule
```

Checks continue with Desktop closed while the placed Home's user is logged in.
They resume captured Flows, respect reviews and holds, record CI failures, and
settle verified merges. They do not repair CI.

## Repair failed CI

```bash
lf repo ci watch               # watch this repository's PR checks until you stop it
lf repo ci watch --once        # check every open PR once and exit
lf repo ci watch --install     # keep it running as a launchd service
lf repo ci watch --uninstall
lf repo ci watch --status      # live or not, last poll, what it started and why
```

The watcher polls GitHub about once a minute and starts one ci-fix when a PR
with a recorded landing (`lf pr arm`, `lf pr land`) fails its required checks. A
failing PR nobody armed is reported, not repaired; so is a PR with no Task.
Loopflow Desktop runs the same command for each open repository and stops it
on quit. A second copy stands by behind a live one. Nothing depends on the
watcher running: without it, failures are recorded and wait.
An idle check launches no provider. Existing work and GitHub merge requests
continue when the schedule is disabled or a Task is held.

Set repository defaults in `.lf/config.yaml`:

```yaml
automation: {enroll_new_tasks: true, retries: 1, timeout_reruns: 1}
```

Historical Tasks stay unenrolled until selected. One unchanged launch/runtime
failure may retry; a completed blocked repair waits for changed evidence.
Pending or missing CI checks allow one timeout diagnosis/rerun after 30 minutes.
Use an always-available Home for progress through laptop logout or shutdown.

## Accounts and children

```bash
lf account connect codex work@example.com --chrome-profile Work
lf --account codex=work@ run code
lf --only-account codex=work@ run code
lf account route set codex work@ personal@
```

A preference permits fallback; a restriction limits spending. Saved Flows capture
one selection per provider and carry it through background children. Remote
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

Use literal paths: `lf pr land`, `lf monitor ps`, and `lf monitor list`.
Owners and command names cannot be abbreviated. Exact commands take precedence
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
Press ⌘K in the desktop to search Waves, Tasks, Sessions and Flow templates.
Task destinations open details; selecting a Flow opens its folded template.

### Inspect all work in a Task

```bash
lf task status LOO-358 --json
lf session list --task LOO-358 --history
```

Task status lists Sessions, Flows and Execs from the checkout and explicit binds,
including headless and completed work. Managed marks the Flow advanced by
`lf --task … flow start`; the managed execution line describes only that worker. Independent
work remains visible and preserves the checkout while unfinished or unresolved.

## Keep a document workspace

```bash
lf session ensure                    # persistent repository conversation
lf session ensure --wave product     # separate persistent Wave conversation
```

Inside the conversation's resident checkout:

```bash
lf commit -m "Record accepted decisions" wave/product/MEMORY.md
lf pr publish
```

`lf wt create planning --resident` also creates or reuses an independent document
workspace; enter its printed path before editing.

Primary conversations reuse their respective worktrees, including after replacement.
They display a workspace without gaining Task membership. Moved checkouts are
rediscovered; missing checkouts recover committed branch state. Live conversations
keep their placement until an idle driver boundary.

Resident workspaces retain scratch locally through commit, sync, publication and
landing. Selected-path commits preserve unrelated staged edits. Publication pushes
the committed range, leaving later local edits alone. Inspect the complete range
before publishing. Ordinary Task delivery retains its existing cleanup policy.

If a sync resolver creates a file where an untracked file was stashed, sync restores
the original and keeps the resolver's file beside it as `<name>.lf-sync-1` (or the
next unused number). Read both notes and reconcile them locally. Tracked conflicts
retain the recovery stash and report its identity; resolve them before restoring it.

Run `lf sync --plan` and `lf sync` at a deliberate maintenance boundary and after a
merged document PR. Network failures leave local work usable; conflicts stay visible
and use `lf sync --continue` or `lf sync --abort`. Resident branches remain reusable
and survive automatic pruning. Memory updates do not require PRs or a schedule.
