# lf Command Reference

One binary, three audiences. `lf` launches prompts for you, gives agents the
verbs to run and steer other agents, and reads the executing Home's planning,
process, journal, and Run evidence. Prefix a command with `lf ssh <home-id>` to
run the same local operation on another Home.

| You are | Start with | Deep dive |
|---|---|---|
| Running prompts | [Basic Usage](#basic-usage), [Context Flags](#context-flags) | [Get Started](getting-started.md) |
| Operating waves | [Running Waves and Tasks](#running-waves-and-tasks), [Speaking to Waves](#speaking-to-waves) | [Waves](waves.md) |
| An agent driving other agents | [Running Waves and Tasks](#running-waves-and-tasks) | [The Agent API](agent-api.md) |
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
lf --task DES-123 design "Revise the discovery design"
lf --task DES-123 code "Implement the accepted slice"
lf --task DES-123 research \
  "Map runtime behavior; write scratch/research-runtime.md"
lf --wave context wave/operate \
  "Reconcile the KRs with current evidence"
lf task restart DES-123 "Reconcile the completed research"
lf task run DES-123 --directive "fix the flaky test" # keep one Task through merge
lf task run DES-124 --stack-on DES-123                # dependent Task, separate worktree
```

`--task` and `--wave` run a named skill, Flow or inline prompt about existing Work
without advancing its managed Flow position. A direct bound Flow has its own
invocation; attribution does not claim the Task worker's cursor. The most
specific selector is the Run subject: a Task implies its Wave. Broader
selectors may qualify it and must match. Task binding supplies the Task seed,
uses its existing worktree, and preloads the complete recursive scratch
Markdown snapshot. Wave binding uses its repository. Several Runs may concern the same Work
concurrently; each keeps a distinct Run id and none reserves the Work. Bound
direct skills and flows leave edits uncommitted; give parallel contributions distinct
paths, reconcile the shared tree, then checkpoint one coherent result with `lf
commit` or aggregate it deliberately with `lf task restart`. Parent Runs use
this same path without becoming the Task worker or taking its claim.
A direct flow creates a fresh Run for each skill, with the same Work subject and
an updated scratch snapshot. Explicit flow operations still execute as authored.
Use `lf task run DES-123 --flow code` to bind and pursue the managed Task workflow.

Bare names prefer skills when a skill and flow share a name. Select explicitly
with `lf skill launch-plan` or `lf flow launch-plan`. `design` and `ship-5whys`
are skills; they need no single-step flow wrapper.

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
4. Core built-in skills, grouped by Task, Wave, and Ops (`lf list` shows the live catalog)
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
| [`concept-review`](concept-review.md) | Rewrite the intended usage, then simplify product concepts, core types, and APIs together or after review-slice |
| `loop-decide` | Assess a pass's progress and evidence to choose Advance, Iterate, or human help |
| `unblock` | Resolve stalled work with the human inside an Ask Session, using concept-review by default |
| `demo` | Walk the User through the changed behavior, or prove it headlessly and ask one exact blocking question |
| `review-design` | Reshape AI-elaborated design into user intent |
| `refine` | Refine existing work |
| `task/clarify` / `task/pursue` / `task/mutate` | Clarify, implement, and judge one durable Task |

Planning skills — shape and pursue the current chapter:

| Skill | What it does |
|------|--------------|
| `wave/operate` | Judge KR evidence and launch the next useful Task in one turn |
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
| `wave/start-chapter` / `wave/review-chapter` | One Wave's chapter: propose its fresh plan / report every KR verdict |
| `s2-scan` / `s2-assess` | Coordination: backlogs, PR/path overlap, conflict risk and safe ordering |
| `s3-scan` / `s3-assess` | Control: live health, velocity, CI, retries, worker-pool size |
| `s4-scan` / `s4-assess` | Intelligence: dependencies, advisories, upstream APIs, what they imply |
| `s5-scan` / `s5-assess` | Identity: wave roster, policy, boundary and autonomy drift |

Ops skills — raw prompt logic around mechanical git, PR, and release commands:

| Skill | What it does |
|------|--------------|
| `init` | Connect the repo to Homes, accounts, Waves, and task execution |
| `start-chapter` / `review-chapter` | Open a new planning chapter with the user / close the old chapter's evidence record |
| `loopflow-validate` | Validate flows and skills |
| `commit-message` | Generate a commit message without committing |
| `rebase-conflicts` | Resolve conflicts after the mechanical rebase stops |
| `pr-message` | Generate a PR title and body without publishing |
| `pr-publish` | Generate PR copy and call `lf pr publish` |
| `pr-submit` | Prepare a PR for a person to land |
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
lf wave --wave designer sync                           # normalized onto `sync`
lf task --wave designer create --title "Fix it" --notes "Repair startup"  # normalized onto `create`
lf commit -m "explain the change"                   # -m remains commit-local
```

Flags may cross nested subcommands to reach a selected command that owns the
spelling. If more than one level owns it, a flag already valid at its current
level stays there. Put `--` before literal arguments that look like flags.

### Files and Directories

| Flag | Description |
|------|-------------|
| `--docs PATH[,PATH...]` | Prefetch docs into context—files, globs, or dirs (default: none) |
| `-w, --wave NAME` | Select Wave Work, or qualify selected Task Work. |
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
lf task run DES-123                 # Project's Flow
lf task run DES-124 --flow feature  # explicit template override
lf flow list --json                 # inspect available templates
```

| Flag | Description |
|------|-------------|
| `--docs PATH[,PATH...]` | Prefetch docs into context—files, globs, or dirs (default: none) |
| `-w, --wave NAME` | Wave name for wave/ scoping |
| `-m, --model MODEL` | Model to use |
| `--tui` / `--ide` | Hand off Claude, Codex, or OpenCode to the terminal, or Claude/Codex to their app; overrides `session.launch` |

Flows are defined in `.lf/flows/`. See [Configuration](config.md).

A **Flow** is a template; a **Flow invocation** captures its fully expanded
graph and execution state. A **loopflow** is a Flow with backward edges.
`lf task run ISSUE --flow feature` creates an invocation owned by the Task.
Independent `lf --wave designer : "Review the plan"` creates a Wave-attributed
Run without invocation membership. See [Flow decisions and recovery](#flow-decisions-and-recovery)
for the protocol and current verification limits, and [authoring](authoring.md)
for composition limitations.

### Builtin Flows

| Flow | Steps |
|------|-------|
| `build` | kickoff → code → review-slice → concept-review → demo |
| `code` | implement → compress |
| `pair` | design → code |
| `design` | author one exact design at a User gate |
| `launch-plan` | keep one coherent core here and launch independent follow-up Tasks |
| `feature` | task-design → pursue (default Task Flow) |
| `task-design` | kickoff → human review-design |
| `pursue` | repeat implement → compress → review-slice → concept-review → loop-decide, then human demo |
| `slice` | code → review-slice (publishes PR) → concept-review |
| `ship` | task-gate → record-learnings → op: pr land -c |
| `ship-demo` | task-gate → demo review → record-learnings → op: pr land -c |
| `deploy` | gate → op: pr land |
| `design-and-ship` | design → implement → reduce → polish → deploy |
| `incident` | restore → 5whys |
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

## Running Waves and Tasks

```bash
lf start designer                                  # serve it on this machine
lf wave serve designer                                   # foreground development mode
lf pause designer                                  # keep listening; queue new turn starts
lf resume designer                                 # enable queued and future turns
lf stop designer                                   # stop it; leave the Home keeper running
lf task prepare DES-123                             # Task Work + worktree, no execution
lf task create --wave designer --title "Dark mode" --notes "Keep contrast readable"
lf task create --run --wave <wave> --title "fix the flaky chord-timeout test"
pbpaste | lf task create --run --wave incidents
lf task run DES-123 --directive "fix the parser before the docs"
lf task run DES-124 --stack-on DES-123
lf task run DES-125 --flow incident
lf task status DES-123
lf task advance DES-123                              # drive the saved Flow
lf --as wave:product : "Which KR owns this?"      # ordinary agent perspective
lf ask "Review this proof with me"                    # block on a human session
lf session list --json                                # unresolved human Sessions
lf session open <session-id> --json                  # reopen the exact boundary
lf session complete <session-id>                     # return review or Ask feedback
lf session rename <session-id> "Release notes"        # suggestions preserve human names
lf task edit DES-123 --title "Rename the flag" --notes "Preserve existing options"
lf task comment DES-123 "take the smaller approach"
lf task comment DES-123 --json                       # read the complete thread
lf task interrupt DES-123                             # end the active turn
lf task wait DES-123
lf task resume DES-123 --reason "provider credentials repaired"
lf task restart DES-123 "Reconcile the new runtime research"
lf task restart DES-123 --flow build                 # replace the pinned Flow; rejected before any checkpoint if unusable
lf flow list --json                                  # every Flow with the topology it would pin
lf work status task task_... --json                  # stable Work projection
lf work interrupt task task_...                      # refuses without exact process ownership
lf work place wave wave_... home_...                 # move idle Wave Work to a Home
lf work relocate wave wave_... --name platform       # rename a stopped Wave
lf work relocate wave wave_... --repo ../moved-repo  # repair or move its repository
lf work enable task task_...                         # restore Task eligibility
lf --wave designer : "scan the runtime"             # independent Wave Run
```

`lf start <name>` asks this machine's shared keeper to serve the Wave and
records this machine as its Home. It enables the Wave in this Home's registry
and never follows a remote placement record.
Bare `lf start` is the automatic form: it starts only repo Waves whose optional
`owner` and `home` fields in `GOAL.md` match this machine and whose recorded
placement is local and enabled. The named form is the explicit override.
`lf wave serve <name>` runs the Wave listener in the foreground for development.
`lf --wave <wave> wave/operate` makes one finite planning pass over the chapter
and Tasks. Every Wave reads its Project in the repository's current Chapter.
Task Work has stable identities and small state:
`ready`, `done`, or `abandoned`. Process liveness, Task condition, Sessions,
PR state, Flow invocation, and Run evidence stay separate. `task prepare` ensures the
Project and Task Work records, one stable Task worktree, and its first serial PR
identity without starting execution. `task run` uses that same substrate,
selects a Flow when none is active, and starts its mechanical driver.
`task restart` waits for its captured worker to exit before replacing the Flow.
Unknown process identity, a worker that remains live, or a concurrent replacement
leaves the Flow intact and reports the unresolved execution. Retry after resolving
the reported worker; restart never discards a new claim acquired while stopping.
`task edit` changes title/notes in Linear and refreshes planning facts locally.
It works before placement; replacing notes preserves the original creation retry
marker. `task comment ISSUE` reads the thread; adding text publishes direction
without starting execution. Both accept the issue ID or a registered Task ID and
resolve the owning Wave from the issue. An optional `--wave` checks that ownership.

`task create` files planning backlog in Linear and refreshes the local snapshot.
It leaves the checkout alone and needs no agent execution credentials. Use
`--notes` for its description, or pipe a report; `--title` overrides the report's
first line. Repeating the same title and report reuses the issue's creation marker.
`task create --run` validates the Flow, workspace name, stack parent, base,
and execution credentials before creating a Linear issue. Retrying the same
report reuses its marked issue and any registered Task placement. If a later
filesystem or snapshot operation fails, the error names the retained issue and
its recovery command. Placement uses the validated base commit even if a later
fetch advances the remote branch. An unconfirmed creation reports the uncertainty
and directs a retry of the same command to look up its marker.
`task advance` drives an existing Flow until human input, a blocker, interruption,
or completion; repeated calls report the active driver. A human review waits
for its exact Session to be completed. Each autonomous boundary starts a fresh provider Run from durable
Task facts; no provider transcript or resident Task process is required.
`resume` preserves the Work, selected Flow, Steers, worktree, branch, and PR
while starting a fresh worker.
`task comment ISSUE TEXT` posts a Linear Task comment and never starts a worker. Direct Linear
comments enter the same delivery path. Only Task advancers consume steering;
independent `--task` or `--as` Runs do not. `task run` selects the Flow when
execution should begin. Wave guidance is extra input to `wave/operate`.
Wave names are repository-scoped. Relocation requires the UUID because the
repository and name may both change; it preserves authored Wave files, journal,
PM binding, Work state, and Home placement. Home-local Run records remain on
the Home that recorded them and are never rewritten as control state.
Relocation refuses a live Wave or claimed Task worker and never keeps an
old-name alias. UUID-addressed `lf wave` reads and mutations also verify that
the selected Work belongs to the invoking repository; a UUID from another
repository is not a capability.

The Task invokes its Project's Flow by default. `lf task run --flow <name>`
selects any template explicitly; Stop & restart can select another. Selection
captures an immutable expanded graph in a new invocation. Editing YAML or
changing the Project's Flow affects future selections only. Finished and
replaced invocations stay readable; completion clears the current invocation
without completing Task Work or selecting another Flow.

A Project normally selects `feature`: design review, then implement → compress
→ review-slice → concept-review → loop-decide. Iterate returns to implement;
Advance reaches `demo human:true`. Completing demo supplies feedback to a
second loop-decide, whose edge can also return to implement. `pursue` starts at
implementation; `task-design` finishes after its design review. PR delivery
and Task completion remain explicit operations.

### Flow decisions and recovery

```sh
lf task run DES-123 --flow feature  # create a Task-owned invocation
lf task advance DES-123             # drive its captured graph
lf task resume DES-123 --reason "provider credentials repaired"
lf task restart DES-123 --flow incident
```

A **loopflow** is a Flow with one or more backward edges. Each edge names an
earlier node. There is no pass limit. Steps outside a backward edge's body
do not repeat when that edge is taken. Advance enters the remaining steps;
Iterate traverses the declared section again. A slice is a unit of work within
a pass. Task binding adds context and Task authority.

The dedicated `loop-decide` step compares the previous direction with the pass's
evidence after both reviews. Its exact Run records one navigation decision:

```sh
lf flow decide advance "Evidence that this boundary's obligations are satisfied"
lf flow decide iterate "Remaining work, next action, and the proof to collect"
lf flow blocked "What stalled, what was tried, and what needs human judgment"
```

The reviews supply evidence; concept-review does not own navigation. Blocked is
a stopped execution outcome, separate from Advance/Iterate. Meaningful learning
counts as progress; repeating a failure without new evidence calls for help.

`lf flow blocked` keys one Ask to the exact invocation, node, and iteration tuple;
duplicate calls join it and retries recover its saved completion. Its Session
runs `unblock`, using concept-review with the human by default or addressing a
specific missing input. Human Complete returns the summary and shared artifact
changes to loop-decide for reassessment. It supplies evidence, not a navigation decision.
If the blocker remains unresolved, report it instead of opening identical Asks.
Invalid or missing decisions remain visible blockers.
A candidate decision takes effect only after its Run succeeds. Inspect any
operation's effects before choosing `--retry`, since an interrupted operation
may already have changed external state.

An invocation keeps its definition, cursor, return counts and runtime children
in the current Home's SQLite store. Continuation uses those captured facts even
if source definitions disappear. Completion grants no implicit merge or
Task-completion authority. Before launch the Task page shows its Flow template;
after launch it shows the expanded invocation.

Composed templates expand before execution. Runtime nested loops have parent
and child invocations, with a local node ID at each level. The displayed
iteration tuple follows that chain. A Run records its exact invocation, node
and tuple when the driver launches it; a Session projects that membership.
An independent conversation about the same Task does not become a Flow step.

Recovery reports unreadable invocation data without replacing it with today's
catalog. Inspect historical Wave continuations with `lf wave recover <name>`.
Custom or uncaptured work stays unresolved until explicitly cancelled with
`--cancel <source-seq> --reason <text>`. Unknown active attempts require
termination evidence before replacement.

A `human:true` step appears as a Flow Session. The reviewer saves feedback and
revised artifacts, then calls `lf session ready "feedback and remaining work"`.
The human ends the conversation with `lf session complete <session-id>`.
Completion returns feedback to the next step. A following loop-decide interprets
it and records Advance or Iterate against its own authored edge. Human reviews
have no implicit revision target. Closing the provider, marking ready, or
reopening a Session does not complete it.
Already finished invocations report completion rather than starting again.

Invocations capture all XOR routers and branch definitions at creation. A router records its exact choice with `lf flow route PATH`;
failed Runs discard that candidate. Nested paths and review completion use the
same cursor transition. Recovery fixtures prove these paths locally; live
provider/desktop review completion → decision → implementation → Ask → reassessment remains a
separate demonstration.

The `advance` skill also resolves these actions from an ordinary coding
conversation: complete an exact review, resume a Task, or prepare work from an
approved design. It reports the actual Task state after the handoff.

Every Task-owned PR keeps its authored title, naming the benefit of that PR.
Loopflow places the canonical Task name and Linear link after the opening
summary and refreshes merge consequences from durable state on publication,
submission, and landing. If the cached PM snapshot has no provider URL, run
`lf wave sync --wave <wave>` before publishing.

Task launch also resolves the execution boundary the work needs: the assigned
worktree, Loopflow's pinned planning store, and network access for delivery.
Headless Tasks require a managed Codex or Claude account with usable
credentials. The shared launch path writes the validated Run row and immutable
launch inputs before starting the provider. Run ancestry is attribution; it
does not reserve Task Work or grant mutation authority.

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
lf --batch --as wave:<name> : "Which proof matters?"
```

A headless Run that needs a decision from the user runs `lf ask "<request>"`. Loopflow
starts an ordinary TUI Run in the caller's exact checkout and waits. The session
agent calls `lf session ready "<summary>"` when its work is ready; this does not
complete, hide, or release anything. The user runs `lf session complete
<session-id>` when the conversation is finished. Completion stops the exact
provider client, removes the Session from the Sessions list, and resumes the caller
with the ready summary and any filesystem changes.

Choose the skill for the human Session when the request needs a particular
kind of conversation:

```bash
lf ask --skill unblock "Two passes repeated the same failure; reconsider the direction"
```

Ask owns the Session and wait; `unblock` guides the conversation inside it,
using concept-review by default. The selected skill name is saved with the Ask
so reopening keeps that choice. Completing this conversation returns
direction to the blocked caller for reassessment. The
`lf flow blocked` command uses this path and retains the Ask result
across retries of the same decision boundary.

A human Flow step uses the same Session surface. Its saved invocation owns
the exact boundary and captured Skill:

```bash
lf session list --json                              # unresolved Sessions
lf session open <session-id> --json                 # prepare/recover attachment
lf session ready "Feedback and remaining work"      # agent state; stays visible
lf session complete <session-id>                    # human ends the review
```

Flow review Sessions read Task and membership from their Runs. Closing, provider exit, or agent readiness never
completes a review; the saved boundary remains waiting. Loopflow.app lists Sessions
and exposes Complete for ready reviews. Selecting a Session already open in the app returns to its
live terminal. If its client is active elsewhere, **Move here** explicitly
stops that client and resumes provider-native history in the selected pane.
Closing a pane keeps the live terminal available in the Sessions list. Complete
finishes the conversation. Flow reviews return feedback to the next step;
Asks return feedback to their blocked caller.

Sessions may use a detached PTY cradle to let the first provider client
start before the desktop is present. That cradle is not Session identity,
readiness storage, liveness authority, or the attachment surface. The boundary
Session row owns resolution; its Run owns native provider identity, and the
provider owns conversation history.

Every Session JSON record includes a required `run_id`. Use it for Run lookup;
do not parse the Session ID. Ask and FlowStep boundaries prepare their Run before
publication, so an unopened Session already has an identity without a provider
process. Launch fills in that Run's context; native resume retains its identity.
For an older boundary without a Run, `lf session open <id> --json` prepares it
without starting a provider. Listing fails with that recovery command until the
boundary is prepared; it never allocates a Run itself.

`--stack-on` places a new Task worktree on another Task's published PR. Its PR
targets that parent branch automatically, then collapses onto `main` after the
parent merges. The two Tasks keep separate identities, worktrees, and workers.
Tmux remains process containment, not product identity or advancement authority.

## Sessions

```bash
lf --interactive --task INF-123 : "Review the change"
lf session list --task INF-123 --json
lf session rename <run-id> "Parser review"
lf session bind <run-id> --task INF-124 --json
lf runs --task INF-124 --json
lf session bind <run-id> --wave infrastructure --json
lf session bind <run-id> --repository --json
```

Session identity is its Run ID. A Run has zero or one Session: an interactive
conversation, Flow review, or Ask. Session title, title provenance, readiness
and completion live on that child record; Task, Wave, provider and membership
come from the Run.

Explicit `--task`, `--wave` or `--as` selects ancestry at launch. Without an
explicit selector, a registered Task checkout supplies the Task. Otherwise the
Run can remain unbound. This changes ancestry only; a companion terminal does
not inherit its neighboring Session's Flow membership.

Bind updates the Run's Task and Wave. `--wave` clears Task while retaining the
chosen Wave; `--repository` clears both. Binding to a completed Task or a landed
PR works without reopening Work. Session lists, Recent runs, usage attribution
and the sidebar follow the same fields. A Run owned by an invocation cannot
change to another Task or lose its Task; the command explains that constraint
and leaves the record unchanged.

Rename keeps the Session ID, provider, terminal, draft and Flow membership.
A human name survives generated suggestions. Bind keeps the same properties
and the name. Orphan sessions have no Task, including Wave-only conversations;
absence from the visible Task plan does not turn a bound Session into an orphan.

`open` resumes provider-native history. `ready` saves feedback without closing
anything. `complete` closes the Session and retains its history; default lists
show open Sessions. A Flow review passes its feedback onward, while an Ask
returns it to the waiting caller. Closing a pane or exiting the provider is
neither completion nor permission to advance the Flow.

## Placing Work and Reaching Homes

```bash
lf home id --json
lf home observe <home-id> ssh://jack@mini.local
lf wave place <wave-id> <home-id>
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

`lf wave enable|disable <wave>` and `lf task enable|disable <issue>` change
the same default-on machine control for the selected Wave or Task. Disabling
a Wave does not prohibit invoking an enabled Task directly, and it does not
stop an already-running descendant.

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

The **thread** is the user surface: durable, replayed, and owned by a running
Wave. Typed Work observations carry Task progress directly to the Wave.

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

Managed Work processes default to their invoking Wave through `LF_WAVE_ID`. From an
interactive shell, pass `--wave`; repository location does not identify one of the
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
lf ls --current --json          # current Waves, including stopped ones
lf status <wave> --json         # Work, Runs, conditions, and live metric_portfolio
lf roadmap --json               # current plan plus that portfolio on every Wave
lf activity                     # durable Work changes, newest first
lf activity --task INF-123 --json # filter before the bounded typed snapshot
lf runs                         # recent Home-local Run records
lf runs --active --json          # current provider-backed Runs and observation gaps
lf runs --active --watch --json  # retain discovery and stream snapshots (macOS)
lf runs --active --task LOO-291  # exact Task attribution, independent of checkout
lf runs --project parser        # one Project's Runs, filtered before the result cap
lf runs --parent run_ab12 --json # every direct child Run, uncapped
lf runs run_ab12                 # inspect one Run by unambiguous prefix
lf runs run_ab12 --final         # print the last durable provider conclusion
lf runs run_ab12 --events        # print its event stream verbatim
lf usage --project parser        # direct Run usage for one Project
lf usage --task INF-123 --json   # direct Run evidence for one Task
events. `lf runs`, `lf replay`, and `lf usage` select Run rows from the Home's
SQLite store. Task and Wave filters use indexed typed fields; Project filters
join through the Task's Project. Detail reads open only the selected evidence
artifacts.
