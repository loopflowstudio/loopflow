# lf command reference

> Design preview: the target CLI for this branch, not the installed binary.
> Other documentation still describes the current CLI.

For examples ordered by workflow, see [lf CLI](lf.md). This page lists
individual controls, arguments, and effects.

```bash
lf debug -c                         # investigate an error from the clipboard
lf run feature                      # run a named flow or skill
lf : "fix the broken link"           # run an inline prompt
lf list                             # discover commands, skills, and flows
lf help task pr                      # inspect pull-request operations
```

Run a skill or flow by name. Use the command tree to manage work, inspect
execution, and operate the machine.

## Find a command

```bash
lf help                             # brief overview
lf help --all                       # the complete command tree
lf help task                        # concrete work and delivery
lf help task pr land                 # explain one operation
lf task pr land --help               # the same page
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
| `task` | Manage concrete work, worktrees, commits, and pull requests |
| `wave` | Maintain goals, memory, conversations, schedules, and chapters |
| `session` | Open conversations, ask for input, and complete reviews |
| `monitor` (`mon`) | Inspect history, live activity, replay, and usage |
| `identity` (`id`) | Connect accounts, inspect capacity, and configure routing |
| `home` | Install Loopflow, inspect the machine, and reach other Homes |
| `repo` | Release software, inspect CI, and configure repository integrations |

## Omit an unambiguous owner

```bash
lf task pr land                     # canonical path
lf pr land                          # omit task
lf land                             # omit task and pr

lf task publish                     # task pr publish
lf repo publish                     # repo release publish
```

An exact command wins first. Otherwise, Loopflow searches beneath the owner
already entered. One match expands to its full path. At the root, the search
covers the whole command tree. Any number of leading owners can be omitted.

Multiple matches print the choices and execute nothing:

```text
$ lf publish
"publish" matches multiple commands:
  lf task pr publish
  lf repo release publish
```

Use an explicit owner to narrow the search. `lf task status` keeps its own
meaning; inspecting just the PR is `lf task pr status`.

Shortcuts have the same arguments and effects as their canonical command.
`lf help land` shows `lf task pr land`. Examples below use full paths so the
owner is visible; omitted-owner shortcuts are derived from that tree.

### Long and short names

```bash
lf monitor top                      # full name
lf mon top                          # fixed short name
lf identity status
lf id status
```

Commands have one canonical name and may advertise a fixed short name in help.
Both names reach the same operation. Already-short names such as `pr`, `task`,
and `land` need no second spelling. Use the documented short name, not an
arbitrary prefix.

| Full name | Short name |
|---|---|
| `monitor` | `mon` |
| `identity` | `id` |
| `pr` | `pr` |
| `task` | `task` |
| `land` | `land` |

An explicit short name wins before owner omission. Thus `lf id` selects
identity commands. Print the machine identifier with `lf home id`.

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
| `refresh` | Rebase, then reconcile the design and implementation |
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
do not advance a Task's managed Flow. A direct flow has its own invocation.
Inside a registered Task checkout, an unqualified launch inherits that Task;
an ordinary branch stays unbound.

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
commit. Several Runs may concern the same Task; attribution does not grant
exclusive ownership of its files.

Put global options before the command. Command-local options retain their
meaning after shorthand expansion: `lf commit -m "Fix startup"` supplies a
commit message, not a model. `--` ends option interpretation where literal
arguments are accepted.

### Flow decisions and recovery

```bash
lf flow resume INVOCATION
lf flow resume INVOCATION --retry
lf task run DES-123                  # continue the Task's saved Flow
```

Resume an invocation to keep its captured definition, feedback, and position.
Invoking its name again starts a new invocation. A changed definition applies
to future invocations; it does not rewrite work already in progress.

Decision steps use the flow's authored edges:

```bash
lf flow decide advance "The required proof passed"
lf flow decide iterate "Fix the remaining parsing case"
lf flow route PATH
lf flow blocked "Two attempts failed; need a decision on scope"
```

Blocked opens or rejoins an Ask for that boundary. Completing the Session
returns feedback to the caller; the decision step chooses the next edge.
Completing a review does not itself mean Advance or Iterate.

Inspect external effects before retrying an interrupted operation. A failed
client may already have pushed a branch or changed remote state. An unreadable
saved invocation remains available for recovery rather than being replaced
silently. See [Authoring](authoring.md) for loops, branches, and review steps.

## Task: concrete work and delivery

```bash
lf task list                        # work in this repository
lf task list --wave designer
lf task list --all                   # work across this Home's repositories
lf task status                      # Task bound to this checkout
lf task pr status                   # current branch's PR, with or without a Task
```

Task commands manage concrete work. Commit, rebase, worktree, and PR operations
also work in an ordinary checkout. They do not create an issue merely to satisfy
the command path.

Task list includes tracked Tasks and unlinked checkout/PR work, with missing
issue links shown. Discovering that work does not create Task records.

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
lf task interrupt DES-123            # interrupt the active provider turn
lf task run DES-123                  # continue after interruption
lf task restart DES-123 --flow feature
lf task run DES-123 --reason "Repository credentials repaired"
lf task wait DES-123 --until terminal --timeout 15m
```

Comments provide direction without starting an idle Task. The advancing worker
reads them; independent Task-bound Runs do not receive a broadcast. Use
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
obligations settle. Delete removes the connected issue and reconciles the local
record; it preserves authored files and retained history. Deleting unfinished
work does not record success. If an operation partially succeeds, its error
identifies the retained state and the command to retry.

### Read and edit another Task's files

```bash
lf task changes DES-123 --base head --json
lf task diff DES-123 src/parser.rs --base parent
lf task file DES-123 src/parser.rs --json
lf task save DES-123 src/parser.rs --revision REVISION --json < draft.rs
```

File reads use recorded placement without starting a Run. Comparisons default
to the recorded PR base (`parent`); use `head` for the current commit or a
returned base SHA for a pinned comparison. `diff --draft` compares stdin
without writing it.

Save requires the revision returned by file inspection. A detected conflict
leaves the current file intact. Submitted drafts and displaced versions are
retained for recovery; `task file --recoveries` includes that history. Text
operations require UTF-8 files within 1 MB and exclude symlinks and Git metadata.

### Worktrees

```bash
lf task worktree create parser
lf task worktree create parser --plan
lf task worktree switch parser
lf task worktree list
lf task worktree prune --dry-run
lf task worktree remove parser
```

Create a worktree for ordinary local work, or use `task checkout ISSUE` for a
tracked Task. Creation refreshes the default branch before choosing its base;
`--plan` previews local placement without fetching or writing.

Prune removes eligible clean, settled or stale worktrees. It preserves dirty
work, the current checkout, the default branch, nonterminal Tasks, and live
owned work. Inspect `--dry-run` before cleanup. Explicit forced removal is a
separate destructive choice.

### Commit and rebase

```bash
lf task commit -m "Fix keyboard navigation in dialogs"
lf task commit --no-add              # commit only the existing index
lf task rebase --plan                # inspect the integration strategy
lf task rebase                      # integrate and publish the branch with a lease
```

Commit stages changes and generates a message unless given explicit options.
Publishing is a separate PR operation.

Rebase refreshes upstream, preserves local edits, integrates the appropriate
base, and pushes the resulting branch with a lease. The default branch's local
unpublished commits are preserved; it is not pushed. `--plan` uses local
evidence and makes no changes.

```bash
lf task rebase --manual
lf task rebase --continue
lf task rebase --abort
```

Manual recovery stays local and does not push. A retained conflict keeps its
sequencer and recovery context. An interrupted edit restoration reports the
retained stash. Use `--adopt` with `--continue` or `--abort` only when explicitly
taking over an operation started outside Loopflow.

### Pull requests

```bash
lf task pr open                     # push a draft and open its page
lf task pr publish                  # push and mark ready, without opening a browser
lf task pr checks --watch
lf task pr submit                   # prepare for a reviewer's merge click
lf task pr arm                      # request auto-merge and return
lf task pr land                     # watch, repair CI, and finish merged
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

Publish leaves scratch notes and integration history alone. It does not rebase.
Submit, arm, and land preserve useful conclusions, clear scratch, prepare a
reviewable commit, and integrate the branch before publication. They preserve
valid reviewer-facing copy and update Task merge consequences.

```bash
lf task pr publish --title "Make dialog navigation follow tab order" \
  --body "Keyboard focus now follows the visible controls."
lf task pr land -c                   # merge, then complete the owning Task
lf task pr land --next focus-ring    # merge, then continue the same Task
lf task pr next focus-ring           # reconcile a merge performed elsewhere
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
lf wave chat --wave designer --follow
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
doppler run -- lf identity connect linear
lf wave connect designer --team-key DSG
lf wave sync designer
lf wave status designer --sync
lf wave status designer --no-sync
```

Connection links the Wave to its planning service. Each Wave has one current
chapter, whose plan holds KRs, metric targets, and Tasks. The provider Project
is internal; navigate with Waves and Tasks.

Status reads local planning unless asked to refresh. A failed fresh read is
reported rather than presented as current evidence. Connecting a repository's
other Waves reuses its Team; `wave connect --all` discovers authored Waves.

### Replace the chapter

```bash
lf wave update-plan --wave designer --plan plan.json
lf wave new-chapter --wave designer --chapter 2026-10 \
  --plan plan.json --dry-run --json
lf wave new-chapter --wave designer --chapter 2026-10 \
  --plan plan.json --json
lf wave history --wave designer --json
lf wave status designer --chapter 2026-09
```

Update-plan changes the current chapter's KRs, metric targets, and recommended
Flow. New-chapter replaces the plan and records a boundary receipt. Started,
unfinished Tasks retain their identities, worktrees, PRs, and Flow positions.
Untouched backlog is abandoned; completed work remains historical. Missing
evidence stays unresolved.

Use the dry run to inspect each Task's disposition. A stale planning label
alone does not erase recorded execution or authored work. Repeat the same
operation to recover an incomplete transition rather than manually rotating
provider Projects.

### Conversations

```bash
lf wave chat --wave designer "Prioritize the dialog audit"
lf wave chat --wave designer --follow
lf wave chat --wave designer --history --json
lf wave reply designer "Does this change need a response?"
```

Chat posts into the durable Wave thread. `--follow` replays recent turns and
keeps the conversation open; `/status` reads health and `/quit` leaves.
`--history` reads saved conversation without starting a listener. Reply makes
one reply decision and prints only a warranted response.

Select a Wave explicitly outside bound work. A repository may contain several
Waves; its location alone does not select one.

### Schedules

```bash
lf wave cron preflight --wave designer
lf wave cron sync --wave designer
lf wave cron list --wave designer --json
lf wave cron trigger --wave designer --flow vsm-operate --wait
lf wave cron history --wave designer --days 35
```

Author schedules in the Wave's `GOAL.md`. Preflight checks its placement and
installed target without changing launchd. Sync reconciles the declaration on
the owning Home. Trigger exercises the installed job; history reads its
durable firing receipts.

Use `wave cron add` or `wave cron remove` for explicit installed-job changes.
Schedules run through the installed CLI and use flow-first definition lookup.
The Home's installation-update schedule is separate under `home install`.

### Placement and retirement

```bash
lf wave place WAVE_ID HOME_ID
lf wave probe designer
lf wave relocate WAVE_ID --name platform
lf wave relocate WAVE_ID --repo ../platform
lf wave retire designer --reason "Responsibility transferred"
lf wave forget WAVE_ID --dry-run
lf wave recover designer
```

Placement selects the Home responsible for execution. It does not launch work
or redirect every local command to that machine. Relocation preserves identity
and authored state; it reports live work that must settle first.

Retirement retains history. Forget removes an empty registration after authored
files have been removed. Recover inspects retained historical continuations;
cancel one explicitly with `--cancel SEQUENCE --reason TEXT` when appropriate.
`wave rename NAME --title TEXT` changes the connected provider's display title.

## Session: conversations and reviews

```bash
lf session list
lf session ask "Review this proof with me"
lf session ask --skill unblock "Choose how to resolve this repeated failure"
lf session open SESSION_ID
lf session rename SESSION_ID "Keyboard navigation review"
lf session complete SESSION_ID
```

Ask opens a conversation and waits for completion. A review step uses the same
Session surface. Ready, provider exit, and closing a pane do not complete it.

```bash
lf session ready "Proof and remaining questions are ready"
lf session complete SESSION_ID
```

Complete returns feedback and artifact changes to the waiting caller. In a
Flow, the following decision step chooses navigation. Complete also finishes
an ordinary interactive Session while preserving its provider history.

`session open --json` prepares attachment and returns `open_argv`; execute that
command to enter the conversation with the recorded binary and data context.
`--replace` stops Loopflow's owned client before resuming here. `--try` asks
the provider to arbitrate an already active session.

Session JSON includes `run_id`; use it to inspect execution rather than parsing
the Session ID. Agents proposing a title use `session rename --suggest`, which
preserves a user-assigned name.

## Monitor: history and live activity

The [proposed overview](lf.md#inspect-work), `lf monitor`,
combines these observations into work needing a decision, blocked work,
progress, and results. The commands below inspect the underlying evidence.

```bash
lf mon list
lf mon list --task DES-123 --json
lf mon list --parent RUN_ID --json
lf mon show RUN_ID --final
lf mon show RUN_ID --events
lf mon ps                           # one live process/call-tree snapshot
lf mon ps --json
lf mon top                          # refresh on a terminal
lf mon active --task DES-123 --json
lf mon active --watch --json
lf mon activity --task DES-123
```

`mon` is the short spelling of `monitor`. History reads durable Run records.
`ps` prints live processes and call trees; `top` refreshes that view on a
terminal and prints once when redirected or given `--json`. `lf ps` and
`lf top` are derived shortcuts for these commands.

Active discovers live Runs, including Task filtering. It is a different view
from the process/call-tree snapshot. Activity orders Work, Run, PR, and
direction changes. These views identify their evidence and gaps; an unfinished
history record is not proof of a live process, and an empty partial observation
is not proof that nothing is running.

Run IDs accept unambiguous prefixes. `--parent` returns every direct child Run
without the recent-history cap. `--final` prints the last durable provider
conclusion; `--events` prints its event stream. Each command reads the selected
Home rather than silently aggregating other machines.

Continuous discovery uses `active --watch --json` on macOS. It streams
newline-delimited snapshots. Linux supports one-shot observation. Live process
ownership remains distinct from durable attribution and permission to signal it.

### Replay and usage

```bash
lf mon replay RUN_ID
lf mon usage --days 30
lf mon usage --task DES-123 --json
lf mon usage --days 0 --json
```

Replay starts a child Run from the recorded request, model, account identity,
and tool boundary. It does not rebuild the prompt from today's configuration.
It can repeat side effects; inspect the original request before replaying it.
Use `session open` to resume a conversation instead.

Usage reports provider-authored counters from Run records. Missing counters
remain unknown, and final receipts are distinguished from partial streams.
`--days 0` includes all retained history. Subscription windows and account
capacity belong to `identity status`, not this usage report.

### Cleanup

```bash
lf mon prune --dry-run
lf mon prune
```

Prune cleans stale receipts and eligible registered orphan process groups. It
does not infer termination authority from a PID or kill unclaimed providers.
Worktree cleanup is `task worktree prune`.

## Identity: accounts, capacity, and routing

The [proposed readiness overview](lf.md#accounts-and-access), `lf identity`,
explains usable connections and what needs attention. Ordinary launches also
check the access they need; the commands below provide explicit control.

```bash
lf id status
lf id status codex --verify
lf id connect codex work@example.com --chrome-profile Work
lf id route set codex work@ personal@
lf id route show
```

`id` is the short spelling of `identity`. Connect an existing provider login,
inspect its credential state and observed capacity, and choose which accounts
a repository may use. Account selection does not change the selected model.
An incompatible explicit account/model combination reports an error.

Status uses cached evidence by default. `--verify` requests fresh supported
observations; `--details --json` includes sources and timestamps. Unknown or
stale capacity is labeled rather than treated as zero or unlimited.

### Connect and configure

```bash
lf id connect claude personal@example.com --chrome-profile Personal
lf id set claude personal@ --chrome-profile Personal
lf id set claude personal@ --routing explicit-only
lf id set claude personal@ --clear-cooldown
lf id disconnect claude personal@
doppler run -- lf id connect linear
```

A managed login is an existing spending identity, not a new provider account.
A browser profile is only the venue used to authenticate it. Saved browser
choices can be reused on reconnect. `identity connect claude EMAIL --import`
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

`--account` prefers matching accounts before the normal provider route.
`--only-account` restricts the launch and its children to the selected accounts.
Both accept provider-qualified selectors and can be repeated; they cannot be
combined. Use these flags for terminal launches too, so the provider shares
the managed credential rather than creating a competing login.

```bash
lf id route set codex work@ personal@
lf id route set codex personal@ --default
lf id route set codex work@ --repo owner/repository
lf id route show --json
```

Repository routes override the default route. Where automatic selection is
used, accounts with known cooling or limited state are skipped. Selection,
credential readiness, and observed capacity are reported separately.

## Home: installation and machines

```bash
lf home id --json
lf home doctor
lf home user name
lf home desktop
lf home install
lf home install schedule daily
```

A Home has a stable machine identity. Its SSH route can change without changing
that identity. Installation and machine inspection work outside a repository.

### Reach another Home

```bash
lf home observe HOME_ID ssh://jack@mini.local
lf home ssh HOME_ID wave status designer --json
lf home ssh HOME_ID task status DES-123 --json
```

Everything after the SSH target is the remote `lf` invocation. Origin SSH
options belong before that target. The target resolves and verifies its Home
identity. Foreground credential forwarding does not install managed provider
credentials remotely; durable descendants must have the authority they need.

### Install and diagnose

```bash
lf home install                     # latest published release
lf home install schedule            # login and weekly checks on macOS
lf home doctor --planning
lf home doctor --json
```

Install updates the CLI and supported application components from verified
release artifacts. It does not require a source checkout. Use `task rebase`
to update repository work instead. Scheduled installation supports weekly,
daily, hourly, and five-minute cadences on macOS; Linux supports explicit
installation.

Doctor reports build provenance, database and migration state, planning drift,
and continuity evidence. Missing scheduler receipts point to the relevant
`wave cron history` command. Inspection does not repair state silently.

Source builds may use a private data directory. Managed Task launches report
the installation that owns execution; follow that reported binary and store
when inspecting the running work. A private database does not isolate shared
worktree edits or external service effects.

### Capture a page or measure context

```bash
lf home screenshot page.html -o page.png
lf home screenshot https://loopflow.studio -o mobile.png --width 390 --height 844
lf home tokens
lf home tokens --days 365
lf home tokens --json
```

Screenshot uses a standalone headless browser and a temporary profile. Failed
or interrupted captures preserve existing output. If the backend is missing,
install it with `playwright install --only-shell chromium`.

Tokens reports line and model-token counts by tracked path. History reads Git
blobs without checking them out; untracked and non-UTF-8 files are skipped.

## Repo: releases and integrations

```bash
lf repo ci --since 7d
lf repo ci --since 7d --json
lf repo release check
lf repo release run patch
lf repo release status
```

CI reports repair attempts and their later passing or merge observations.
Release run owns the release lifecycle: preparation, verification, notes,
tagging, publication, and recovery. An interrupted release resumes its
recorded state rather than skipping to a newer version.

### Release operations

```bash
lf repo release run minor
lf repo release notes 1.2.3 --preview
lf repo release bump 1.2.3
lf repo release tag 1.2.3
lf repo release publish v1.2.3 --notes RELEASE_NOTES.md --asset dist/lf.tar.gz
lf repo release publish v1.2.3 --finalize
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
doppler run -- lf repo webhook register --url https://example.com/linear
doppler run -- lf repo webhook serve
```

Reteam moves linked planning onto the repository's Team and refreshes ownership
and snapshots. It defers work that can still write an old identifier and resumes
an incomplete migration. Webhooks deliver connected issue and comment changes
to the Task workflow; the signing secret is supplied through Doppler.

## Scripts and structured output

```bash
lf task status DES-123 --json
lf wave status designer --json
lf mon list --task DES-123 --json
lf id status --details --json
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
