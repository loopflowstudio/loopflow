# lf CLI

## Run a workflow

```bash
lf run feature "Add CSV export"     # design, review, implement, check, and merge
lf run code                         # implement and simplify the current plan locally
lf run incident "Export times out"  # restore the workflow, investigate, plan prevention
lf run ship                         # verify, merge, and complete the Task
```

Flows combine agent work, deterministic operations, and review points.
`feature` and `ship` include delivery; `code` leaves local changes.
Use `lf help feature` to inspect a flow before running it.

## Use one Home

Ordinary commands, Task workers and agent tools use the installed `lf` and
`~/.lf`. Source builds forward there too. For an explicit disposable experiment:

```bash
LF_HOME="$(mktemp -d)" target/debug/lf wave list --json
```

Children stay in that Home. Experiments start empty; Loopflow does not upgrade,
repair or preserve them. Use a fresh directory when its schema changes.

## Start and continue Tasks

```bash
lf task create --run --wave exports --title "Add CSV export"
lf task run EXP-12                   # start or continue the saved workflow
lf task run EXP-13 --stack-on EXP-12 # start dependent work on the parent's PR
lf task status EXP-12                # progress, blockers, and delivery state
lf task wait EXP-12 --until terminal --timeout 15m
```

Task execution prepares its worktree and selects one managed FlowSession.
Its steps use the same execution path as a direct Flow in that checkout.
Repeating `run` continues the Task or reports its active worker. Connected
Task creation requires the Wave's planning service; direct workflows do not.

## Direct work and review results

```bash
lf task comment EXP-12 "Keep this version to CSV; leave PDF out"
lf task interrupt EXP-12             # end the current provider turn
lf task run EXP-12                   # continue with retained direction

lf session list                     # find open conversations and reviews
lf session open SESSION_ID           # join or resume one
lf session complete SESSION_ID       # return feedback to its caller
```

Comments reach the Task's advancing worker; idle Tasks retain them without
starting execution. Completing a review returns feedback. The following flow
step decides whether to advance or revise; closing the terminal does neither.

## Inspect work

```bash
lf wave status exports              # progress toward a Wave's goals
lf activity --task EXP-12           # recorded changes to the work
lf ps                               # snapshot of live processes and call trees
lf top                              # continuously refresh the live view
lf runs INPUT --final                # the recorded conclusion of one captured input
lf usage --days 30                  # reported consumption
lf context --task EXP-12 --json     # effective context limits, sources and usage
```

Inspection reads this Home. Live processes, recorded outcomes, and missing
observations remain distinct. `lf runs` reads AgentSession history; `lf exec list`
reads actual lf processes and their command results.
`lf context` previews local launch input; [configure context budgets](config.md#context-budgets)
in existing personal, repository or Wave settings.

Read Wave state as JSON with the supported inspection commands:

```bash
lf wave list --json                 # durable Waves and their runtime evidence
lf wave status <wave> --json         # one Wave's Tasks, Sessions, and conditions
lf roadmap --json                   # current plans across Waves
```

## Publish and finish

```bash
lf task pr EXP-12 publish           # push and create/update a ready PR
lf task pr EXP-12 submit            # prepare for a reviewer's merge click
lf task pr EXP-12 arm               # prepare, request auto-merge, and return
lf task pr EXP-12 land              # prepare, watch, repair CI, and finish merged
lf task pr EXP-12 land -c           # also complete the owning Task
lf release run patch                # verify, prepare notes, tag, and publish a release
```

Choose one delivery operation for the desired endpoint. Submit, arm, and land
own preparation and integration; publish does not sync. PR operations work
on ordinary branches without creating a Task. Bare `land` keeps a Task open.

## Accounts and access

```bash
lf auth status                      # cached credentials and subscription windows
lf auth status codex --verify       # refresh supported account observations
lf auth connect codex work@example.com --chrome-profile Work
lf --account work@ run code          # prefer an account for this launch
lf --only-account codex=work@ run code # restrict this launch and its children
lf auth route set codex work@ personal@ # set account selection order
```

Launches check the access their work requires; explicit account controls
inspect or override selection. Model and account selection are independent. An incompatible explicit choice reports an error.
Background and remote execution require accounts available at their destination.

## Control individual steps

### Skills

```bash
lf skill design "Add CSV export"    # write and review the plan
lf skill debug -c                    # investigate an error from the clipboard
lf : "Include the author column"   # run an inline request
lf -m codex --docs src/export/ run code
```

A skill supplies instructions for one kind of work. `run NAME` selects an
authored flow first, then a skill. `skill NAME` and `flow NAME` select a kind
explicitly. See [Authoring](authoring.md) to define either.

### Checkout and delivery controls

```bash
lf task checkout EXP-12              # prepare a Task's worktree without starting it
lf wt create csv-export             # create a worktree without a tracked Task
lf commit -m "Add CSV export"       # save local changes
lf sync --plan                       # inspect the integration strategy
lf task pr EXP-12 checks --watch     # follow the PR's checks
lf task sync EXP-12 --plan           # inspect this Task's integration
```

These controls operate on the same work as the higher-level workflows.
[The reference](lf-reference.md) covers file access, manual recovery, machine
setup, remote execution, and release operations.

### Flow decisions and recovery

```bash
lf flow resume FLOW_SESSION         # continue a direct flow at its saved position
lf task run EXP-12                   # continue a Task's saved flow
lf task restart EXP-12 --flow feature # deliberately replace its workflow
```

Invoking a flow by name starts a new FlowSession. Resuming retains the captured
definition and feedback. See [decision and retry controls](lf-reference.md#flow-decisions-and-recovery).

## Discover commands

```bash
lf help                             # overview
lf help pr land                     # arguments and effects of one operation
lf help feature                     # inspect a workflow without starting it
lf list                             # commands, skills, and flows
lf help --all                       # full command tree
```

Use `lf pr land` in the checkout or `lf task pr EXP-12 land` from elsewhere.
Omit unambiguous owners: `lf land` resolves to `lf pr land`. Multiple
matches list the canonical choices and execute nothing. Exact commands win;
installed skills do not change command resolution. Use `lf run land` to
select an authored definition instead.

Help accepts either `lf help PATH` or `lf PATH --help`. Help and list do not
launch agents, connect accounts, or fetch definitions. Use canonical paths
and supported `--json` output in scripts.

[Full command reference](lf-reference.md) · [Authoring](authoring.md) ·
[Configuration](config.md)

### Open a Task in the desktop

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
`lf task run`; the managed execution line describes only that worker. Independent
work remains visible and preserves the checkout while unfinished or unresolved.
