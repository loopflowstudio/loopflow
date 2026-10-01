# lf command reference

For examples ordered by workflow, see [lf CLI](lf.md). This page lists
individual controls, arguments, and effects.

```bash
lf debug -c                         # investigate an error from the clipboard
lf run feature                      # run a named flow or skill
lf : "fix the broken link"           # run an inline prompt
lf list                             # discover commands, skills, and flows
lf help pr                          # inspect pull-request operations
```

Run a skill or flow by name. Use the command tree to manage work, inspect
execution, and operate the machine.

## Find a command

```bash
lf help                             # brief overview
lf help --all                       # the complete command tree
lf help task                        # concrete work and delivery
lf help pr land                     # explain one operation
lf pr land --help                   # the same page
lf help debug                       # explain a definition without running it
lf debug --help                     # the same definition page
lf run debug --help                  # the same definition page

lf list                             # commands, skills, and flows
lf list --json                      # stable, structured entries
lf skill list                       # skills and namespaces
lf skill list team                  # skills inside a namespace
lf flow list                        # authored flows and their steps
```

Help shows the canonical command path, arguments, and effects. Definition help
shows its kind and source. Help and list never start an agent, connect an
account, or fetch a missing skill. An uncached external skill is identified as
uncached, with the invocation that would fetch it.

Lists keep skills and flows separate even when they share a name. JSON uses
stable ordering rather than usage rankings.

| Command | What it does |
|---|---|
| `run`, `skill`, `flow` | Execute and inspect definitions |
| `:` | Run an inline prompt |
| `list`, `help` | Discover names and explain commands |
| `task` | Create, inspect, and continue concrete work |
| `wave`, `repo` | Connect planning and replace the shared chapter |
| `session`, `ask` | Open conversations and return review feedback |
| `runs`, `exec`, `activity`, `usage` | Inspect conversation, process, and Work history |
| `ps`, `top`, `prune`, `doctor` | Inspect processes and diagnose the Home |
| `auth` | Connect accounts and configure routing |
| `wt`, `commit`, `sync`, `pr` | Manage checkouts and deliver changes |
| `home`, `ssh`, `install` | Inspect placement, reach another Home, and install releases |
| `release`, `ci`, `cron` | Release software, inspect CI, and schedule work |

## Address delivery by checkout or Task

```bash
lf pr land                          # operate in this checkout
lf task pr DES-123 land             # select the Task's checkout by issue ID
lf task sync DES-123 --plan         # inspect integration for that Task
```

Task actions reuse the PR and sync commands, including their flags. Use
`lf help pr land` for landing options or `lf task status DES-123` for the
Task's outcome and execution state.

An exact command wins first. Otherwise, Loopflow searches beneath the owner
already entered. One match expands to its full path. Multiple matches print
the canonical choices and execute nothing. Use the full path when a name is
ambiguous, and use canonical paths in scripts.

### Commands take precedence over definitions

```bash
lf land                             # the built-in PR operation
lf run land                         # an authored flow or skill named land, if installed
lf skill pr-land                    # the bundled landing skill
```

Installed skills do not participate in command-shortcut matching. Adding a
skill named `land` does not change `lf land`. A name matching several commands
still reports those choices, even when a same-named skill exists.

Only a name with no command matches falls through to definition lookup.
`lf run NAME` explicitly enters definition lookup. Within that lookup, a
flow takes precedence over a same-named skill.

## Run a skill or flow

```bash
lf debug -c                         # shorthand when no command matches debug
lf run debug -c                     # explicitly select a definition
lf skill debug -c                   # explicitly select the skill
lf flow feature                     # select a flow
lf run team/review "Check the API"
lf : "Explain why this test fails"
```

When both kinds exist, an authored flow wins. Adding a flow with the same
name intentionally changes what an untyped definition reference runs. Select
the skill explicitly to keep running its prompt directly:

```bash
lf run release-run                  # repository flow, when present
lf skill release-run                # the skill with the same name
lf help release-run                 # selected definition and the other typed choice
```

Names use `/` for namespaces and `-` within words: `wave/operate`,
`review-design`, `team/review`.

Command words remain available as definition names. Use the explicit kind;
if the name is also a verb within that family, put `--` before it:

```bash
lf skill commit                     # a skill named commit
lf flow -- list                     # a flow named list
lf skill -- show                    # a skill named show
```

### Skills

```bash
lf skill show debug
lf skill debug "The request fails after reconnecting"
lf skill implement: add caching
lf run npx/vercel-labs/deep-research
```

A skill supplies a prompt with assembled context. `{args}` expands to the
arguments after `:`. Execution can fetch an external `npx/` skill; inspection
uses only local definitions and cached content.

Within the skill kind, repository definitions override personal and bundled
definitions. Lookup checks:

1. `.lf/skills/` and `.claude/commands/` in the repository.
2. `~/.lf/skills/` and `~/.claude/commands/`.
3. Bundled skills, including a unique short name within a namespace.
4. Available external skill sources and cached Agent Skills.

Use `lf skill list` for the current catalog. See [Authoring](authoring.md)
for skill files and [Configuration](config.md) for defaults.

### Flows

```bash
lf flow show feature
lf flow validate feature
lf run code                         # implement and simplify
lf run queue                        # prepare and verify without publishing
lf run pursue                       # iterate, publish, and review the demo
lf run feature                      # design review through delivery
```

A flow composes skills and operations. Repository flows live in `.lf/flows/`.
Inspect the definition before choosing its endpoint; starting an agent is not
the same as asking it to publish or merge.

| Flow | What it does |
|---|---|
| `code` | Implement and simplify locally |
| `task-design` | Shape the implementation plan and review the design |
| `refresh` | Sync, then reconcile the design and implementation |
| `queue` | Simplify, refresh, and verify without PR publication |
| `pursue` | Iterate on implementation, publish, and review a demo |
| `feature` | Review the design, pursue the work, verify, land, and complete |
| `ship` | Verify, then land and complete |
| `ship-demo` | Verify, review a demo, then land and complete |
| `deploy` | Verify and land while keeping the Task open |
| `incident` | Restore the workflow, investigate causes, and select prevention |
| `vsm-operate` | Work through delivery, coordination, capacity, adaptation, and identity |

Flow-oriented callers that accept skills use the same flow-first selection.
A skill can serve as a one-step flow conceptually; direct skill execution is
used when it preserves the behavior that caller needs. This does not create
another authored flow or another catalog entry.

Bare YAML names and `flow: NAME` accept either kind with flow first.
`step: NAME` selects a skill explicitly, including when a same-named flow
exists. Use `step:` for a flow that calls its own same-named skill; an untyped
self-reference is a cycle. XOR `skill:`, `steps:`, and `router:` slots select
skills; XOR `flow:` accepts either kind.

### Context and launch options

```bash
lf --task DES-123 run research "Map the runtime behavior"
lf --wave designer skill wave/operate "Review the chapter evidence"
lf --as wave:designer : "Which outcome needs attention?"
lf -m codex run implement --docs src/api/ -c
lf skill design --tui
```

`--task`, `--wave`, and `--as` select the work a direct launch concerns. They
do not advance a Task's managed Flow. A direct flow has its own FlowSession.
Attribution resolves this command's `--as`, then the Task owning its checkout,
then an ancestor's explicit declaration carried by `LF_AS`. Without any of these,
the launch stays unbound. A child command in Task Y's checkout belongs to Y even
when its ancestor declared X; its Exec still retains that causal parent.
`lf task run Y` supplies the same explicit declaration as `--as task:Y` and
selects Y's managed Flow. Claims and Session ownership never infer Task identity.

| Option | What it does |
|---|---|
| `--task ISSUE` | Bind a direct launch to an existing Task and its checkout |
| `-w, --wave NAME` | Bind to a Wave, or qualify a selected Task |
| `--as WORK` | Select Work explicitly, such as `wave:designer` |
| `-m, --model MODEL` | Select a provider/model independently of the account |
| `--docs PATH[,PATH...]` | Add files, directories, or globs to context |
| `-c, --clipboard` | Include clipboard content |
| `--diff` / `--no-diff` | Include or omit the raw Git diff |
| `--diff-files` / `--no-diff-files` | Include or omit changed files |
| `--no-loopflow` | Omit Loopflow's operating guidance |
| `-i, --interactive` | Run interactively |
| `-b, --batch` | Run headlessly |
| `--max-turns N` | Bound provider turns for this launch |
| `--tui` / `--ide` | Select terminal or supported provider-app handoff |
| `--chrome` / `--no-chrome` | Enable or disable browser automation |

Repository agent guidance, recursive scratch notes, and selected work context
are assembled automatically. Direct launches leave edits for an explicit
commit. Several AgentSessions may concern the same Task; attribution does not grant
exclusive ownership of its files.

Selected Wave goals are supplied once as complete `GOAL.md` documents. Repeated
requests for the same document do not repeat its contents; distinct memory files
remain separate even when their text matches. IDE launches with Wave documents
use the assembled prompt so those references reach the provider.

Launch assembly allows 8,000 tokens of Wave memory, 16,000 across scratch notes,
and 16,000 for the launch goal/message. Oversized sources become marked excerpts
with their full local paths; oversized messages are preserved under
`.lf/tmp/context/`. Read relevant omitted sections before acting. The complete
assembled input must fit 64,000 cl100k tokens and 512 KiB, otherwise launch reports
which explicit sources to reduce before contacting the provider. Native provider
instructions, tools, later file reads and conversation history are outside this
launch budget. Run context evidence records reductions and original sizes.

Put global options before the command. Command-local options retain their
meaning after shorthand expansion: `lf commit -m "Fix startup"` supplies a
commit message, not a model. `--` ends option interpretation where literal
arguments are accepted.

### Flow decisions and recovery

```sh
lf task run DES-123 --flow feature  # create a Task-owned invocation
lf task run DES-123             # drive its captured graph
lf task run DES-123 --reason "provider credentials repaired"
lf task restart DES-123 --flow incident
```

Use `lf flow resume FLOW_SESSION --retry` or `lf task run DES-123 --retry`
after a command and its native engine both stop before recording completion.
Retry requires confirmed engine exit and keeps the earlier outcome unknown.
A surviving engine is observed through completion without sending another turn.
Task failure reasons and decision-unblock feedback still apply. Automatic provider
retries retain each turn's history; a failed turn's decision or route is discarded
when its successor is selected.

A **loopflow** is a Flow with one or more backward edges. Each edge names an
earlier node. There is no pass limit. Steps outside a backward edge's body
do not repeat when that edge is taken. Advance enters the remaining steps;
Iterate traverses the declared section again. A slice is a unit of work within
a pass. Task binding adds context and Task authority.

`loop-decide` compares the caller's objective and previous direction with the
available evidence and feedback. It works with any Flow; its selected native turn supplies
one typed result from its final answer:

```json
{"decision":"iterate","summary":"Remaining work, next action, and proof to collect"}
```

The captured step declares `advance` or `iterate` with a nonempty `summary`,
or `blocked` with a nonempty `reason`.
The provider receives that schema before generation. The Flow consumes only the
selected successful turn's validated result. Invalid output gets at most two
corrective turns in the same conversation; exhaustion retains all evidence and
stops the boundary. To request feedback, end the turn with:

```json
{"decision":"blocked","reason":"What stalled, what was tried, and which question needs an answer"}
```

The answer starts another turn in the same conversation for reassessment.

Implement builds the intended behavior and updates the working plan with what
remains. When replacing a path, move its consumer and delete the predecessor.
Compress simplifies code related to the change. Refresh syncs, then runs
realign. Realign edits the plan, corrects clear code mismatches, and reads and
updates the identified Wave's memory using what the work has taught us. Before
curating a parent, it discovers immediate-child `MEMORY.md` files with filesystem
tools, reads relevant sections, and promotes shared lessons while keeping local
detail in the child. Unread coverage stays explicit. Accepted
requirements and unresolved evidence stay visible; no per-pass report or
replacement count is required. Loop-decide reads the current work and evidence
to decide whether to continue. Pursue publishes after convergence, before demo.

Concept-review is interactive, on request or inside unblock, and does not own
navigation. Blocked ends the deciding turn and requests feedback. Meaningful learning
counts as progress; repeating a failure without new evidence calls for help.

A blocked result keys one Ask to its exact captured event and Flow position;
recovery reuses that question and its saved completion. Its Session
runs `unblock`, using concept-review with the user by default or addressing a
specific missing input. Human Complete returns the summary and shared artifact
changes to loop-decide for reassessment. It supplies evidence, not a navigation decision.
While a live Task decision waits, CLI status and the desktop show Blocked with
its selected conversation and unblock Session. Complete that Session to return feedback to the
same caller; no Task resume is needed. Completed or earlier-boundary Asks do
not keep that caller Blocked. If the blocker remains unresolved, report it
instead of opening identical Asks.
An alive Task body shows Stalled after five minutes without an execution event or
sampled CPU progress in the body and its tool descendants. The Task worker
samples every 15 seconds; missing samples, a changed body identity, and
observations older than 45 seconds stay Unknown. Status names the selected execution and the
same recovery in CLI and desktop: interrupt the Task, then resume it. A live
unblock Session still shows Blocked. Observation never authorizes termination.

If a Task decision finishes without a valid verdict, its saved Flow becomes
Blocked, retains its history and failure reason, and opens the same keyed unblock
Session. CLI status and the desktop use that shared projection. After completing
the Session, run `lf task run TASK`; its feedback seeds reassessment at the
failed decision, without choosing Advance or Iterate. Retrying Session launch
reuses its record, including after a launcher failure.
A candidate decision takes effect only after its selected native turn succeeds. Inspect any
operation's effects before choosing `--retry`, since an interrupted operation
may already have changed external state.

An invocation keeps its definition, cursor and return counts
in the current Home's SQLite store. Continuation uses those captured facts even
if source definitions disappear. Completion grants no implicit merge or
Task-completion authority. Before launch the Task page shows its Flow template;
after starting it shows the compiled graph.

The graph JSON field `sources` lists each step's source definitions, outermost
first. Desktop uses it for the `from …` breadcrumb. These labels grant no
execution authority.

Composed templates compile before execution. Taking an Iterate edge moves the
cursor and increments its return counter in the same FlowSession. Retry retains
the pass; resume retains the selected boundary, including a pending review.
Membership names a node and iteration tuple in the
captured graph; an independent conversation about the same Task does not become
a Flow step. Graph keys, current/completed nodes, return edges and Session
membership use captured numeric node IDs, local to that FlowSession. Authored
occurrence names remain labels; nested containment comes from the graph.

A boundary can fail several times before succeeding. AgentSession history keeps
each provider start, result and usage receipt; the Flow consumes only its exact
selected successful completion. A mechanical boundary runs in its own child
`lf` process and records start and outcome in Flow history. Resume waits for a
surviving step and consumes its result once. Task restart stops the selected step
even when its driver has exited or released its claim. The interrupted Exec is
recorded separately from an unresolved mechanical effect. An earlier success, helper completion or stale writer cannot
advance the current selection. Missing capture data is reported without replacing
it with today's catalog; missing external-effect evidence requires inspection.

A `human:true` step opens an interactive AgentSession at the FlowSession's
selected boundary. The reviewer saves feedback and revised artifacts, then calls `lf session ready "feedback and remaining work"`.
The reviewer ends the conversation with `lf session complete <session-id>`.
Completion returns feedback to the next step. A following loop-decide interprets
it and records Advance or Iterate against its own authored edge. Human reviews
have no implicit revision target. Closing the provider, marking ready, or
reopening a Session does not complete it.
Already finished invocations report completion rather than starting again.

Invocations capture all XOR routers and branch definitions at creation. A router returns `{"path":"NAME"}` constrained to its captured paths;
failed native turns cannot donate that candidate to a retry. Nested paths and review completion use the
same cursor transition. Recovery fixtures prove these paths locally; live
provider/desktop review completion → decision → implementation → Ask → reassessment remains a
separate demonstration.

The `advance` skill also resolves these actions from an ordinary coding
conversation: complete an exact review, resume a Task, or prepare work from an
approved design. It reports the actual Task state after the handoff.

### Saved FlowSessions

```bash
lf flow list --sessions --json --limit 100
lf flow list --sessions --for-task INF-123 --managed true --json
lf flow show --sessions --json FLOW_SESSION
```

List saved progress without starting work. `--after` accepts the previous page's
`next` ID with the same filters. `--all` includes other repositories and unknown
historical repository evidence. `--state` selects current, completed or replaced
records; `--search` matches a literal name or identity. Detail reads the captured
graph even if its template or checkout is gone. Loop passes share that graph and
FlowSession; their node and iteration positions distinguish their history.

`lf flow list --json` and `lf flow show TEMPLATE` still inspect reusable templates.


## Task: concrete work and delivery

```bash
lf task status                      # Task bound to this checkout
lf pr status                        # current branch's PR, with or without a Task
```

Task commands manage concrete work. Commit, sync, worktree, and PR operations
also work in an ordinary checkout. They do not create an issue merely to satisfy
the command path.

Use `lf roadmap --json` to inspect Tasks across current Wave plans and
`lf task status ISSUE` for one retained Task, including past planning ownership.

### Create and advance a Task

```bash
lf task create --wave designer --title "Repair keyboard navigation" \
  --notes "Tab order should follow the visible controls"
lf task checkout DES-123             # prepare its worktree without launching a worker
lf task run DES-123                  # start or continue the saved workflow
lf task run DES-124 --stack-on DES-123
lf task create --run --wave designer --title "Repair keyboard navigation"
```

Connected Task creation files the issue and refreshes local planning. Checkout
preserves the Task's identity and prepares its worktree. Repeating checkout
reuses that placement; repeating run continues the saved workflow or reports
the active worker.

`--stack-on` gives a dependent Task its own worktree and PR, based on the
parent Task's published PR. Its PR initially targets the parent branch and
can move to the default branch after the parent merges.

### Change direction and recover

```bash
lf task edit DES-123 --title "Repair dialog keyboard navigation"
lf task comment DES-123 "Keep this change inside the dialog"
lf task comment DES-123 --json       # read the thread
lf task comment DES-123 --steer "Keep the public API" # direction from an agent Run
lf task interrupt DES-123            # interrupt the active provider turn
lf task run DES-123                  # continue after interruption
lf task restart DES-123 --flow feature
lf task run DES-123 --reason "Repository credentials repaired"
lf task wait DES-123 --until terminal --timeout 15m
```

Comments provide direction without starting an idle Task. The advancing worker
reads them; independent Task-bound AgentSessions do not receive a broadcast. Use
restart to replace a saved workflow after the captured worker stops. Use
`run --reason` after repairing a reported execution blocker.

Manage Task state through Task commands and inspect the resulting facts with
task status. Planning, provider liveness, review readiness, PR state, and
completion remain distinct. A missing worker does not prove completion.

```bash
lf task complete DES-123 --summary "Keyboard navigation verified"
lf task delete DES-123
```

Complete planning work directly, or complete a placed Task after its delivery
obligations settle. Delete cancels unfinished placed work and removes its PRs
and branches before trashing the issue. Completed work keeps its successful
outcome and uses completion cleanup. Both retain delivery history. If an operation partially succeeds, its error
identifies the retained state and the command to retry.

### Cancel work and sweep old chapters

```bash
lf task abandon DES-123              # cancel Linear and the Task, close PRs, delete branches
lf task abandon jack/old-feature     # resolve retained Task identity by branch
lf task abandon                     # Task in this checkout
lf task sweep --json                # preview open issues outside current chapters
lf task sweep --apply               # cancel eligible issues; print exclusions and failures
lf pr abandon jack/old-feature      # close this PR and delete its checkout; retain Task outcome
lf wt delete jack/old-feature       # delete checkout and local/remote branches; retain PR/Task
```

Cancellation retains issue, Task, PR and Run history. It refuses completed work
and live or unresolved worker ownership. Interrupt the Task and wait for its
worker to exit first. Pending cancellation prevents new worker claims; retry the
cancellation after a provider failure. A dirty checkout requires explicit `--force`. Retry the
same issue ID or full branch after a partial failure, including when the checkout
or remote branch is already absent. Lower commands state that the Task remains
open; they never equate a closed PR with an abandoned Task.

Sweep reads every linked Wave's Initiative, including archived Projects, and
compares issue membership with that Wave's current chapter. Current-chapter and
terminal issues are excluded. Projects owned entirely by other Teams are skipped
and named once in the preview (`issue: null` in JSON). Wave sync and planning
refresh also omit them; sync never renames them. Missing Team ownership and
Projects shared with the repository Team still require resolution.
Worker claims, open PRs, unreconciled merges,
dirty checkouts and unavailable evidence are reported without canceling those
Tasks. Preview and apply check retained unmerged PRs even after local abandonment.
Apply rechecks ownership and chapter membership, uses Task abandonment,
and reports each outcome. A partial cancellation exits with an error and its
retry command. A preview is not evidence of applied cleanup.

### Lifecycle action inventory

`task pr ISSUE ACTION` selects the Task's checkout and passes the action and
flags to the existing PR command. `task sync ISSUE` does the same for sync.
Neither needs a prior `cd`. These use the selected installation for managed
operations; a source checkout cannot transfer private Task identity implicitly.

| Need | Task action | Composition and owned records |
|---|---|---|
| File | `task create` | Linear issue; local planning snapshot |
| Allocate or recover checkout | `task checkout ISSUE` | Store Task/PR identity + local checkout/branch |
| Start or resume | `task run ISSUE` | Checkout + saved Flow + worker |
| Pause | `task interrupt ISSUE` | Interrupt current provider turn; retain saved cursor |
| Replace workflow | `task restart ISSUE --flow FLOW` | Checkpoint + stop worker + replace Flow + start |
| Hand off direction | `task comment ISSUE --steer TEXT` | Linear direction + durable steer; does not launch work |
| Split dependent work | `task create --run --stack-on ISSUE` | New issue, Task, checkout and PR based on parent |
| Sync with main/parent | `task sync ISSUE` | Existing integration operation in Task checkout |
| Publish | `task pr ISSUE publish` | Commit/push + GitHub ready PR + Task PR linkage |
| Open review | `task pr ISSUE open` | Push + draft PR + browser; ready PR stays ready |
| Submit | `task pr ISSUE submit` | Prepare + user merge request; no automatic merge |
| Arm | `task pr ISSUE arm` | Prepare + head-specific auto-merge request |
| Land | `task pr ISSUE land` | Arm + watch/repair + record authoritative merge; keep Task open |
| Land and complete | `task pr ISSUE land -c` | Land + store Done + Linear completed + cleanup after worker settlement |
| Record success | `task complete ISSUE --summary TEXT` | Complete settled delivery + cleanup; retry incomplete cleanup by issue ID |
| Continue serial delivery | `task pr ISSUE next [SLUG]` | Retain prior PR + rotate to next branch, carry follow-up |
| Cancel | `task abandon ISSUE` | Linear canceled + store abandoned + PR abandonment + checkout deletion |
| Delete issue | `task delete ISSUE` | Cancel unfinished placed work or clean completed delivery, then Linear trash; retain history |
| Recover interrupted execution | `task run ISSUE --reason TEXT` | Retry saved boundary after correcting the blocker |
| Delete checkout | `wt delete BRANCH` | Remote branch + local checkout/branch; retain PR and Task outcomes |

`pr abandon BRANCH` owns closing GitHub and settling its Task PR record, then
uses the same checkout deletion as `wt delete`. `wt remove` and `wt rm` are
removed. Prune remains a separate selection policy for eligible clean checkouts.
`pr land` removes a merged standalone PR's checkout and branches. Task landing
keeps the Task open and explicitly retains its checkout for the saved Flow or
next PR; `-c` also completes the Task and cleans up. A running Task worker keeps
its checkout until its provider stops and its exact claim settles. Cleanup never
forces dirty files or branch tips beyond the merged head. The primary checkout
is retained and reported.

`task complete ISSUE --summary "Retry cleanup"` retries partial cleanup without
reopening the outcome or duplicating completion. Empty successors retain their
branch identity as abandoned PR history so cleanup can retry after a crash.
`task delete ISSUE` composes cancellation for unfinished placed Tasks, cleanup
for completed Tasks, and issue trash. A live or unresolved worker blocks deletion.
Planning-only deletion allocates no checkout. Provider trash confirmation and
Task/PR/Run history survive retries.

Recovery restores missing placement (`task checkout ISSUE`), resumes saved
execution (`task run ISSUE --reason TEXT`), or retries the failed lifecycle
command. It does not reopen a terminal outcome or undelete a Linear issue.

| Lower layer | Actions | Owned effects |
|---|---|---|
| PR | `publish`, `open`, `submit`, `arm`, `land`, `abandon`, `next` | GitHub lifecycle and retained Task PR record; land/abandon use checkout deletion |
| Worktree | `wt create`, `wt delete` | Local checkout/branch; delete also removes the remote branch |
| Integration | `sync`, `task sync ISSUE` | Integrate main or the recorded stack parent; preserve Task and PR outcomes |

### Read and edit another Task's files

```bash
lf task changes DES-123 --base head --json
lf task diff DES-123 src/parser.rs --base parent
lf task file DES-123 src/parser.rs --json
lf task save DES-123 src/parser.rs --revision REVISION --json < draft.rs
```

File reads use recorded placement without starting an agent. Comparisons default
to the recorded PR base (`parent`); use `head` for the current commit or a
returned base SHA for a pinned comparison. `diff --draft` compares stdin
without writing it.

Save requires the revision returned by file inspection. A detected conflict
leaves the current file intact. Submitted drafts and displaced versions are
retained for recovery; `task file --recoveries` includes that history. Text
operations require UTF-8 files within 1 MB and exclude symlinks and Git metadata.

### Worktrees

```bash
lf wt create parser
lf wt create parser --plan
lf wt switch parser
lf wt list
lf wt prune --dry-run
lf wt delete parser
```

Create a worktree for ordinary local work, or use `task checkout ISSUE` for a
tracked Task. Creation refreshes the default branch before choosing its base;
`--plan` previews local placement without fetching or writing.

Prune removes eligible clean, settled or stale worktrees. It preserves dirty
work, the current checkout, the default branch, nonterminal Tasks, and live
owned work. Inspect `--dry-run` before cleanup. Explicit forced removal is a
separate destructive choice.

### Commit and sync

```bash
lf commit -m "Fix keyboard navigation in dialogs"
lf commit --no-add                  # commit only the existing index
lf sync --plan                      # inspect the integration strategy
lf sync                             # integrate and publish the branch with a lease
```

Commit stages changes and generates a message unless given explicit options.
Publishing is a separate PR operation.

`lf sync` replaces `lf rebase`. Saved Flow command records migrate the old name
on decode while preserving captured arguments and structure. It merges main into
the current branch, or the live parent into a stacked child, then publishes with
a lease. Original commits and reviewed merge resolutions stay in the branch.
Unstacked empty and scratch-only branches reset to their target while preserving scratch.
Stacked children retain their initial `Clear inherited scratch` commit and merge
updates while keeping their own scratch, including inherited files the child deleted.
CLI and Flow sync use the same Task parent. If the child already contains its
parent's head, sync leaves its history and remote branch unchanged.
Main retains unpublished commits and edits and is never pushed. `--plan` uses local
evidence without fetching.

After a stack parent lands by squash, synchronization uses the recorded parent
base to compare the child's changes with main. The resulting merge records main
as its parent, preserving the child's edits and original history. PR landing uses
the same integration; GitHub's squash merge adds one commit to main.

```bash
lf sync --manual
lf sync --continue
lf sync --abort
```

Manual recovery stays local and does not push. A retained conflict keeps its
sequencer and recovery context. An interrupted edit restoration reports the
retained stash. Use `--adopt` with `--continue` or `--abort` only when explicitly
taking over an operation started outside Loopflow.

### Pull requests

```bash
lf pr open                          # push a draft and open its page
lf pr publish                       # push and mark ready, without opening a browser
lf pr checks --watch
lf pr submit                        # prepare for a reviewer's merge click
lf pr arm                           # request auto-merge and return
lf pr land                          # watch, repair CI, and finish merged
```

These operations use the selected Task or checkout. An unbound branch uses
ordinary branch and PR behavior; no synthetic Task or Linear connection is
required. An unreadable Task binding reports an error rather than being treated
as an unbound branch.

| Operation | What it does |
|---|---|
| `status` | Read the current PR's state |
| `checks` | Read CI; `--watch` waits and `--logs` includes failure logs |
| `open` | Push, create/update a draft, and open its page; ready PRs stay ready |
| `publish` | Push, create/update a ready PR, and print its URL |
| `submit` | Integrate, prepare, assign, and stop for manual merge |
| `arm` | Prepare the head, request auto-merge, and return |
| `land` | Prepare, request auto-merge, watch, repair CI, and finish merged |
| `next` | Continue a Task on its next PR after an out-of-band merge |
| `abandon` | Close the PR and remove its branch and worktree |

Publish leaves scratch notes and integration history alone. It does not sync.
Submit, arm, and land preserve useful conclusions, clear scratch, prepare a
reviewable commit, and integrate the branch before publication. They preserve
valid reviewer-facing copy and update Task merge consequences.

```bash
lf pr publish --title "Make dialog navigation follow tab order" \
  --body "Keyboard focus now follows the visible controls."
lf task pr DES-123 land -c          # merge, then complete the owning Task
lf pr land --next focus-ring        # merge, then continue the same Task
lf pr next focus-ring               # reconcile a merge performed elsewhere
```

Bare land keeps the Task open. `-c` requests completion after authoritative
merge evidence; `--next` continues its PR chain. Arm records the same intended
outcome, but returns before merge. A published PR alone does not complete work.

On failing required checks, land runs bounded repair for that PR head and
observes the replacement head. Repeating land resumes retained work. It does
not invent another PR when settled work can already complete the Task.

## Wave: goals, memory, and chapters

```bash
lf wave list
lf wave status designer
lf --wave designer : "Review the plan"
lf --wave designer skill wave/operate "Review progress toward keyboard access"
```

A Wave keeps long-term goals and persistent notes in `wave/NAME/GOAL.md` and
`wave/NAME/MEMORY.md`. It is useful before connecting a planning service or
creating Tasks. Wave discovery combines authored files with registered state
and shows whether each Wave is connected.

Edit authored goals and memory as repository files. Task owns concrete work;
Wave owns the direction that work serves.

### Connect planning

```bash
doppler run -- lf auth connect linear
lf wave connect designer --team-key DSG
lf wave sync designer
lf wave status designer --sync
lf wave status designer --no-sync
```

Connection links the Wave to its planning service. A Chapter is the shared name
of every Wave's one In Progress Linear Project. Each Project holds Tasks, KRs,
metric targets and its Flow default. Current navigation is Wave → Task; Completed
Projects retain past plans. There is no separate Chapter record.

Status reads local planning unless asked to refresh. A failed fresh read is
reported rather than presented as current evidence. Connecting a repository's
other Waves reuses its Team; `wave connect --all` discovers authored Waves.

### Replace the chapter

```bash
lf wave update-plan --wave designer --plan plan.json
lf repo new-chapter 2026-10 --dry-run --json
lf repo new-chapter 2026-10 --json
```

Update-plan changes the current Project's KRs, metric targets, and Flow.
New-chapter advances every Wave together and records a boundary receipt. Started,
unfinished Tasks retain their identities, worktrees, PRs, and FlowSessions.
Untouched backlog is abandoned; completed work remains historical. Missing
evidence stays unresolved.

Use the dry run to inspect each Task's disposition. A stale planning label
alone does not erase recorded execution or authored work. Repeat the same
operation to recover an incomplete transition rather than manually rotating
provider Projects.

### Conversations

```bash
lf --wave product : "Which Task should own this change?"
lf --wave product wave/operate "Review the current blockers"
doppler run -- lf discord serve product
```

Ordinary Sessions retain conversations; `wave/operate` makes one finite planning
pass. Memory stays in `wave/<name>/MEMORY.md`. The independent Discord bridge
uses the GOAL.md channel binding, starts a bounded conversation for each new message and
posts its final answer. Its cursor is in memory and startup skips old messages;
see [Discord bridge](waves.md#discord-bridge) for the delivery limits.

### Schedules

```bash
lf cron preflight --wave designer
lf cron sync --wave designer
lf cron list --wave designer --json
lf cron trigger --wave designer --flow vsm-operate --wait
lf cron history --wave designer --days 35
```

Author schedules in the Wave's `GOAL.md`. Preflight checks its placement and
installed target without changing launchd. Sync reconciles the declaration on
the owning Home. Trigger exercises the installed job; history reads its
durable firing receipts.

Use `cron add` or `cron remove` for explicit installed-job changes.
Schedules run through the installed CLI and use flow-first definition lookup.
The Home's installation-update schedule is separate under `install`.

### Placement and retirement

```bash
lf wave place WAVE_ID HOME_ID
lf wave relocate WAVE_ID --name platform
lf wave relocate WAVE_ID --repo ../platform
lf wave retire designer --reason "Responsibility transferred"
lf wave forget WAVE_ID --dry-run
```

Placement selects the Home responsible for execution. It does not launch work
or redirect every local command to that machine. Relocation preserves identity
and authored state; it reports live work that must settle first.

Retirement retains history. Forget removes an empty registration after authored
files have been removed.
`wave rename NAME --title TEXT` changes the connected provider's display title.

## Session: conversations and reviews

These examples use current spellings. The lifecycle contract and remaining
implementation are in [cutover status](architecture-reference.md#cutover-status).

```bash
lf -b implement
lf session list --interactive false --task INF-123 --json
lf session connect SESSION
lf session rename SESSION "Migration review"
lf session bind SESSION --task INF-123
lf session ready "Ready for review"
lf session complete SESSION
```

Every agent conversation has an AgentSession, including headless skills, inline
prompts, helpers, Asks and reviews. Default lists show interactive conversations;
explicit filters expose headless and completed history. `--all` retains its
all-repositories meaning. Interactive mode does not decide Flow membership,
completion or permission to advance a review.

List state `unknown` means there is no observed active client, explicit readiness,
or recorded closure. The conversation remains visible and can be connected;
Connect validates its current owner and native history. Lists retain recorded
Flow membership even when its relative position is unknown. They do not open
captured graphs or transcript/history bodies to fill missing display facts.

For bounded inventory, use `lf session list --page --json --limit 100` and pass
`--after NEXT` until `next` is null. Pages use stable Session IDs, so renaming does
not shift their order. They are separate reads, not a snapshot of concurrent
insertions or filter changes. Desktop merges partial pages with retained records
and removes absent Sessions only after the final successful page. A failed page
keeps the last observations and open terminals.

Use `--interactive all` for both modes in one inventory. In Desktop, the outline
menu's **Show headless Sessions** reveals headless conversations. Hiding them
keeps their open terminals and drafts.

Choose **Bind to Task…** beside a Session name, enter an issue identifier or
stable Task ID, then review the resolved Task before **Bind permanently**.
`lf session bind SESSION --task INF-123 --dry-run --json` resolves that same
identity without assigning work. Confirmation submits its stable Task ID;
existing Wave and Flow constraints are checked by the binding transaction.

Connect uses the live engine where possible and retains conversation identity,
name, feedback and native history. `session open SESSION --try` lets the
provider arbitrate an active native Session. The new driver receives write
authority; the old client can remain a passive display. `--replace` stops Loopflow-owned clients
before connecting here; with a live Codex engine, the active turn and sibling
conversations continue in that engine. `--json --replace` prepares the command
without stopping clients or transferring the driver. There is no separate
Session engine-restart command. `lf flow resume FLOW --retry` recovers a saved
Flow after confirmed engine exit while retaining its native conversation;
`lf task restart TASK` instead replaces the Task's saved workflow.
Unsubmitted editor text requires its own surface-preservation proof.

Session replacement and native client stopping require readable process evidence.
If inspection fails, retry after it is available; the command leaves termination
unconfirmed and preserves native history. Saved Ask or Flow feedback survives a
cleanup failure, which is reported separately from completing the review.
Native client publication and stopping share exact launch exclusion, so stopping waits
for an in-flight launcher to publish its client before inspecting it. A Task
with confirmed deletion cannot start an Ask or resume a native Session. Exact
`runs --active --task ISSUE` still inspects its retained process evidence.

Explicit `--task`, `--wave` or `--as` selects ancestry at launch; a registered
Task checkout takes precedence over inherited `LF_AS` when this command has no
explicit selector. A conversation can remain unbound. CLI states the permanent bind target and writes; Desktop confirms it. Same-target bind is a no-op; there is no reassignment or unbind.
Existing Wave ancestry must agree. Done/landed Tasks remain valid without being
reopened. Flow membership cannot be changed to make an incompatible bind work.

Jack Heart selected prospective attribution: binding affects subsequent work;
prior usage retains its recorded owner. First assignment, including bind, sets
Task Started once without rewriting earlier work or usage.
Inspection commands are still visible in Exec history but do not start Tasks.
Help, version and rejected arguments retain their actual exit code when a
compatible process ledger already exists. These early paths do not initialize
or migrate a Home. Missing storage is reported without changing the command's
result; agent launch still requires durable admission.
Rename and bind retain the Session ID, pane, draft and membership. Human names
survive generated suggestions. A bound Session absent from the visible roadmap
remains bound.

Ready saves feedback without closing the Session. Complete persists the result
before teardown; keyed Ask retries return the same answer. Flow reviews pass
feedback to the following decision, which owns navigation. Closing a pane,
exiting a provider, or marking ready never completes that review implicitly.

Session identity and captured-input selection come from SQLite. Exact Session
IDs select their current input; `lf runs INPUT --json` reads its recorded history
without requiring the payload file. There is no legacy Home import command.

## History and live activity

Captured inputs and provider outcomes belong to AgentSession history. Exec
history records the processes that performed commands.

```bash
lf activity --task INF-123 --json   # durable Work changes, newest first
lf runs                             # recent Home-local conversation history
lf runs --active --watch --json     # stream live Sessions and observation gaps (macOS)
lf runs --active --task INF-123     # exact Task attribution, independent of checkout
lf runs --project parser            # filter before the result cap
lf runs --parent INPUT --json       # captured inputs issued by this input, uncapped
lf runs INPUT --final               # last durable provider conclusion
lf runs INPUT --events              # retained event stream
lf replay INPUT                     # execute the saved request again
lf usage --task INF-123 --json      # recorded provider usage for one Task
lf usage --days 0 --json            # all retained history; default is 30 days
lf ci --since 7d --json             # CI repair attempts, latency and outcomes
```

`lf runs` inspects history. Continue its conversation with
`lf session connect SESSION`, or use `lf replay INPUT` to launch the immutable
recorded request again.

`lf wave list` reads the local Wave registry. `--current` excludes abandoned and retired
registrations. `lf roadmap --all` spans repositories without inheriting the
launching process's Wave; an explicit `--wave` still scopes the query.

```bash
lf wave forget <wave-id> --dry-run --json
lf wave forget <wave-id> --json
```

Forget an abandoned, empty registration after removing its authored
`GOAL.md`. The command leaves repository files alone and refuses registrations
with Projects, Tasks, child Waves, planning snapshots, or metric evidence.
Use installed `lf` to change installed data; development binaries use private
branch data directories.

When a source build inherits the installed data directory from a Session, it
reports and uses `~/.lf-dev/worktrees/<source-identity>` instead. Its first database open
starts from a SQLite snapshot of the installed store, including committed WAL
data. Later invocations keep branch writes; they never copy them back. Explicit
disposable data directories still work. The snapshot does not copy captured payloads
or account files, and does not transfer the launching Session's execution authority.
An explicit private `LF_HOME` also selects the observation and execution destination,
even when the Session supplied installed control paths. Child processes use
that same data directory; re-entering it preserves its own execution context.
`LF_HOME` is the data-directory setting, not a Home placement selector. The
snapshot retains recorded Home IDs and placements; it does not register a new
execution Home or move work to another machine.

```bash
target/debug/lf task run LOO-321       # reports the installed executable and data directory
target/debug/lf task status LOO-321    # reads the independent branch copy
```

When an installation exists, managed Task operations use its CLI and store from
the start: `run`, `create --run`, `restart`, and Task review Session completion.
Preparation, issue creation, checkpoints and worker claims happen there.
The returned Task state comes from the installed database. Non-launching data
commands keep using the branch copy. A Task that exists only in the branch is preserved there;
these commands do not register it in installation. Review completion names the
exact invocation boundary and consumes the installed Session's readiness, without
copying branch feedback. Direct branch workers are refused while an installation
owns execution. With no installation, the source build runs workers against its
own branch data directory; installation is not a prerequisite for development.
Later `target/debug/lf task status` reads still show the private branch copy.
Follow managed execution with the installed executable and data directory reported
by the operation; the two data copies do not synchronize.
A private data copy does not isolate external effects such
as provider issue deletion or shared worktree edits.

`lf wave status` focuses one Wave's local
planning and runtime projection. `lf roadmap` overlays the current
Linear-backed plan without creating a second runtime model. `lf activity`
orders durable Work creation, execution, Task PR, and Steer facts; it reuses
`WorkRef` identity and does not read reconstructable Task or Project wake
events. `lf runs --task` selects retained AgentSession input history from the
Home's SQLite store before decoding its evidence. Each input keeps its original
Work attribution, provider, usage and outcome across conversation continuation.
Command names remain `runs` and `usage`; their JSON describes Session history.

`lf runs --active` retains its command spelling and now returns live AgentSessions.
Rows use stable Session `id`, `title`, current typed `work`, and verified `processes`;
input replacement does not change their identity. The JSON collection is `sessions`,
replacing `runs`. Per-input subjects, harness, model and repository metadata are
removed from this live projection. Historical input/usage commands are unchanged.

Current SQL drivers attribute exact live Exec/process receipts. Native client
receipts resolve through retained input membership, including after driver exit.
Retained provider PID/start evidence keeps earlier engines off a later Session in
one Exec. A driver, endpoint or idle engine alone never proves activity. Ambiguous
shared engines or multiple current Sessions stay explicit gaps. This observation
has no process-control or Flow-settlement authority.

On macOS, `--watch --json` emits bounded newline-delimited snapshots every two
seconds. Each tick rereads SQL ownership, including a database outside Home;
filesystem events only invalidate process receipts. Send `{"action":"refresh"}`
for an immediate read or `{"action":"rescan"}` after sleep/wake. Closing stdin or
stdout exits only this reader. Notification loss requests a full cold rescan;
unchanged warm reads avoid enumerating retained input directories. Linux supports
one-shot reads and reports continuous discovery as unsupported.

JSON requires `discovery` (`scanning`, `ready`, `unavailable`), `home`,
`observed_at`, optional `task`, `sessions`, and `gaps`. Empty `sessions` confirms
no observed active conversations only when discovery is ready and gaps are empty.
Ownership changes during projection yield Scanning; Desktop retains its last good
frame. A replaced Home requires a fresh reader. Use one unfiltered Home observation
for several Task views, matching typed `work`, never a checkout or subject string.

The `--parent` drill resolves one captured input and returns inputs whose
recorded caller is that input, without the seven-day presentation cap. Use
`lf exec list --parent EXEC_ID` for actual process ancestry. `--final` projects
the last durable provider conclusion from Session events; `--events` retains
recorded order. Both reads work without the captured payload directory.
Summary and usage reads exclude conversation text before loading payloads.
Missing provider completion remains explicit and supplies no invented conclusion.
Replay uses the immutable prompt, agent/model, non-secret provider account ID,
and tool boundary recorded before spawn; it never reconstructs those inputs
from current planning or prompt configuration. Managed Claude/Codex replay
resolves that account ID through the current Home's deterministic credential
directory or an explicit forwarded lease; replay uses the same capture and launch
path as any other launch.
None of these commands silently queries or aggregates another Home.

```bash
lf exec list --task LOO-298 --json
lf exec list --all --search 'pr land' --outcome failed --limit 25 --json
lf exec list --all --parent EXEC_ID --json
lf exec show EXEC_ID --json
```

Exec history records actual `lf` processes and observed command results.
`list` defaults to the current repository; `--all` includes every repository.
`--task` and `--wave` select recorded agent or mechanical work, including historical
Tasks. `--caller` selects commands issued by an AgentSession. These are distinct
from the command's own recorded Wave context and causal parent.

JSON returns `entries` and an optional `next` cursor. Pass that object as JSON to
`--after`, retaining the same filters. The default page size is 100; zero is invalid.
Pages sort by descending start time, then ID. Refresh from page one for new data:
continuation does not freeze a snapshot across new records or changing outcomes.

Search matches literal command text across argv elements, ignoring ASCII case;
`%` and `_` are ordinary characters. The original stored command remains in JSON.
Malformed historical command text remains searchable as recorded. Exact lookup
accepts a full ID or an unambiguous prefix. An `unknown` outcome means no terminal
observation, and says nothing about whether the process is alive. Discovery reads
bounded command rows without loading conversation captures or transcripts.

```bash
lf ps --json          # one OS-live process and call-tree snapshot
lf top                # refresh every two seconds on a TTY; emit once when redirected
lf top --json         # emit one snapshot
lf prune --dry-run    # inspect stale receipts and registered orphan process groups
lf prune              # remove those receipts and reap those process groups
```

`lf ps` and `lf top` show OS-live processes only. Exact PID/start-time receipts
attach `lf` processes to call records; exact ancestry attaches provider
processes. Completed calls and launches disappear. Unclaimed providers remain
separate because Loopflow has no exact authority to attach or signal them.
Elapsed time never implies death.

Both commands read process identity from the selected Exec ledger and ownership
registry without migration. They do not replay command journals to reconstruct
Execs or treat a recorded command outcome as current OS liveness. Source builds
retain the same private-data selection as other commands.
`lf prune` is the separate write boundary. It removes dead Exec receipts and
reaps only OpenCode process groups whose registered owner is absent. It never
kills unclaimed provider PIDs; inspect exact targets with `--dry-run` first.

`lf usage` reads the same Home-local Session history as `lf runs`, newest first.
`--days` defaults to 30; zero selects all retained history. Captures use their
event observation time. Native turns without a captured input use
their own first observed receipt, preserving missing capture/start membership.
`--wave`, `--project`, and `--task` select recorded ownership, including native
turns after a bind. Earlier usage keeps its original owner. Receipts without an
original start remain unattributed and appear in unfiltered discovery.

JSON returns `SessionHistory` rows with `session_id`, optional `captured`, retained
`artifact_key`/`caller_artifact_key` selectors, `providers`, and `usage`. A provider
entry references its exact native thread/turn/start/completion, or older recorded
attempt evidence beneath a captured event. `recorded_outcome`/`recorded_at` retain
old recorder results separately from provider outcomes and actual Exec exits.
These history projections have no resumable lifecycle; the owning AgentSession
has that lifecycle. Rust and Swift use the same wire shape.

Cumulative counters are reduced once per stream. Omitted counters stay unknown,
final receipts and evidence gaps remain explicit. Native thread/turn correlation
prevents counting recorder checkpoints twice. Without a retained baseline, usage
reports the observed suffix as partial. A native completion supplies neither
missing usage nor a command outcome.

Recent `lf runs` summaries select their budget before reading history payloads,
retaining every eligible unfinished entry. Exact Task and caller drills have no
presentation cap. A Session selector reads its current captured event. Historical records from
retired owners are not imported; configured-provider acceptance remains separate.

`lf ci` reads durable CI incidents from the local Home store. One failed head is
one attempt; later passing and merge observations close every open attempt on
that PR. `--wave` and `--repo owner/repo` filter the same local report.

`lf doctor` also prints the binary's build provenance, the resolved database
path, and the latest known and applied migrations. Those fields still print
when the database is too new or came from a divergent development build.

The `continuity` check reads installed cron activation and scheduled receipts.
Only the latest due interval for each cron is live: a missing receipt names the
cron, Home, expected interval, and `lf cron history` command. Pre-activation and
older ledger gaps remain visible history without keeping every later doctor
red. A failed target still proves the scheduler fired; its own receipt and
target error remain the actionable evidence.

## Auth: accounts, capacity, and routing

```bash
lf auth status
lf auth status codex --verify
lf auth connect codex work@example.com --chrome-profile Work
lf auth route set codex work@ personal@
lf auth route show
```

Connect an existing provider login,
inspect its credential state and observed capacity, and choose which accounts
a repository may use. Account selection does not change the selected model.
An incompatible explicit account/model combination reports an error.

Status uses cached evidence by default. `--verify` requests fresh supported
observations; `--details --json` includes sources and timestamps. Unknown or
stale capacity is labeled rather than treated as zero or unlimited.

### Connect and configure

```bash
lf auth connect claude personal@example.com --chrome-profile Personal
lf auth set claude personal@ --chrome-profile Personal
lf auth set claude personal@ --routing explicit-only
lf auth set claude personal@ --clear-cooldown
lf auth disconnect claude personal@
doppler run -- lf auth connect linear
```

A managed login is an existing spending identity, not a new provider account.
A browser profile is only the venue used to authenticate it. Saved browser
choices can be reused on reconnect. `auth connect claude EMAIL --import`
explicitly adopts an ambient Claude login.

Claude and Codex accounts can be routed individually. Service credentials such
as GitHub and Linear are connected for their operations. Source secrets through
Doppler; never paste credentials into command arguments or saved documents.

### Choose accounts for a launch

```bash
lf -m codex --account work@ run implement
lf --account claude=personal@ --account codex=work@ run feature
lf --only-account codex=work@ skill review-design
```

`--account <email-prefix>` prefers matching managed accounts before the normal
provider route. The first preferred attempt bypasses stored health; a missing
credential continues through the healthy fallback route. `--only-account`
restricts the launch and its children to the selected accounts. Both accept
provider-qualified selectors and can be repeated; they cannot be combined.
Use these flags for terminal launches too (`--tui`), so the provider shares
the managed credential rather than creating a competing login.

```bash
lf auth route set codex work@ personal@
lf auth route set codex personal@ --default
lf auth route set codex work@ --repo owner/repository
lf auth route show --json
```

Repository routes override the default route. If neither exists, all automatic
managed logins are eligible. Automatic selection skips known cooling or limited
accounts; with no managed login, the provider CLI uses its ambient credentials.
Selection, credential readiness, and observed capacity are reported separately.

## Home: installation and machines

```bash
lf home id --json
lf doctor
lf user name
lf desktop
lf install
lf install schedule daily
```

A Home has a stable machine identity. Its SSH route can change without changing
that identity. Installation and machine inspection work outside a repository.

### Reach another Home

```bash
lf home observe HOME_ID ssh://jack@mini.local
lf ssh HOME_ID wave status designer --json
lf ssh HOME_ID task status DES-123 --json
```

Everything after the SSH target is the remote `lf` invocation. Origin SSH
options belong before that target. The target resolves and verifies its Home
identity. Foreground credential forwarding does not install managed provider
credentials remotely; durable descendants must have the authority they need.

### Install and diagnose

```bash
lf install                          # latest published release
lf install schedule                 # login and weekly checks on macOS
lf doctor --planning
lf doctor --json
```

Install updates the CLI and supported application components from verified
release artifacts. It does not require a source checkout. Use `sync`
to update repository work instead. Scheduled installation supports weekly,
daily, hourly, and five-minute cadences on macOS; Linux supports explicit
installation.

Doctor reports build provenance, database and migration state, planning drift,
and continuity evidence. Missing scheduler receipts point to the relevant
`cron history` command. Inspection does not repair state silently.

Source builds may use a private data directory. Managed Task launches report
the installation that owns execution; follow that reported binary and store
when inspecting the running work. A private database does not isolate shared
worktree edits or external service effects.

### Capture a page or measure context

```bash
lf screenshot page.html -o page.png
lf screenshot https://loopflow.studio -o mobile.png --width 390 --height 844
lf tokens
lf tokens --days 365
lf tokens --json
```

Screenshot uses a standalone headless browser and a temporary profile. Failed
or interrupted captures preserve existing output. If the backend is missing,
install it with `playwright install --only-shell chromium`.

Tokens reports line and model-token counts by tracked path. History reads Git
blobs without checking them out; untracked and non-UTF-8 files are skipped.

## Repo: releases and integrations

```bash
lf ci --since 7d
lf ci --since 7d --json
lf release check
lf release run patch
lf release status
```

CI reports repair attempts and their later passing or merge observations.
Release run owns the release lifecycle: preparation, verification, notes,
tagging, publication, and recovery. An interrupted release resumes its
recorded state rather than skipping to a newer version.

### Release operations

```bash
lf release run minor
lf release notes 1.2.3 --preview
lf release bump 1.2.3
lf release tag 1.2.3
lf release publish v1.2.3 --notes RELEASE_NOTES.md --asset dist/lf.tar.gz
lf release publish v1.2.3 --finalize
```

Use the full run for a release, or individual operations for deliberate control.
Tag creates and pushes a tag; publish changes the hosted release. Notes preview
prints the selected version's narrative without writing release state.

Repository-specific checks and artifact publishers live under `release.targets`
in configuration. Release evidence combines the exact shipped range with
`release/unreleased/DECISIONS.md`; published records live under
`release/vVERSION/`. `RELEASE_NOTES.md` holds the latest notes.

### Planning integrations

```bash
lf repo reteam                      # preview the repository Team migration
lf repo reteam --apply
```

Reteam moves linked planning onto the repository's Team and refreshes ownership
and snapshots. It defers work that can still write an old identifier and resumes
an incomplete migration. Task workers refresh connected issue comments through
the planning provider; no resident webhook service is required.

## Scripts and structured output

```bash
lf task status DES-123 --json
lf wave status designer --json
lf runs --task DES-123 --json
lf auth status --details --json
```

Use `--json` on read commands that advertise it. Help lists the supported
options at each command. JSON distinguishes unknown values from empty results
and keeps ordering independent of terminal presentation. Errors go to stderr.

Use canonical command paths in durable automation. They state the owner and
remain explicit when a future command adds another matching short name.
Use `run`, `skill`, or `flow` when the intended target is an authored definition.

## See also

[Get Started](getting-started.md) · [Waves](waves.md) ·
[The Agent API](agent-api.md) · [Conducting](conducting.md) ·
[Authoring](authoring.md) · [Configuration](config.md) · [Security](security.md)
