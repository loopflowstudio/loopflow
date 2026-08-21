# lf Command Reference

One binary, three audiences. `lf` launches prompts for humans, gives agents
the verbs to run and steer other agents, and reads the local ledger for
whoever is watching. The map:

| You are | Start with | Deep dive |
|---|---|---|
| A human running prompts | [Basic Usage](#basic-usage), [Context Flags](#context-flags) | [Get Started](getting-started.md) |
| A human operating waves | [Running Waves, Projects, and Tasks](#running-waves-projects-and-tasks), [Speaking to Waves](#speaking-to-waves) | [Waves](waves.md) |
| An agent driving other agents | [Running Waves, Projects, and Tasks](#running-waves-projects-and-tasks) | [The Agent API](agent-api.md) |
| Watching the whole machine | [Reading the Local Ledger](#reading-the-local-ledger) | [Conducting](conducting.md) |

Every read surface takes `--json`; that JSON is the same wire the Mac app
renders.

## Basic Usage

```bash
lf                                 # terminal-native Loopflow control conversation
lf desktop                         # explicitly open or focus Loopflow.app
lf <skill>                        # run a skill file
lf <skill>: args                  # run with arguments
lf <namespace>/<skill>            # run a repo-local or installed namespaced skill
lf npx/<owner>/<repo>            # fetch any Claude Skill live via npx skills
lf : "inline prompt"             # no skill file, just prompt
lf list                          # show skills and flows, including flow expansions
lf -l                            # short form of `lf list`
lf ls                            # list Waves in the local registry
```

## Examples

```bash
lf gate                           # run the gate skill
lf implement: add auth            # pass arguments after colon
lf team/review                    # run .lf/skills/team/review.md
lf npx/vercel-labs/deep-research  # fetch a skill from the npx skills catalog
lf : "fix the typo"               # inline prompt
lf debug -c                       # paste clipboard, fix the bug
lf --as task:DES-123 implement    # run one skill as existing Task Work
lf task run DES-123 --directive "fix the flaky test" # keep one Task through merge
lf task run DES-124 --stack-on DES-123                # dependent Task, separate worktree
```

## Browser Captures

```bash
lf screenshot page.html -o page.png
lf screenshot https://loopflow.studio -o mobile.png --width 390 --height 844
```

`lf screenshot` uses the standalone `chrome-headless-shell`, a temporary
profile, and a fixed 30-second lifetime. It never falls back to the Google
Chrome app, so unattended capture cannot claim the user's browser instance.
Failed and interrupted captures leave any existing output unchanged. Install a
missing backend with `playwright install --only-shell chromium`.

## Skills

Names resolve in this order:

1. `.lf/skills/<skill>.md` or `.lf/skills/<ns>/<skill>.md` — repo-local (also overrides builtins)
2. `.claude/commands/<skill>.md` — Claude Code compatible
3. `~/.lf/skills/<skill>.md`, `~/.lf/skills/<ns>/<skill>.md`, or `~/.claude/commands/<skill>.md` — user-global
4. Core built-in skills, grouped by Task, Project, Wave, and Ops (`lf list` shows the live catalog)
5. External skill namespaces — `npx/<owner>/<repo>` fetches live via `npx skills` and caches under `.agents/skills/`; cached or searchable skills can often be run as `npx/<name>`. The legacy `rams/rams` alias also resolves when `~/.claude/commands/rams.md` exists.

Namespaced skills and flows use `/`, not `:`. Run `team/review`, not `team:review`.
Ownership uses `/` (`wave/clarify`); words within one name use `-`
(`review-slice`). Public catalog names never use `_`.

### Skill Arguments

```bash
lf implement: add user authentication
```

Inside skill files, `{args}` is replaced with whatever comes after the colon.

### Builtin Catalog

Skills and flows share one catalog organized by the thing they act on:
**task**, **project**, **wave**, and **ops**. `lf list` shows each flow both as
written and collapsed into the skills and operations that execute.

Task skills — concrete implementation, investigation, review, and delivery:

| Skill | What it does |
|------|--------------|
| `kickoff` | Elaborate design — alternatives, research, imagine success/failure |
| `research` | Map the territory — architecture, complexity, quality, potential |
| `iterate` | Read research, write design to address it |
| `refresh-plan` | Reconcile scratch/ with the branch after rebasing |
| `5whys` | Root cause analysis on a bug fix |
| `implement` | Build from a design doc |
| `compress` | Simplify touched code |
| `gate` | Ship-ready code and reviewer-friendly docs |
| `debug` | Fix an error |
| `ci-fix` | Fix failing CI checks for the current PR |
| `integrate-upstream` | Adapt wave code after rebasing onto main |
| `qa` | Thorough quality assessment of the current branch |
| `triage` | Assess QA findings, separate blocking from polish |
| `design` | Interactive design session |
| `explore` | Investigate the codebase |
| `review-slice` | Autonomously demonstrate behavior, audit implementation against plan, and publish the slice |
| `demo` | Walk the User through the changed behavior, or prove it headlessly and ask one exact blocking question |
| `review-design` | Reshape AI-elaborated design into user intent |
| `refine` | Refine existing work |
| `task/clarify` / `task/pursue` / `task/mutate` | Clarify, implement, and judge one durable Task |

Project skills — shape and pursue measured bets inside a Wave:

| Skill | What it does |
|------|--------------|
| `project/clarify` / `project/pursue` / `project/mutate` | Clarify, advance, and judge a Project |
| `project-promote` | Promote a Project into a resident child Wave |
| `expand` / `reduce` / `polish` | Find higher leverage, simplifications, and finish quality |
| `testing-audit` | Audit test value, rigor, cost, lifecycle ownership, and product proof |

Wave skills — maintain the durable operating context and its portfolio:

| Skill | What it does |
|------|--------------|
| `scan` | Read member wave state — PRs, blocks, progress, git activity |
| `assess` | Judge wave health and identify pressure points |
| `wave-report` | Read health signals across all waves |
| `mutate` | Compose and apply coordinated mutations across member waves |
| `review` | Review mutations, amend or revert if needed |
| `wave/clarify` / `wave/pursue` / `wave/mutate` | Clarify, direct, and evolve a Wave |
| `review-open-work` | Survey branches, PRs, worktrees, and waves for inbox-zero triage |
| `update-wave` / `split-wave` | Maintain Wave structure and memory |
| `s2-scan` / `s2-assess` | Coordination: backlogs, PR/path overlap, conflict risk and safe ordering |
| `s3-scan` / `s3-assess` | Control: live health, velocity, CI, retries, worker-pool size |
| `s4-scan` / `s4-assess` | Intelligence: dependencies, advisories, upstream APIs, what they imply |
| `s5-scan` / `s5-assess` | Identity: wave roster, policy, boundary and autonomy drift |

Ops skills — raw prompt logic around mechanical git, PR, and release commands:

| Skill | What it does |
|------|--------------|
| `init` | Connect the repo to Homes, accounts, Waves, and task execution |
| `loopflow-validate` | Validate flows, skills, and directions |
| `commit-message` | Generate a commit message without committing |
| `rebase-conflicts` | Resolve conflicts after the mechanical rebase stops |
| `pr-message` | Generate a PR title and body without publishing |
| `pr-publish` | Generate PR copy and call `lf pr publish` |
| `pr-submit` | Prepare a PR for a human to land |
| `pr-land` | Prepare and land a PR through Loopflow's git machinery |
| `release-run` | Run the full release workflow (notes, PR, tag, status) |
| `release-notes` | Write narrative `RELEASE_NOTES.md` from release context |
| `token-compress` | Compress text into a token budget without silently dropping information |

## Context Flags

Write global flags before a built-in subcommand. Unambiguous flags also work
after it:

```bash
lf task run DES-123 --json                           # durable Task Work
lf task status DES-123 --json                        # same identity and worktree
lf task changes DES-123 --json                       # committed + working changes
lf task diff DES-123 src/parser.rs --json            # one file's Task patch
lf task file DES-123 src/parser.rs --json            # current worktree contents
lf pm --wave designer show                           # normalized onto `show`
lf pm task --wave designer create --title "Fix it"  # normalized onto `create`
lf commit -m "explain the change"                   # -m remains commit-local
```

Flags may cross nested subcommands to reach a selected command that owns the
spelling. If more than one level owns it, a flag already valid at its current
level stays there. Put `--` before literal arguments that look like flags.

### Files and Directories

| Flag | Description |
|------|-------------|
| `--docs PATH[,PATH...]` | Prefetch docs into context—files, globs, or dirs (default: none) |
| `-w, --wave NAME` | Wave name for wave/ scoping |
| `--as task\|project\|wave:SELECTOR` | Run one named skill as existing Work. In a plain terminal this starts a supervised User Run in that Work's cwd; inside a Run it asserts the exact ambient Work. Flows are rejected. |
| `--diff-files / --no-diff-files` | Include files touched by branch (default: off) |
| `--diff / --no-diff` | Include raw `git diff` output |

### Loopflow Guidance

| Flag | Description |
|------|-------------|
| `--no-loopflow` | Omit `LOOPFLOW.md` operating guidance |

### Clipboard

| Flag | Description |
|------|-------------|
| `-c, --clipboard` | Include clipboard content in prompt |

## Run Mode Flags

| Flag | Description |
|------|-------------|
| `-i, --interactive` | Run interactively (can interrupt, redirect) |
| `-b, --batch` | Run in batch/headless mode |
| `--max-turns N` | Cap agent turns for this invocation |

## Model Flags

| Flag | Description |
|------|-------------|
| `-m, --model MODEL` | Choose model (e.g., `claude:opus`, `codex`, `gemini`, `opencode`) |
| `-d, --direction DIRECTION` | Apply direction (comma-separated for multiple) |

## Output Flags

| Flag | Description |
|------|-------------|
| `--tui` / `--ide` | Hand off Claude, Codex, or OpenCode to the terminal, or Claude/Codex to their app; overrides `session.launch` |

## Browser Automation

| Flag | Description |
|------|-------------|
| `--chrome / --no-chrome` | Enable Chrome browser automation |

## Running Flows

Run a named flow (chains of skills):

```bash
lf <flow>
lf ship -w feature-branch
```

| Flag | Description |
|------|-------------|
| `--docs PATH[,PATH...]` | Prefetch docs into context—files, globs, or dirs (default: none) |
| `-w, --wave NAME` | Wave name for wave/ scoping |
| `-m, --model MODEL` | Model to use |
| `--tui` / `--ide` | Hand off Claude, Codex, or OpenCode to the terminal, or Claude/Codex to their app; overrides `session.launch` |

Flows are defined in `.lf/flows/`. See [Configuration](config.md).

### Builtin Flows

| Flow | Steps |
|------|-------|
| `build` | kickoff → code → review-slice → demo |
| `code` | implement → compress |
| `pair` | design → code |
| `design` | author one exact design at a User gate |
| `launch-plan` | keep one coherent core here and launch independent follow-up Tasks |
| `task-design` | kickoff → review-design |
| `slice` | code → review-slice → publish/refresh Task PR |
| `ship` | task-gate → record-learnings → op: pr land -c |
| `ship-demo` | task-gate → human demo review → record-learnings → op: pr land -c |
| `deploy` | gate → op: pr land |
| `design-and-ship` | design → implement → reduce → polish → deploy |
| `incident` | restore → 5whys |
| `ship-5whys` | implement the next open prevention from the 5 Whys |
| `queue` | compress → update-wave → gate |
| `garden` | scan → assess → xor(garden-act, silence) |
| `govern-coordination` | s2-scan → s2-assess → mutate |
| `govern-control` | s3-scan → s3-assess → mutate |
| `govern-intelligence` | s4-scan → s4-assess → mutate |
| `govern-identity` | s5-scan → s5-assess → mutate |
| `sync` | rebase → integrate-upstream |

`sync` rebases the current branch and refreshes the default branch. The
default-branch refresh is safe from sibling worktrees: it stashes dirty edits
on the checked-out default branch, syncs, then restores them — unless they
collide with paths the sync rewrote, in which case they stay in a
`sync_main: auto-stash` stash so a sync can never silently revert just-landed
work.

Flow authoring — `op:` steps, `xor` branching, routers — is covered in
[Authoring](authoring.md).

## Running Waves, Projects, and Tasks

```bash
lf start designer                                  # serve it on this machine
lf wave designer                                   # foreground development mode
lf pause designer                                  # keep listening; queue new turn starts
lf resume designer                                 # enable queued and future turns
lf stop designer                                   # stop it; leave the Home keeper running
lf project run <linear-project-id>                  # durable Project Work
lf task start <linear-project-id> "fix the flaky chord-timeout test"
pbpaste | lf task start incident-management
lf task run DES-123 --directive "fix the parser before the docs"
lf task run DES-124 --stack-on DES-123
lf task run DES-125 --first incident --loop ship-5whys --finally ship-demo
lf task status DES-123
lf ask list                                           # parent Ask queue
lf ask list --outgoing                                # this Work's unresolved requests
lf ask list --user --json                             # User attention projection
lf ask open ask_...                                   # sibling Ask terminal
lf task steer DES-123 "rename the flag"
lf task interrupt DES-123                            # no replacement direction
lf task steer DES-123 "take the smaller approach"
lf task wait DES-123
lf task resume DES-123 --model codex --reason "Claude quota exhausted"
lf project resume <linear-project-id> --model codex
lf work status task task_... --json                  # stable Work projection
lf work place wave wave_... home_...                 # move idle Wave Work to a Home
lf work relocate wave wave_... --name platform       # rename a stopped Wave
lf work relocate wave wave_... --repo ../moved-repo  # repair or move its repository
lf work disable project project_...                  # exclude it from Wave selection
lf work enable task task_...                         # restore Task eligibility
lf flow scan-pass "scan the runtime"               # one pass, no loop worktree
```

`lf start <name>` asks this machine's shared keeper to serve the Wave and
records this machine as its Home. It enables the Wave in this Home's registry
and never follows a remote placement record.
Bare `lf start` is the automatic form: it starts only repo Waves whose optional
`owner` and `home` fields in `GOAL.md` match this machine and whose recorded
placement is local and enabled. The named form is the explicit override.
`lf wave <name>` runs that Wave listener and resident in the foreground for
development. Project Work pursues one Linear Project's KRs without a worktree.
Each Project phase refreshes Linear before it starts. Definition, Task-flow, and
KR edits take effect together on the next phase without replacing the Project
Work or its direction; an unavailable or invalid plan stops before another
provider turn and status prints the restart reason.
Each Linear Project has at most one current Work; terminal Work remains readable
history and the next pursuit creates a successor. Every Task requires the
current Project Work; `task start/run` ensures it before reserving the Task.
The Task starts only after its Linear issue exists and owns one stable worktree.
Its provider process and transcript are replaceable execution state: plain
`resume` keeps compatible history; `resume --model <agent>` preserves the same
Work, durable Steers, worktree, and PR chain while selecting another provider.
The Task remains resumable through serial PRs, review, and explicit
completion.
Wave names are repository-scoped. Relocation requires the UUID because the
repository and name may both change; it preserves authored Wave files, journal,
PM binding, Work/Run history, and Home placement. It refuses live Wave,
Project, or Task Runs and never keeps an old-name alias. UUID-addressed `lf
work` reads and mutations also verify that the selected Work belongs to the
invoking repository; a UUID from another repository is not a capability.

Every Task runs `first → loop N → finally`. Its Project supplies those three
flows; Task launch pins their resolved names. `--first`, `--loop`, and
`--finally` override them only while creating the Task. Task launch expands the
three flows as one lifecycle: the loop must contain an autonomous skill step,
and the final flow must end with `op: pr land -c`. The default and feature
lifecycles ask a human to review the design before implementation, then review
the configured-path demo before settlement. The fix lifecycle keeps only the
demo review. Invalid persisted lifecycles remain visible in status with
`no_action`; abandon and replace them with a valid selection.

Every Task-owned PR keeps the Linear Task name at the start of its title and a
direct `Linear Task: [KEY](URL)` link in its body. Loopflow restores those
anchors whenever it publishes, refreshes, submits, or lands a serial PR. If the
cached PM snapshot has no provider URL, run `lf pm sync --wave <wave>` before
publishing.

Task launch also resolves the exact execution boundary the lifecycle needs:
the linked worktree's shared Git metadata, Loopflow's pinned control store, and
network access for delivery. Headless Tasks require a managed Codex or Claude
account with usable credentials. Loopflow probes the linked Git and control
roots before reserving a Run, then launches the provider inside the trusted
managed delivery boundary with unrestricted network and filesystem access. A
supervised Task establishes its active Turn record before provider work begins,
so `lf ask` always has durable authority while that Turn is live.

If a provider returns normally after a permission, control-authority, or
network command failure, Loopflow records the exact command blocker as a
non-resumable Task failure. Status assigns the next move to the User with
`no_action`; missing-process reconciliation and automatic recovery do not
replace or repeat it. Correct the capability, then run
`lf task resume ID --reason "<what changed>"` to create a fresh input boundary.

A worktree writer is authoritative only while its exact Run, Turn, and optional
Ask are live. Commit, rebase, and land refuse a genuinely concurrent writer but
atomically reclaim a writer whose durable owner is terminal, absent, or
superseded. A surviving unclaimed provider PID has no mutation authority and
cannot block settlement; its revoked writer token prevents later writes.

A skill that needs judgment runs
`lf ask "<intervention>"`; the Ask routes to the
immediate parent Run. `--user` is explicit and never inferred for root Work.
The ordinary command prints its id and request, then blocks without ending the
provider invocation. `--noblock` returns an id and `lf ask wait` joins it
later. Bare `wait` selects the newest unresolved Ask from the ambient
Invocation, then Run, then Work; pass an id when the choice must be exact.

An intervention Ask does not move Work Basis or advance a flow. A human flow
node uses a `FlowStep` Ask as its authored body: resolve advances that node,
decline returns to the preceding autonomous step, and release or process exit
keeps it parked. `lf task steer` and `lf project steer` remain unsolicited
durable direction.

## Ask sessions

```bash
lf ask "Choose the proof"                       # ask the parent; block this shell
lf ask --user "Connect Linear"                  # explicit absent-User intervention
lf ask --noblock "Check the release"            # queue and print the Ask id
lf ask wait [ask_...]                            # join newest outgoing or exact Ask
lf ask list [--user] [--outgoing] [--json]       # attention or outgoing requests
lf ask open ask_... [--json]                     # open or reattach a sibling session
lf ask open ask_... --prepare --json             # return its exact attach descriptor
lf ask presented ask_... invocation_... --json   # confirm that exact presentation
lf ask resolve ask_... "Verified summary" [--json] # explicit success from its Invocation
lf ask decline ask_... "Unsafe request" [--json] # explicit refusal
lf ask release ask_... "Unfinished" [--json]     # close this attempt and requeue
lf ask escalate ask_... --user [--json]          # transfer one parent Ask
lf ask cancel ask_... "Withdrawn" [--json]       # requester/User cancellation
```

Ask Invocations start in the origin Run's captured cwd. An intervention
Invocation receives no Run lease; a human flow-step Invocation also receives
the active step's fenced writer lease so it runs the actual authored skill.
The explicit id selects the Ask; the ambient AgentInvocation id authorizes the
mutation and must be that Ask's active Invocation. A clean exit, Ctrl-D,
exiting Ctrl-C, TERM, HUP, or proven local
disappearance never means success; it requeues the same Ask. Unreachable remote
liveness stays claimed rather than expiring on time. If the configured external
terminal fails to open, the attachable attempt remains `not-presented`; repeat
`open` to present that exact Invocation.

Loopflow.app uses the same two-part presentation boundary: `open --prepare`
claims or recovers the Ask session without launching a terminal, then `presented`
records success only after Ghostty or an external target attaches the exact
returned Invocation. A failed venue launch leaves the Ask `not-presented`.

`--stack-on` places a new Task worktree on another Task's published PR. Its PR
targets that parent branch automatically, then collapses onto `main` after the
parent merges. The two Tasks keep separate identities, worktrees, and workers.
tmux remains containment and a presentation route, not product identity.

## Placing Work and Reaching Homes

```bash
lf home id --json
lf home observe <home-id> ssh://jack@mini.local
lf work place wave <wave-id> <home-id>
lf start shipper --json
lf pause shipper --json
lf resume shipper --json
lf stop shipper
lf ssh <home-id> status shipper --json
lf ssh <home-id> start shipper --json
lf ssh <home-id> pause shipper --json
```

`lf start` returns the same Wave rows as `lf ls --json`; it does not define a
second launch-result model. With no names it starts every eligible Wave in the
current repo on this machine. `lfd` starts the same eligible set across all
repositories known to its local store and reconciles it every 30 seconds.
`lf stop` stops the selected Wave on this machine while `lfd` and sibling Waves
continue. It disables the Wave in this Home's SQLite registry, so the Home
leaves that Wave off across daemon and machine restarts without changing the
repository. An explicit `lf start <name>` enables it again. Bare `lf start`
does not start disabled Waves.

`lf work enable|disable <wave|project|task> <id>` changes the same default-on
machine control for every Work kind. The control applies only to that Work:
disabling a Wave or Project does not prohibit a User from invoking an enabled
Task directly, and it does not stop an already-running descendant.

`lf pause` and `lf resume` change turn intent, not process residency. A paused
listener keeps serving and queues messages while refusing message, heartbeat,
and cron turn starts. `lf ls` reports that authored intent as the required
`paused` field and the `TURNS` column, independently from `live`. The commands
preserve the GOAL body and unrelated frontmatter; resume removes the key because
enabled turns are the default.

`lf ssh <HomeId>` resolves the Home's current observed route and makes the
target prove that identity. The remote `lf` is implicit, so everything after
the target is normal `lf` syntax. Foreground commands can use origin and target
accounts; durable processes scrub forwarded authority before detaching.

## Presenting an Opaque AgentInvocation

```bash
lf invocation list --active --json
lf invocation status invocation_... --json
lf invocation present invocation_...                 # exec the tmux/provider attach route
lf invocation handback invocation_... --outcome succeeded
```

`present` is the generic presentation adapter for an opaque TUI Invocation: it
executes that Invocation's attach route but does not create an Ask or become
its identity. The descriptor carries the supervising Run and its stable Work,
Wave, Home, cwd, and containment alongside provider trace, explicit
handback evidence, and optional attach argv.

Closing the app or terminal does not supply handback evidence. Record the
observed boundary result with `handback --outcome
succeeded|failed|interrupted|unknown`; process exit alone does not claim
success. Invocation handback describes opaque surfaces only; it never advances
a Task flow. Human Task nodes use the durable Ask contract above.

## Speaking to Waves

The **thread** is the human surface: durable, replayed, and owned by a running
Wave. Typed Work observations carry Project and Task progress to their parent.

```bash
lf chat "ship the button audit first"       # post into the current wave's thread
lf chat -w infra "CI is red on the PR"      # target a wave by name
lf chat --parent "blocked on schema change" # escalate to the parent wave
lf chat --follow -w intelligence            # watch and speak from one terminal pane
lf chat --history --json -w intelligence    # read the saved tail while stopped
```

| Command | What it does |
|---------|--------------|
| `lf chat [TEXT]` | Post into a wave's thread; `--follow` replays the latest 12 turns and continues live while typed lines post, `/status` reads health, and `/quit` leaves. `--history --json` reads the same bounded tail directly from the journal without a listener. Commands, tools, and loop bookkeeping stay out of chat; turn failures remain visible. Without `--follow`, omitted TEXT reads stdin. Outside any wave, one-shot chat prints a short drop note and exits 0 |

A Wave's durable memory is the ordinary repository file `wave/<name>/MEMORY.md`
— read and edit it directly.

Managed Work processes default to their invoking Wave through `LF_WAVE_ID`. From a
human shell, pass `--wave`; repository location does not identify one of the
Waves sharing `main`.

| Flag | Description |
|------|-------------|
| `-w, --wave NAME` | Target a wave by name |
| `--parent` | Target the invoking wave's parent (`lf chat`) |
| `--follow` | Replay the selected thread's latest 12 turns and continue live while typed lines post (`lf chat`) |
| `--history --json` | Read the selected Wave's durable local thread without requiring a listener (`lf chat`) |
| `--limit N` | Bound a `--history` read (default: 12) |

## Reading the Local Ledger

```bash
lf ls --json                    # every durable Wave and its Home/runtime evidence
lf status <wave> --json         # one Wave's Work hierarchy, Runs, and attention
lf roadmap --json               # current plan across Waves joined to runtime truth
lf activity                     # durable Work changes, newest first
lf activity --task INF-123 --json # filter before the bounded typed snapshot
lf runs                         # one row per skill call: context, tokens, cost
lf runs --project parser        # one Project's Runs, filtered before the result cap
lf execs                        # one row per lf process
lf trace 66863649               # select an exec or trace; render its process tree
lf trace 66863649 --json        # inspect the same tree and its skill invocations
lf trace 66863649 --json --content --invocation <invocation> --turn <turn>
lf context --days 30 --repo "$PWD" --project context --task W2-71 --json
lf context --days 30 --repo "$PWD" --steered-only --current-revision-only --json
lf usage                        # subscriptions plus provider tokens, cache, and cost
lf usage --refresh              # poll every account's provider now
lf usage --json                 # fixed 5s, 5m, 1h, and 24h UsageSnapshot
lf ci --since 7d                # CI repair attempts, latency, and outcomes
lf ci --since 7d --json         # complete machine-wide incident receipt
lf ps                            # one live call-tree snapshot, ranked by completed output
lf ps --sort rate                # rank siblings by five-second live output
lf ps --json                     # versioned flat nodes with stable parent ids
lf top                           # refresh the same snapshot every two seconds on a TTY
lf top --json                    # emit once; redirected output also emits once without ANSI
lf prune --dry-run               # list stale receipts and registered orphan process groups
lf prune                         # remove those receipts and reap those process groups
lf doctor                       # audit continuity, identity, lineage, coverage, receipts
lf doctor --json                # machine-readable audit
```

`lf ls` reads the Wave registry. `lf status` focuses one Wave's operational
truth. `lf roadmap` overlays the current Linear-backed plan without creating a
second runtime model. `lf activity` orders durable Work creation, Run, Task PR,
and Steer facts; it reuses `WorkRef` identity and does not read reconstructable
Task or Project wake events.

`lf ps` and `lf top` show OS-live processes only. Exact PID/start-time receipts
attach `lf` processes to call records; exact ancestry attaches provider
processes. Completed calls and launches disappear. The embedded `UsageSnapshot`
uses provider receipts for 5-second, 5-minute, 1-hour, and 24-hour windows;
unattributed Loopflow Turns count globally but never leak into a Work rollup.
Exec rows fold their live descendants once. Missing measurements stay explicit
and elapsed time never implies death.

Both commands open the live Home ledger and ownership registry read-only. This
also applies under `scripts/dev-lf`: source builds can inspect real activity
without gaining migration or write authority over the installed database.
`lf prune` is the separate write boundary. It removes dead Exec receipts and
reaps only OpenCode process groups whose registered owner is absent. It never
kills unclaimed provider PIDs; inspect exact targets with `--dry-run` first.

```bash
lf -m codex --account manabot-eng@ : "fix the tests"   # prefer this login, then route
lf --account claude=jack@ --account codex=loopflow-eng@ implement
lf --only-account codex=manabot-eng@ review             # no fallback login
```

`--account <email-prefix>` prefers each matching managed login before its
provider's normal route. The first preferred attempt bypasses stored health;
a missing credential continues through the healthy fallback route.
`--only-account` restricts the invocation and its children to exactly the
selected provider accounts. Both flags are repeatable and accept
`claude=<selector>` or `codex=<selector>`. They cannot be combined.

Use the flags for Claude and Codex terminal sessions too (`--tui`): logging
into a managed login with a bare `codex login` creates a second session and
evicts the managed one ("needs re-login"); entering through lf shares one
session.

Without an account flag, managed Claude and Codex launches use the repository
route, then the default route. If neither exists, all automatic managed logins
are eligible and Loopflow skips known cooling or limited accounts. If no
managed login exists, the provider CLI uses its ambient default credentials.

`lf usage` leads with each managed account's subscription state — provider-
reported plan, session and weekly windows as percent *used*, reset times —
from stored observations (harness streams report them mid-run) topped up by a
live poll when older than 15 minutes. `--refresh` polls everything now;
`--cached` skips polling. A revoked credential shows the fix
(`lf auth connect <provider> <email>`), not a blank. The table below it keeps
provider input, cache reads, cache writes, inclusive output, reasoning, and cost
separate. Reasoning is already included in output and is never added twice.

Under forwarded account authority, subscription polling is unavailable so the
remote account store is never consulted. `lf usage` still prints provider usage
from the local execution ledger.

The repository's `telemetry-daily` operator flow combines the same accepted
per-Turn evidence with pre-land records under the Git common directory. Its
scorecard generator is deterministic internal code, not a general-user `lf`
API. Missing provider usage never becomes zero; an explicit provider-reported
zero remains a measured sample. Versioned policy lives in
`performance/budgets.json`, while generated reports remain runtime evidence.

A run is one agent-backed skill invocation. It owns the context, model, token,
cost, and outcome evidence. An exec is one `lf` process; nested execs share a
trace. `lf trace` accepts an exec or trace id and leaves killed processes open instead
of hiding them. `lf context` aggregates one filtered Invocation set without opening
bodies. Its Project and Task filters use captured control identity rather than
inferring ownership from a worktree path. The research-state flags require an
observed steer or a launch containing a current resolvable file-backed instruction
revision; missing revision identity does not match the current-only filter.
`lf trace --content` is the explicit
reader for the exact prompt and normalized conversation at one immutable
run/invocation/turn address.

`lf ci` reads durable CI incidents from the local Home store. One failed head is
one attempt; later passing and merge observations close every open attempt on
that PR. `--wave` and `--repo owner/repo` filter the same local report.

`lf doctor` also prints the binary's build provenance, the resolved database
path, and the latest known and applied migrations. Those fields still print
when the database is too new or came from a divergent development build.

The `capture` check keeps partial captures and unclaimed trace artifacts visible
as historical evidence. A loss stays red until a later complete capture starts
a 48-hour loss-free window; after that window it reports `capture recovered`
without rewriting the ledger or deleting traces. Any recurrence resets the
window and reports its UTC time, owner, provider or reason, and current `.lf`
storage context. `lf doctor` and `lf doctor --json` apply the same gate.
## Measuring Codebase Weight

```bash
lf tokens                       # lines and model tokens by tracked path
lf tokens --days 365            # daily history, grouped by file extension
lf tokens --json                # token-weighted tree for other tools
```

`lf tokens` counts with the same tokenizer used by the context budget. It skips
untracked and non-UTF-8 files; a symlink counts its tracked link text instead of
duplicating its target; history walks git blobs without checking them out.

## What's Included by Default

Every skill automatically includes:

| Context | Default | How to disable |
|---------|---------|----------------|
| **Agent doc** (AGENTS.md / CLAUDE.md / STYLE.md) | ✓ included | — |
| **Loopflow operating guidance** | ✓ included | `--no-loopflow` |
| **scratch/** | ✓ included | — |
| **wave/** | ✓ included | — |

## What's Opt-In

These require explicit flags or config:

| Context | How to enable |
|---------|---------------|
| **Docs** (files, globs, directories) | `--docs README.md,docs/` or `docs:` config |
| **Raw diff** (line-by-line changes) | `--diff` |
| **Branch files** (full changed file bodies) | `--diff-files` |
| **Clipboard** | `-c` / `--clipboard` |
| **Chrome automation** | `--chrome` |

See [Configuration](config.md) for setting defaults via config file.

## Examples

### Debug with clipboard

```bash
# Run tests, copy the error
lf debug -c
```

### Prefetch docs into context

```bash
lf qa --docs src/api/
```

Gathers `*.md` under `src/api/` into context before the prompt runs. Unlike
the old area scope, `--docs` only prefetches—it doesn't restrict which
files the agent touches.

### Use a different model

```bash
lf implement: add caching -m codex
```

### Apply a direction

```bash
lf gate -d ux
lf implement -d ux,clarity
```

### Disable loopflow operating guidance

```bash
lf gate --no-loopflow
```

`LOOPFLOW.md` carries loopflow-specific guidance for inline execution and
mechanical git/PR operations. Tier skills add scoped delegation. Use
`--no-loopflow` for a leaner prompt.

### Include clipboard content

```bash
lf debug -c    # include current clipboard text in the prompt
```

### Launch Claude, Codex, or OpenCode with a present human

```bash
lf design                 # direct TTY → uses session.launch (default: tui)
lf gate --tui             # force a terminal handoff for a normally-headless skill
lf : "fix the bug" --ide -m codex   # force the Codex app instead
```

`--tui` opens Claude, Codex, or OpenCode in the terminal. `--ide` opens Claude
or Codex in its app. Both override the repo default. Set `session.launch: ide`
in `.lf/config.yaml` to make the app the default for direct human-present
skills. Automated flow nodes and `--batch` remain headless.

### External skills

```bash
lf npx/vercel-labs/deep-research   # fetch + run from the npx skills catalog
lf npx/explain-code                # already-cached skill (no network)
```

`npx/` uses `.agents/skills/` in the current repo as a cache. Use `npx/<owner>/<repo>` when you know the package name; cached or searchable skills can often be run as `npx/<name>`. On a cache miss, Loopflow runs `npx skills add` first, then falls back to `npx skills find` when it needs a package hint. The core `task/` / `project/` / `wave/` / `ops/` catalogs are always available, and the legacy `rams/rams` alias still works when `~/.claude/commands/rams.md` is installed.

## PR Operations

The publish/submit/land contract every launched agent receives is
`rust/loopflow/src/engine/builtins/LOOPFLOW.md` — that file is canonical for
agent-facing semantics; this section is the human reference.

### lf pr publish

Push and create or refresh a PR, then print its state and URL. Opens no
browser — this is the headless publication command agents use.

```bash
lf pr publish
lf pr publish --title "area: short title" --body "## Summary ..."
lf -m codex pr publish        # one-off agent override for copy generation
```

When `-m` is omitted, copy generation uses `agent:` from `.lf/config.yaml` or
`~/.lf/config.yaml`. Use the `pr` ops skill to generate `--title`/`--body`
with agent judgment. When task gate has written cached PR copy, publication
consumes it and removes the gate-owned copy/review files before its first
commit or push. Other `scratch/` state remains untouched. Publication never
fetches to integrate, rebases, rewrites Task stack metadata, or launches
conflict recovery. A PR may remain behind its base until `lf rebase`, `lf gate`,
`lf pr submit`, or `lf pr land` owns integration. Push or GitHub failure returns
an error and presents nothing.

### lf pr open

Publish (same as `lf pr publish`), then open the PR for review — the GitHub
page in the browser. The explicit, human-initiated review action; agents use
`publish`, `submit`, or `land`. If launching the browser fails, only `open`
fails — the PR is already published and its URL printed.

### lf pr submit

Prepare the exact PR head, assign it to you, and stop for your merge click.
Nothing merges automatically.

```bash
lf pr submit
```

On Task PRs, submit records one User merge request containing the exact head
and Continue/Complete disposition. A later Task resume or head-changing
Loopflow operation clears it.

### lf pr land

Arm auto-merge. GitHub merges when required checks and repository rules pass.

```bash
lf pr land                    # land one PR; the Task stays open
lf pr land -c                 # land, then complete the owning Task
lf pr land --next parser-proof  # name the next serial Task PR
```

On Task PRs, land records the same head-and-disposition request with Auto as
the operator. `--match-head-commit` fences the arming command; Loopflow revokes
Auto before its own later head mutation. Concurrent Loopflow finalization and
push commands in one worktree are refused rather than interleaved.

Task publication persists a non-empty reviewer-facing title and body for the
current head. A published PR with missing or stale copy remains actionable, as
does a PR whose auto-merge settlement is not armed. Only a current-head Auto
merge request with Complete disposition records the terminal `lf pr land -c`
intent.

Task PR copy carries the Task contract before its reviewer-authored detail:

```markdown
LOO-249: Make Task PR copy explain intent and lifecycle

> [!NOTE]
> **Task:** [LOO-249 — Make Task PR copy explain intent and lifecycle](https://linear.app/...)
> **Task flow:** `incident` → `slice` → `ship-demo`
> **PR lifecycle:** Merging PR 1 completes the Task.
```

The exact identifier, name, provider link, pinned flows, PR sequence, and merge
disposition come from durable Task state. Generated or gate-authored prose adds
the evaluation path, importance, and implementation-specific scope without
repeating that context. Ordinary non-Task PR copy keeps its authored title and
body unchanged.

If a Task reaches `finally` after its work already merged and rotation left a
provably empty unpublished successor, `lf pr land -c` completes over the merged
PR without creating another one. Earlier lifecycle phases still refuse the
empty range.

Submit and land clear `scratch/`, preserve a recovery ref, collapse the
authored range to one tree-identical commit, replay that commit onto the pinned
target, verify it, and push once. Ordinary `lf rebase` keeps commit history.

### lf pr abandon

Close the PR, remove the worktree, delete the branch.

```bash
lf pr abandon feature-branch
lf pr abandon feature-branch --force   # skip confirmation, allow dirty
```

## lf commit

```bash
lf commit                     # stage all changes, generate a message, commit
lf commit -m "message"        # override the generated message
lf commit -p                  # commit and push
lf commit --no-add            # commit only what is already staged
```

## lf rebase

Plan or update the current branch against the right base.

```bash
lf rebase          # update the branch
lf rebase --plan   # show the strategy without changing git
lf rebase origin/main          # explicit target
```

Classifies the branch before mutating git: disposable branches can reset to
their base, authored work uses a normal rebase path. Clean updates stay
mechanical. A conflict keeps the first sequencer in place for one authorized
recovery agent; Loopflow verifies the pinned target, branch, dirty state, and
remote head before reporting success. If `scratch/` needs to survive a reset,
Loopflow stashes it under `.lf/tmp/scratch-stash/` and restores it afterward.
Loopflow records reviewed conflict resolutions with command-scoped rerere and
keeps auto-staging disabled. Repeating the same conflict reuses that resolution
mechanically and stages only its unmerged paths.

Keep conflict resolution local when the branch is too large or sensitive to
hand to another agent:

```bash
lf rebase --manual
# edit the conflict paths printed by lf
lf rebase --continue   # stages only the current conflict paths; repeat
lf rebase --abort      # restore the pre-rebase branch
```

Manual recovery stays local and never pushes. `--continue` and `--abort`
atomically adopt a stale Loopflow operation after its owner dies. A rebase
started with raw Git has no owner record, so name that destructive intent:

```bash
lf rebase --continue --adopt
lf rebase --abort --adopt
```

Plain `lf rebase` never adopts or aborts an existing Git operation.

## lf wt

Inspect, switch, and clean worktrees. Normal roadmap work starts with
`lf task run <issue-id>`; `lf wt` remains a low-level Git primitive. Place
dependent roadmap work through `lf task run CHILD --stack-on PARENT`, not
`lf wt`.

```bash
lf wt switch bugs             # by directory name, identity leaf, or full branch
lf wt list                    # worktrees as a tree; --format json
lf wt ci                      # CI status for the current branch
lf wt prune --dry-run         # show terminal or week-stale worktrees
lf wt prune                   # remove them and their local branches
```

`prune` never removes a worktree with uncommitted files. It removes clean
worktrees immediately when the remote branch is gone, the work landed, or the
current-head PR closed. It also removes a clean branch after seven days without
branch activity when no current-head PR is open. Main, the current worktree,
nonterminal Tasks, and worktrees owned by live processes remain protected.
Use `lf wt remove NAME --force` for an explicit destructive override.

`lfd` runs a lossless sweep on startup and every 15 minutes: only clean
landed, remotely deleted, or terminal Task worktrees are removed. Disable
with `autoprune: false` in config. Subscribe the daemon to GitHub merge and
branch-deletion webhooks by defining `LF_GITHUB_WEBHOOK_URL` and
`LF_GITHUB_WEBHOOK_SECRET` in Doppler; the secret travels over stdin and
never appears in process arguments or the service file.

## lf cron

Reconcile a Wave's `GOAL.md` schedules onto its placed macOS Home and inspect
each launchd firing through durable receipts.

```bash
lf cron preflight --wave infrastructure
lf cron sync --wave infrastructure
lf cron list --wave infrastructure --json
lf cron trigger --wave infrastructure --flow telemetry-daily --wait --timeout 15m
lf cron history --wave infrastructure --days 35
```

`preflight` proves the installed release binary, Wave placement, authoritative
checkout, target catalog, and fixed-daily schedules without changing launchd.
`sync` repeats those checks before changing
launchd, refuses a Home that does not own the Wave placement, and prunes jobs
removed from the declaration. Jobs execute through the installed release `lf`
with a secret-free host environment. Each firing writes a running receipt
before the target starts and atomically replaces it with `succeeded` or
`failed`; an interrupted runner remains visibly stale. Logs stay under
`<repo>/.lf/logs/`, while receipts survive checkout replacement under
`<LF_HOME>/cron/receipts/`.

`trigger` asks launchd to fire the installed job; it never bypasses the
configured path. `history` defaults to 35 days so nightly, weekly, credential,
and host-drift observation windows share one evidence surface.

## lf pm

Read and edit a wave's Linear planning state. Each wave is backed by one
Linear Initiative, projects are Linear Projects, tasks are Issues. `sync`
refreshes the local SQLite read model used by every other read surface.

```bash
lf pm status                                # linked waves and task counts
lf pm init --wave designer --team-key DSG   # connect Wave; establish repo Team once
lf pm sync --wave designer                  # refresh SQLite from Linear
lf pm sync --plan                           # report drift without writing
lf pm show --wave designer                  # read; refresh when stale
lf pm show --wave designer --no-sync        # cache-only agent/app read
lf pm show --wave designer --project ui     # filter to one project
lf pm project create --wave designer --title "..." --definition "..." \
  --first task-design --loop slice --finally ship-demo --kr "..."
lf pm project update --wave designer --project ui --first incident \
  --loop ship-5whys --finally ship-demo
lf pm project archive --wave designer --project retired-bet
lf pm task create --wave designer --project ui --title "Dark mode"
lf pm task update --id 1207... --title "Refine dark mode"
lf pm task done --id 1207... --pr "https://github.com/acme/app/pull/42"
lf pm task move --id 1207... --wave designer --project api
lf pm rename --wave designer --title "Designer"   # rename the Initiative
lf pm reteam                            # dry-run the repository-wide Team migration
lf pm reteam --apply                    # migrate when no Task Run can write old ids
lf pm doctor                            # flag ownership and title drift
```

Connect Linear first with `lf auth linear`. `lf pm init` pins the Initiative
into `GOAL.md` and the repository Team into `.lf/config.yaml`. Every Wave in
that repository reuses the Team and Task prefix (`LOO-1`, `LOO-2`); Initiatives
and Project membership decide which Wave owns a Task. `pm init --all` discovers
nested `GOAL.md` files recursively and initializes them against the same Team.
When no Initiative is pinned, init links one exact title match, creates one
when absent, and fails on duplicates. Creation fails closed unless the
repository Team and its Git-origin claim both validate.

```yaml
# .lf/config.yaml
pm:
  provider: linear
  linear_team: "stable-team-uuid"
```

Linear's Projects view is flat, so provider titles use
`<canonical Wave path> — <Project>`; nested Waves remain legible as
`Survival / Infrastructure — Gmail`. Loopflow resolves ownership from stable
Initiative and Project ids, then strips that presentation prefix and keeps the
canonical slug. `show` serves
snapshots younger than an hour without a network request, tries a
five-second refresh for older ones, and refuses to silently serve a snapshot
older than a week. Use `--no-sync` in agents and UI paths so rendering never
waits on Linear.

`lf pm reteam` migrates every linked Wave onto the repository Team. It
**defaults to a dry run** and only mutates with `--apply`; it defers an issue
while a Task Run can still write its old identifier. Completed issues move too.
Loopflow first attaches the destination Team to every Project, comments and
moves Issues by UUID, narrows Projects to exactly that Team, repairs Wave-path
titles, verifies every association, refreshes every snapshot, and only then
removes legacy Wave Team fields. Interrupted runs keep a legacy sentinel and
resume without duplicating comments or moves.

## lf release

Mechanical release subcommands; `lf release run` is the full workflow.

```bash
lf release run patch          # full release workflow
lf release check              # exact commits in the target range
lf release notes 1.2.3        # narrative notes from decisions + commits + PRs
lf release bump 1.2.3         # bump manifests
lf release tag 1.2.3          # create + push git tag
lf release publish v1.2.3 --notes RELEASE_NOTES.md --asset dist/lf.tar.gz
lf release publish v1.2.3 --finalize
lf release status             # workflow + GitHub Release status
```

`release.targets.<name>.publisher` is an argv list for the credentialed host
publisher. `lf release run` appends `check` before changing release state, then
downloads the successful hosted build and invokes it with `publish --tag ...
--artifacts ...` from an exact-tag worktree. No merged changes is a successful
no-op. An incomplete latest tag resumes; it never cuts a newer tag around a
failed publication. Use `{repo}` in a publisher argument to name the current
synchronized repository; `LF_RELEASE_SOURCE_REPO` names the leased exact-tag
worktree during publication.

| Path | What it holds |
|------|--------------|
| `release/unreleased/DECISIONS.md` | Append-only ledger of release-worthy decisions during the current cycle |
| `release/vX.Y.Z/DECISIONS.md` | Archived decision ledger for a shipped version |
| `release/vX.Y.Z/NOTES.md` | Snapshot of that version's release notes |
| `RELEASE_NOTES.md` | Always-latest release notes at the repo root |

Interactive runs append durable product and process decisions to the
unreleased ledger; headless runs do not. The release workflow promotes
`release/unreleased/` to `release/v<version>/`, uses `DECISIONS.md` as the
intent source, the exact git range as shipped-behavior truth, and merged PRs as
narrative context, then archives the generated notes. If the ledger is absent,
notes fall back to commits and PR history. Headless release automation needs no
healthy notes provider. Missing CLIs, cooldowns, rate limits, quota or
authentication failures, and provider outages write deterministic notes from
bounded context. Unknown skill failures and missing, stale-version, or
oversized output keep the release gate red. `lf release status` reports note
quality and gate safety separately from workflow and GitHub Release completion.
Configure repository-specific verification, preparation, and completion
evidence under `release.targets`; see [Configuration](config.md).

## See Also

[The Agent API](agent-api.md) · [Conducting](conducting.md) · [Authoring](authoring.md) · [Get Started](getting-started.md) · [Configuration](config.md)
