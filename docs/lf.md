# lf Command Reference

One binary, three audiences. `lf` launches prompts for humans, gives agents the
verbs to run and steer other agents, and reads the executing Home's planning,
process, journal, and Run evidence. Prefix a command with `lf ssh <home-id>` to
run the same local operation on another Home.

| You are | Start with | Deep dive |
|---|---|---|
| A human running prompts | [Basic Usage](#basic-usage), [Context Flags](#context-flags) | [Get Started](getting-started.md) |
| A human operating waves | [Running Waves, Projects, and Tasks](#running-waves-projects-and-tasks), [Speaking to Waves](#speaking-to-waves) | [Waves](waves.md) |
| An agent driving other agents | [Running Waves, Projects, and Tasks](#running-waves-projects-and-tasks) | [The Agent API](agent-api.md) |
| Watching the whole machine | [Reading This Home](#reading-this-home) | [Conducting](conducting.md) |

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
lf task prepare DES-123           # tracked Work + worktree, no execution
lf project prepare runtime-model  # tracked Project Work, no execution
lf --task DES-123 research \
  "Map runtime behavior; write scratch/research-runtime.md"
lf --project context project/operate \
  "Reconcile the KRs with current evidence"
lf task restart DES-123 "Reconcile the completed research"
lf task run DES-123 --directive "fix the flaky test" # keep one Task through merge
lf task run DES-124 --stack-on DES-123                # dependent Task, separate worktree
```

`--task`, `--project`, and `--wave` run one named skill about existing Work
without advancing its Flow position. The most specific selector is the
Run subject: Task implies Project and Wave; Project implies Wave. Broader
selectors may qualify it and must match. Task binding supplies the Task seed,
uses its existing worktree, and preloads the complete recursive scratch
Markdown snapshot. Project and Wave binding use the owning Wave repository
because Projects do not own worktrees. Several Runs may concern the same Work
concurrently; each keeps a distinct Run id and none reserves the Work. Bound
direct skills leave edits uncommitted; give parallel contributions distinct
paths, reconcile the shared tree, then checkpoint one coherent result with `lf
commit` or aggregate it deliberately with `lf task restart`. Parent Runs use
this same path without becoming the Task worker or taking its claim.

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
Ownership uses `/` (`wave/operate`); words within one name use `-`
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
| `project/operate` | Judge KR evidence and launch the next useful Task in one turn |
| `project/start-chapter` / `project/review-chapter` | One bet's chapter: propose definition, KRs, and Task dispositions / verdict every KR on dated evidence |
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
| `wave/operate` | Read, decide, and take the one or two useful Wave moves in one turn |
| `review-open-work` | Survey branches, PRs, worktrees, and waves for inbox-zero triage |
| `update-wave` / `split-wave` | Maintain Wave structure and memory |
| `wave/start-chapter` / `wave/review-chapter` | One Wave's chapter: propose its Project portfolio / report every KR verdict |
| `s2-scan` / `s2-assess` | Coordination: backlogs, PR/path overlap, conflict risk and safe ordering |
| `s3-scan` / `s3-assess` | Control: live health, velocity, CI, retries, worker-pool size |
| `s4-scan` / `s4-assess` | Intelligence: dependencies, advisories, upstream APIs, what they imply |
| `s5-scan` / `s5-assess` | Identity: wave roster, policy, boundary and autonomy drift |

Ops skills — raw prompt logic around mechanical git, PR, and release commands:

| Skill | What it does |
|------|--------------|
| `init` | Connect the repo to Homes, accounts, Waves, and task execution |
| `start-chapter` / `review-chapter` | Open a new planning chapter with the human / close the old chapter's evidence record |
| `loopflow-validate` | Validate flows and skills |
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
| `-w, --wave NAME` | Select Wave Work, or qualify selected Project or Task Work. |
| `--project SELECTOR` | Select Project Work, or qualify selected Task Work. |
| `--task ISSUE` | Select Task Work. |
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
| `--max-turns N` | Cap agent turns for this launch |

## Model Flags

| Flag | Description |
|------|-------------|
| `-m, --model MODEL` | Choose model (e.g., `claude:opus`, `codex`, `opencode`) |

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
lf project prepare <linear-project-id>              # Project Work, no execution
lf task prepare DES-123                             # Task Work + worktree, no execution
lf task start <linear-project-id> "fix the flaky chord-timeout test"
pbpaste | lf task start incident-management
lf task run DES-123 --directive "fix the parser before the docs"
lf task run DES-124 --stack-on DES-123
lf task run DES-125 --flow incident
lf task status DES-123
lf --as project:proj_... : "Which KR owns this?"      # ordinary agent perspective
lf ask "Review this proof with me"                    # block on a human session
lf session list --json                                # unresolved human Sessions
lf session open task_...:flow:node:0 --json           # exact native provider resume
lf session complete <ask-id>                          # finish an ad-hoc Ask
lf session approve <flowstep-id> "Verified"           # approve a Task FlowStep
lf session iterate <flowstep-id> "Narrow the design"
lf task steer DES-123 "rename the flag"
lf task steer DES-123 "take the smaller approach"
lf task interrupt DES-123                             # end the active turn
lf task wait DES-123
lf task resume DES-123 --reason "provider credentials repaired"
lf task restart DES-123 "Reconcile the new runtime research"
lf work status task task_... --json                  # stable Work projection
lf work interrupt task task_...                      # refuses without exact process ownership
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
development. `lf project run` launches one finite `project/operate` Run without
a Project worktree or resident process. It starts with stored Project context
and reads the available planning evidence during the operation. A failed PM
read does not prevent the skill from continuing with its known KRs.
Project and Task Work have stable identities and small state:
`ready`, `done`, or `abandoned`. Process liveness, Task condition, Sessions,
PR state, Flow position, and Run evidence stay separate. `task prepare` ensures the
Project and Task Work records, one stable Task worktree, and its first serial PR
identity without starting execution. `task run` uses that same substrate,
selects a Flow when none is active, and ensures the exact next boundary has one
Task worker. Each autonomous boundary starts a fresh provider Run from durable
Task facts; no provider transcript or resident Task process is required.
`resume` preserves the Work, selected Flow, Steers, worktree, branch, and PR
while starting a fresh worker.
`steer` posts a Linear Task comment and never starts a worker. Direct Linear
comments enter the same delivery path. Only Task advancers consume steering;
independent `--task` or `--as` Runs do not. `task run` selects the Flow when
execution should begin. Project and Wave guidance is extra input to their
operate skills.
Wave names are repository-scoped. Relocation requires the UUID because the
repository and name may both change; it preserves authored Wave files, journal,
PM binding, Work state, and Home placement. Home-local Run records remain on
the Home that recorded them and are never rewritten as control state.
Relocation refuses a live Wave or claimed Task worker and never keeps an
old-name alias. UUID-addressed `lf work` reads and mutations also verify that
the selected Work belongs to the invoking repository; a UUID from another
repository is not a capability.

A Linear Project may recommend one Task Flow. `lf task run --flow <name>`
overrides that recommendation when it selects the next worker's Flow. The
selected Flow definition is persisted immutably for that invocation, so edits
to repository Flow YAML cannot change a Task already in progress. When the
Flow completes, Loopflow deletes its position and leaves the Task ready. The
next worker chooses afresh; PR delivery and Task completion remain explicit
commands rather than an implied next lifecycle phase.

Every Task-owned PR keeps its authored title, naming the benefit of that PR.
Loopflow places the canonical Task name and Linear link after the opening
summary and refreshes merge consequences from durable state on publication,
submission, and landing. If the cached PM snapshot has no provider URL, run
`lf pm sync --wave <wave>` before publishing.

Task launch also resolves the execution boundary the work needs: the assigned
worktree, Loopflow's pinned planning store, and network access for delivery.
Headless Tasks require a managed Codex or Claude account with usable
credentials. The generic harness publishes one Home-local Run manifest before
the provider starts; the bundle records the launch but does not reserve Task
Work or grant mutation authority.

If a provider returns normally after a permission, control-authority, or
network command failure, Loopflow records the exact command blocker as a
non-resumable Task failure. Status assigns the next move to the User with
`no_action`; missing-process reconciliation and automatic recovery do not
replace or repeat it. Correct the capability, then run
`lf task resume ID --reason "<what changed>"` to create a fresh input boundary.

Worktree safety comes from short OS-held mutation locks and prepared Git state,
not Run identity. Commit, restart checkpointing, rebase, and land serialize
their exact critical sections and refuse a conflicting prepared state. An
active rebase additionally owns one exact operation id so its recovery child
can continue that sequencer without authorizing any other Git mutation. A
surviving unclaimed provider PID has no mutation or signal authority.

A skill that needs another Work's perspective launches an ordinary Run directly:

```bash
lf --batch --as project:<id> : "Which proof matters?"
```

A headless Run that needs human judgment runs `lf ask "<request>"`. Loopflow
starts an ordinary TUI Run in the caller's exact checkout and waits. The session
agent calls `lf session ready "<summary>"` when its work is ready; this does not
complete, hide, or release anything. The human runs `lf session complete
<session-id>` when the conversation is finished. Completion stops the exact
provider client, removes the Session from the Sessions list, and resumes the caller
with the ready summary and any filesystem changes.

A Task human FlowStep uses the same surface. It persists its exact playhead and
starts one ordinary provider Run for its authored Skill:

```bash
lf session list --json                              # unresolved human Sessions
lf session open <session-id> --json                 # prepare/recover attachment
lf session ready "Ready for review"                 # agent state; stays visible
lf session approve <session-id> "Verified summary"  # approve the FlowStep
lf session iterate <session-id> "Narrow the design"
```

The FlowStep session runs `lf --tui --as task:<id> <skill>`. Closing, provider
exit, or agent readiness never means approval; the persisted Task playhead
remains waiting. Loopflow.app lists the same sessions and exposes the FlowStep
decision controls. Selecting a Session already open in the app returns to its
live terminal. If its client is active elsewhere, **Move here** explicitly
stops that client and resumes provider-native history in the selected pane.
Closing a pane keeps the live terminal available in the Sessions list; Complete
ends the Session.

Human sessions may use a detached PTY cradle to let the first provider client
start before the desktop is present. That cradle is not Session identity,
readiness storage, liveness authority, or the attachment surface. The boundary
record owns resolution; the ordinary Run owns provider identity and history.

`--stack-on` places a new Task worktree on another Task's published PR. Its PR
targets that parent branch automatically, then collapses onto `main` after the
parent merges. The two Tasks keep separate identities, worktrees, and workers.
Tmux remains process containment, not product identity or advancement authority.

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

## Speaking to Waves

The **thread** is the human surface: durable, replayed, and owned by a running
Wave. Typed Work observations carry Project and Task progress to their parent.

```bash
lf chat "ship the button audit first"       # post into the current wave's thread
lf chat -w infra "CI is red on the PR"      # target a wave by name
lf chat --parent "blocked on schema change" # escalate to the parent wave
lf chat --follow -w intelligence            # watch and speak from one terminal pane
lf chat --history --json -w intelligence    # read the saved tail while stopped
lf reply intelligence "Should this get an answer?"  # one reply decision, no listener
```

| Command | What it does |
|---------|--------------|
| `lf chat [TEXT]` | Post into a wave's thread; `--follow` replays the latest 12 turns and continues live while typed lines post, `/status` reads health, and `/quit` leaves. `--history --json` reads the same bounded tail directly from the journal without a listener. Commands, tools, and loop bookkeeping stay out of chat; turn failures remain visible. Without `--follow`, omitted TEXT reads stdin. Outside any wave, one-shot chat prints a short drop note and exits 0 |
| `lf reply WAVE [TEXT]` | Run the Wave's chat-reply capability once and print only a warranted reply. Reads stdin when TEXT is omitted; `--agent` selects a provider and `--max-turns` bounds it. It starts no listener, resident, or governance pass. |

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

## Reading This Home

```bash
lf ls --json                    # every durable Wave and its Home/runtime evidence
lf status <wave> --json         # Work, Runs, conditions, and live metric_portfolio
lf roadmap --json               # current plan plus that portfolio on every Wave
lf activity                     # durable Work changes, newest first
lf activity --task INF-123 --json # filter before the bounded typed snapshot
lf runs                         # recent Home-local Run records
lf runs --project parser        # one Project's Runs, filtered before the result cap
lf runs --parent run_ab12 --json # every direct child Run, uncapped
lf runs run_ab12                 # inspect one Run by unambiguous prefix
lf runs run_ab12 --final         # print the last durable provider conclusion
lf runs run_ab12 --events        # print its event stream verbatim
lf usage --project parser        # direct Run usage for one Project
lf usage --task INF-123 --json   # direct Run evidence for one Task
lf session list                  # interactive, Ask, and FlowStep sessions
lf session open run_ab12         # continue a closed native provider session
lf session open run_ab12 --try   # let the provider arbitrate an active session
lf session open run_ab12 --replace # stop Loopflow's client, then continue here
lf session open run_ab12 --json --replace # prepare a takeover command without stopping it yet
lf session complete run_ab12     # finish it; provider history remains resumable
lf replay run_ab12               # launch that request as a child Run
lf usage --days 30              # direct provider-authored usage per Run
lf usage --days 0 --json        # all RunSnapshot rows; zero means all time
lf ci --since 7d                # CI repair attempts, latency, and outcomes
lf ci --since 7d --json         # complete machine-wide incident receipt
lf ps                            # one OS-live process and call-tree snapshot
lf ps --json                     # versioned flat nodes with stable parent ids
lf top                           # refresh the same snapshot every two seconds on a TTY
lf top --json                    # emit once; redirected output also emits once without ANSI
lf prune --dry-run               # list stale receipts and registered orphan process groups
lf prune                         # remove those receipts and reap those process groups
lf doctor                       # audit continuity, identity, lineage, coverage, receipts
lf doctor --json                # machine-readable audit
```

`lf ls` reads the local Wave registry. `lf status` focuses one Wave's local
planning and runtime projection. `lf roadmap` overlays the current
Linear-backed plan without creating a second runtime model. `lf activity`
orders durable Work creation, Run, Task PR, and Steer facts; it reuses
`WorkRef` identity and does not read reconstructable Task or Project wake
events. `lf runs`, `lf replay`, and `lf usage` scan `$LF_HOME/runs/` directly.
The `--parent` drill resolves one exact Run and returns all direct children
without the seven-day presentation cap. The one-Run `--final` read projects the
last durable provider conclusion from normalized conversation events. Records
without a phase receipt are labeled and expose streamed prose from their last
completed provider turn. It does not parse vendor output or invent a conclusion
for an unsettled Run.
Replay uses the immutable prompt, agent/model, non-secret provider account ID,
and tool boundary recorded before spawn; it never reconstructs those inputs
from current planning or prompt configuration. Managed Claude/Codex replay
resolves that account ID through the current Home's deterministic credential
directory or an explicit forwarded lease, without opening planning SQLite.
None of these commands silently queries or aggregates another Home.

`lf ps` and `lf top` show OS-live processes only. Exact PID/start-time receipts
attach `lf` processes to call records; exact ancestry attaches provider
processes. Completed calls and launches disappear. Unclaimed providers remain
separate because Loopflow has no exact authority to attach or signal them.
Elapsed time never implies death.

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
`--only-account` restricts the launch and its children to exactly the
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

`lf usage` scans the same Home-local bundles as `lf runs`, ordered newest first.
`--days` filters by Run start time and defaults to 30; zero selects all recorded
Runs. `--wave`, `--project`, and `--task` apply the same Work attribution drill
as `lf runs`; the table names the most specific Work on each row. JSON remains
the filtered direct `RunSnapshot` array. Each row preserves provider-authored
cumulative counters once per usage stream. Omitted counters stay unknown,
provider final receipts are counted explicitly, and evidence gaps remain visible.
Run settlement never invents provider finality.

A Run is one Home-local harness bundle. Its immutable manifest names launch
context and causal parent; append-only event and conversation streams retain
direct evidence; an exclusive terminal receipt settles it once. Provider
retry/failover remains inside that Run as distinct attempts and usage streams.
No owner receipt means no process signal authority.

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

The publish/submit/arm/land contract every launched agent receives is
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
`lf pr submit`, `lf pr arm`, or `lf pr land` owns integration. Push or GitHub failure returns
an error and presents nothing.

### lf pr open

Publish (same as `lf pr publish`), then open the PR for review — the GitHub
page in the browser. The explicit, human-initiated review action; agents use
`publish`, `submit`, `arm`, or `land`. If launching the browser fails, only `open`
fails — the PR is already published and its URL printed.

### lf pr submit

Prepare the exact PR head, assign it to you, and stop for your merge click.
Nothing merges automatically. Task and non-Task branches use the same command.

```bash
lf pr submit
```

Inside a managed Task worktree, `submit` records a user-owned exact-head merge
request in the Task PR state. It does not advance or consult the Task's Flow
position. Use `-c` to complete the Task after merge or `--next <slug>` to
rotate its serial PR chain.

### lf pr arm

Prepare the exact PR head, request auto-merge, and return without watching.

```bash
lf pr arm
lf pr arm -c
lf pr arm --next parser-proof
```

Task disposition is recorded for the exact armed head, but completion and
rotation wait for a later authoritative merged observation.

### lf pr land

Prepare and arm the PR, then watch GitHub until merged or actionably blocked.
Failing required checks launch one bounded `ci-fix` agent for the exact failed
head. That agent rebases first, repairs and verifies, then pushes and enables
auto-merge with the original Task disposition. The watcher observes the new
head and completes after merge; it does not publish or re-arm repairs.

```bash
lf pr land                    # land one PR; the Task stays open
lf pr land -c                 # land, then complete the owning Task
lf pr land --next parser-proof  # name the next serial Task PR
```

On Task PRs, arm and land record the same head-and-disposition request with Auto
as the operator. `--match-head-commit` fences the arming command; Loopflow
revokes Auto before its own later head mutation. Land applies completion or
rotation only after GitHub reports the PR merged. Concurrent Loopflow
finalization and push commands in one worktree are refused rather than
interleaved.

Task publication persists a non-empty reviewer-facing title and body for the
current head. A published PR with missing or stale copy remains actionable, as
does a PR whose auto-merge settlement is not armed. Only a current-head Auto
merge request with Complete disposition records the terminal `lf pr land -c`
intent.

Task PR copy leads with the benefit of its own change:

```markdown
Understand what merging this PR will do

Reviewers can see whether merging this change completes the Task or leaves
follow-up work, without reconstructing its execution history.

> [!NOTE]
> **Task:** [Make Task PR copy explain intent and lifecycle · LOO-249](https://linear.app/...)
> **PR lifecycle:** Merging PR 1 completes the Task.

## Try it

Read the opening summary, then follow the Task link in the note below it.
You should be able to distinguish this PR's change from the broader Task and
see whether merging completes the Task or leaves it open.
```

The opening summary is the first Markdown paragraph. Loopflow inserts its
managed Task block after that paragraph, preserving the authored title and
remaining body. Refresh replaces the block instead of accumulating history.
The exact Task identifier, name, provider link, PR sequence, and merge
consequence come from durable delivery state. Publication alone does not
request Task settlement. Refreshing the same head preserves and describes an
existing merge request; publishing a changed head clears superseded intent.
Task lifecycle phase is intentionally absent.
Generated or gate-authored prose describes this increment's meaningful changes
and puts a useful **Try it** walkthrough last. Test and lint evidence belongs
in **Checks** or CI, never in the walkthrough. Ordinary non-Task PR copy is unchanged.

If a Task's work already merged and rotation left a provably empty unpublished
successor, `lf pr land -c` completes over the merged PR without creating
another one. This is a delivery-state decision and needs no Flow-position claim.

Submit, arm, and land clear `scratch/`, preserve a recovery ref, collapse the
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

Every invocation fetches current upstream and updates the local default branch,
including when called from a sibling worktree. Main fast-forwards when possible;
divergent unpublished commits are retained through a merge. Main is never pushed.
The caller then integrates that updated local main (or its explicit/stacked
target). An already-current main does not skip a behind caller's rebase.
Staged, unstaged and untracked edits are saved and restored. If restoration
conflicts, the error names the retained stash. `--plan` stays read-only and
describes locally known refs without fetching.

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

## lf install

```bash
lf install             # install the latest published Loopflow from any directory
lf install schedule    # update Loopflow at login and weekly (macOS)
lf install schedule daily  # weekly, daily, hourly, or 5min
```

Updates the installed CLI, daemon, and macOS application through verified
release downloads and the existing promotion transaction. Requires no Git
repository, source checkout, Python, uv, or Homebrew. An already-current,
complete release skips asset downloads; missing or stale artifacts are repaired.
Use `lf rebase` for checkout updates and your project's own tools for dependency
setup.

For an older `lf` whose install command requires a source checkout, upgrade once
with the external installer, then use `lf install` for subsequent updates:

```bash
curl -fsSL https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh | sh
```

The external installer verifies release assets and enters the same promotion
transaction. The old `scripts/install.py refresh` and `pull-local-bin.sh` entrypoints
have been removed.

The scheduled job invokes the installed `lf install`, with stable tool paths and
logs at `~/Library/Logs/Loopflow/refresh.log`. It runs at login and on the selected
cadence: weekly on Monday at 09:00 (the default), daily at 09:00, hourly on the
hour, or every five minutes on clock multiples of five. All times are local;
launchd coalesces sleeping calendar intervals into one run at wake. Rerun
`lf install schedule` to update an existing job and remove its old source-checkout
dependency. Failed downloads or promotion remain nonzero and can be retried.
Linux supports `lf install`; automatic scheduling currently requires macOS.

`lf list`, authentication, profiles, Home identity, and machine inspection also
work outside repositories. `lf route show` displays defaults there;
`lf route set --repo owner/name` selects a repository explicitly. Inside a
repository, catalog and listing commands use its context. Outside, `lf ls`,
`lf roadmap`, and `lf session list` show machine-wide records.

## lf wt

Inspect, switch, and clean worktrees. Normal roadmap work starts with
`lf task run <issue-id>`; `lf wt` remains a low-level Git primitive. Place
dependent roadmap work through `lf task run CHILD --stack-on PARENT`, not
`lf wt`.

```bash
lf wt create next             # refresh main, then create a sibling from it
lf wt create next --plan      # preview placement without fetching or writing
lf wt switch bugs             # by directory name, identity leaf, or full branch
lf wt list                    # worktrees as a tree; --format json
lf wt list --sync             # refresh main before listing
lf wt ci                      # CI status for the current branch
lf wt prune --dry-run         # show terminal or week-stale worktrees
lf wt prune                   # remove them and their local branches
```

`create`, `list --sync`, and `prune` fetch and integrate current upstream into
main, preserving unpublished commits and local edits. Refresh failures stop
the command. `create --plan` and `prune --dry-run` preview without fetching or
updating main; they do not establish upstream freshness.

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
lf cron trigger --wave infrastructure --flow <flow> --wait --timeout 15m
lf cron history --wave infrastructure --days 35
```

`preflight` proves the installed release binary, Wave placement, authoritative
checkout, target catalog, and fixed-daily schedules without changing launchd.
`sync` repeats those checks before changing
launchd, refuses a Home that does not own the Wave placement, and prunes jobs
removed from the declaration. It preserves each unchanged job's activation
timestamp; changing its Home, schedule, target, or identity starts a new
obligation. Jobs execute through the installed release `lf` with a secret-free
host environment. Each firing writes a running receipt before the target starts
and atomically replaces it with `succeeded` or `failed`; an interrupted runner
remains visibly stale. Logs stay under
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

Connect Linear first with `doppler run -- lf auth linear`. `lf pm init` pins the Initiative
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

Fresh PM operations renew expiring Linear credentials automatically and store
the rotated access/refresh pair together. Temporary endpoint failures get one
retry within the read deadline; `--sync` reports failure if it cannot obtain a
fresh snapshot. Retry a temporary failure with `lf pm sync --wave <wave>`.
Reconnect with `doppler run -- lf auth linear` only when the error identifies a
missing credential or unusable refresh grant/client configuration. A timeout
during persistence can leave its outcome pending; the next read checks the
stored credential before attempting another exchange.

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
publisher. `lf release run` invokes that command with `check`, then with
`prepare --tag ... --artifacts ... --output ...` before tagging, and finally
with `publish --tag ... --artifacts ...` after tagging. The candidate phase
builds the merged commit under a disposable ref, validates the installer and
migration authority, notarizes the DMG, and records the exact artifact hashes.
Only that prepared candidate receives the immutable version tag. Publication
consumes the prepared bytes from an exact-tag worktree. No merged changes is a
successful no-op. An incomplete latest tag resumes; it never cuts a newer tag
around a failed publication. Use `{repo}` in a publisher argument to name the
current synchronized repository; `LF_RELEASE_SOURCE_REPO` names the leased
candidate or exact-tag worktree.

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
