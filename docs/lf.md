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
lf checkout EXP-12                  # prepare its checkout without execution
lf --task EXP-12 flow start
lf roadmap --json                   # plans and Tasks across Waves
lf wave status exports --json       # one Wave's detailed evidence
```

Connected planning needs a Linear login and repository Team. Connection names
missing access and the command to obtain it. Authored Waves remain discoverable
before connection. Planning setup is separate from direct local execution.

## Inspect and continue

```bash
lf ps                              # one live process snapshot
lf top                             # refresh on a terminal
lf mon list --json                 # bounded Exec history with a next cursor
lf mon active --watch --json        # NDJSON until stdin closes
lf mon show SESSION --final         # provider conclusion
lf usage --days 30                  # measured consumption and missing evidence
lf session connect SESSION         # continue a conversation
lf session complete SESSION         # return review feedback
lf context --task EXP-12 --json     # effective context limits, sources and usage
```

Monitor keeps live processes, recorded outcomes and missing observations distinct.
Its overview explains each item's state and next action. A mechanical Exec has
no provider conclusion. JSON reads emit one document; the active watch emits
newline-delimited snapshots. Progress and errors go to stderr.

`lf context` previews local launch input; [configure context budgets](config.md#context-budgets)
in existing personal, repository or Wave settings.

## Publish and finish

```bash
lf pr publish                         # push a ready PR
lf submit                          # prepare for a reviewer's merge click
lf arm                             # prepare and request auto-merge; return
lf land                            # watch CI, repair failures, and finish merged
lf --task EXP-12 land -c            # also request completion after merge
lf task sync --plan                     # preview integration with main or stack parent
lf wt create csv-export
lf release check                   # inspect release eligibility
```

PR operations work on ordinary branches. Publication does not integrate main.
Arm returns after the request. Land watches GitHub, repairs failed CI through
ci-fix, and re-arms until merged or blocked. Task completion requires observed
merged evidence. LOO-332 will make Land finite once repository ticks own this
watch-and-repair lifecycle.

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

Omit owners when the remaining command resolves uniquely: `lf land`, `lf ps`,
`lf top`, and `lf mon list`. `mon` is a unique prefix, not an alias. Ambiguity
lists canonical choices and performs no action. Exact commands take precedence
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
