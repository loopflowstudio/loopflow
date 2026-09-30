# lf Command Reference

One binary, three audiences. `lf` launches prompts for you, gives agents the
verbs to run and steer other agents, and reads the executing Home's planning,
process, conversation and Flow history. Prefix a command with `lf ssh <home-id>` to
run the same local operation on another Home.

| You are | Start with | Deep dive |
|---|---|---|
| Running prompts | [Basic Usage](#basic-usage), [Context Flags](#context-flags) | [Get Started](getting-started.md) |
| Operating waves | [Running Waves and Tasks](#running-waves-and-tasks), [Speaking to Waves](#speaking-to-waves) | [Waves](waves.md) |
| An agent driving other agents | [Running Waves and Tasks](#running-waves-and-tasks) | [The Agent API](agent-api.md) |
| Watching the whole machine | [Reading This Home](#reading-this-home) | [Conducting](conducting.md) |

Every read surface takes `--json`; that JSON is the same wire the Mac app
renders. This reference uses current command spellings. The accepted ownership
contract and remaining implementation are centralized in
[cutover status](architecture-reference.md#cutover-status), including transitional
`lf runs` and `lf replay` command spellings. Session history uses captured event
sequences and exact provider references; `SessionRecord.run_id` is removed.

## Basic Usage

```bash
lf                                 # terminal-native Loopflow control conversation
lf desktop                         # explicitly open or focus Loopflow.app
lf <skill>                        # run a skill file
lf <skill>: args                  # run with arguments
lf <namespace>/<skill>            # run a repo-local or installed namespaced skill
lf npx/<owner>/<repo>            # fetch any Claude Skill live via npx skills
lf : "inline prompt"             # no skill file, just prompt
lf catalog                          # show skills and flows, including flow expansions
lf wave list                            # list Waves in the local registry
```

## Examples

```bash
lf gate                           # run the gate skill
lf implement: add auth            # pass arguments after colon
lf team/review                    # run .lf/skills/team/review.md
lf npx/vercel-labs/deep-research  # fetch a skill from the npx skills catalog
lf : "fix the typo"               # inline prompt
lf debug -c                       # paste clipboard, fix the bug
lf task checkout DES-123           # tracked Work + worktree, no execution
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

`--task` and `--wave` attribute a skill, Flow or inline prompt to existing Work.
Task implies Wave; broader selectors must agree. Task context includes its seed,
checkout and recursive scratch; Wave context uses its repository. A direct Flow
owns a separate FlowSession and cannot advance the Task's managed selection.
Independent conversations may share Task attribution without sharing driver or
Flow authority. Give concurrent file contributions distinct ownership and
checkpoint a coherent result with `lf commit`.

Inside a Task checkout, ordinary skills and Flows receive the same Task seed,
workspace details and live steers as managed steps, including after its PR lands.
Use `--as task:DES-123` from elsewhere. Work resolves in this order: this
command's explicit selection, the Task checkout, then an ancestor's explicit
selection carried in `LF_AS`. Checkout inference never changes that variable.
`lf task run DES-123` declares the same Work as `--as task:DES-123` and drives
the Task's selected Flow. An agent declared for X can enter Y's checkout and
start work for Y; the Exec parent still records X's agent as its caller.
Context does not grant permission to advance the managed Flow. Unattended commands in that checkout share the same
worktree confinement and account selection. Every assembled agent prompt resolves
the participant name through `LF_USER_NAME`, global `user.name`, then Git.

Flow steps resolve `lf` through PATH, including an explicit lock's leading
directory. If PATH has no `lf`, they use the selected installation, then the
driver executable as a last resort. Each child's Exec command records its
executable path. If a newer child advances the database
schema beyond its driver's support, the driver preserves the selected result
and exits with a compatibility diagnostic. Resume through a compatible `lf`;
the old driver does not settle or replay the completed step.

Each agent boundary has a conversation and native completion history. Flow retry
continues the selected conversation; a new boundary captures its instructions and
current context. Mechanical operations execute as authored. Use
`lf task run DES-123 --flow code` for the managed Task workflow.

Bare names prefer skills when a custom skill and Flow share a name. Use
`lf skill <name>` or `lf flow <name>` to select explicitly. `design`,
`launch-plan` and `debug` are skills.

## Repair a failure

```bash
lf debug : "Why does this request fail after reconnecting?"
cat incident.txt | lf incident
lf 5whys : "Investigate why yesterday's deployment failed"
```

`debug` investigates a code failure and fixes its cause. `unbreak` prioritizes
restoring the broken workflow, including a useful workaround when necessary.
Both accept the report where it is, including clipboard input with `-c`, and
verify the original workflow. `incident` starts with unbreak, then continues
through causal analysis and the `launch-plan` skill to select worthwhile prevention using ordinary Task
operations. Findings alone do not approve new work; a repair can finish with
no prevention Task.

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
4. Core built-in skills, grouped by Task, Wave, and Ops (`lf catalog` shows the live catalog)
5. External skill namespaces — `npx/<owner>/<repo>` fetches live via `npx skills` and caches under `.agents/skills/`; cached or searchable skills can often be run as `npx/<name>`. The legacy `rams/rams` alias also resolves when `~/.claude/commands/rams.md` exists.

Namespaced skills and flows use `/`, not `:`. Run `team/review`, not `team:review`.
Ownership uses `/` (`wave/operate`); words within one name use `-`
(`review-design`). Public catalog names never use `_`.

### Skill Arguments

```bash
lf implement: add user authentication
```

Inside skill files, `{args}` is replaced with whatever comes after the colon.

### Builtin Catalog

Skills and flows share one catalog organized by the thing they act on:
**task**, **project**, **wave**, and **ops**. `lf catalog` shows each flow both as
written and collapsed into the skills and operations that execute.

Task skills — concrete implementation, investigation, review, and delivery:

| Skill | What it does |
|------|--------------|
| `kickoff` | Turn product direction into an implementation plan, risks and proof |
| `research` | Answer a codebase question or investigate a decision with sourced evidence |
| `5whys` | Root cause analysis on a bug fix |
| `implement` | Build from a design doc |
| `compress` | Simplify code and surrounding implementation related to the diff |
| `gate` | Ship-ready code and reviewer-friendly docs |
| `unbreak` | Repair a reported failure and verify the original workflow |
| `debug` | Investigate a code failure, fix its cause and verify it |
| `ci-fix` | Fix failing CI checks for the current PR |
| `qa` | Thorough quality assessment of the current branch |
| `triage` | Correct and prioritize findings in their existing location |
| `review-design` | Reshape the working design around the intended experience |
| `design` | Interactive design session |
| `launch-plan` | Select useful work from a design or findings and arrange authorized Task execution |
| `realign` | Reconcile plan, code, and Wave memory with what the work has taught us |
| `concept-review` | Reconsider the intended experience together, then follow through concepts, data, and APIs |
| `loop-decide` | Assess a pass's progress and evidence to choose Advance, Iterate, or human help |
| `unblock` | Resolve stalled work with the human inside an Ask Session, using concept-review by default |
| `demo` | Walk the User through the changed behavior, or prove it headlessly and ask one exact blocking question |
| `refine` | Refine existing work |

Planning skills — shape and pursue the current chapter:

| Skill | What it does |
|------|--------------|
| `reduce` / `polish` | Find structural simplification and improvements to the experience |
| `testing-audit` | Audit test value, rigor, cost, lifecycle ownership, and product proof |

Wave skills — maintain the durable operating context and its portfolio:

| Skill | What it does |
|------|--------------|
| `s1` / `s2` / `s3` / `s4` / `s5` | Delivery, coordination, capacity, adaptation and identity; each investigates and acts |
| `wave-report` | Read health signals across all waves |
| `wave/operate` | Read, judge S1–S5, and focus on a few useful Wave moves |
| `review-open-work` | Survey branches, PRs, worktrees, and waves for inbox-zero triage |
| `split-wave` | Split future responsibility while preserving active work |
| `wave/start-chapter` / `wave/review-chapter` | One Wave's chapter: propose its fresh plan / report every KR verdict |

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
lf task changes DES-123 --base head --json           # net changes + recursive scratch listing
lf task diff DES-123 src/parser.rs --base parent --json # patch against recorded PR base
lf task file DES-123 src/parser.rs --json            # current worktree contents
lf wave --wave designer sync                           # normalized onto `sync`
lf task --wave designer create --title "Fix it" --notes "Repair startup"  # normalized onto `create`
lf commit -m "explain the change"                   # -m remains commit-local
```

Task file access reads recorded placement directly from the local registry; it
does not sync planning, reconcile PRs, or start a Run. Comparisons default to `parent`. Use `head` for the current commit,
or pass the `base_commit` SHA returned by `task changes` to pin subsequent reads.
`task diff ISSUE PATH --draft` reads a UTF-8 draft from stdin without changing the
worktree or index. `task file` returns complete UTF-8 content and a byte revision,
or an explicit binary, unsupported-encoding, missing, or over-1-MB state.

```bash
lf task save DES-123 scratch/notes.md --revision <revision-from-task-file> --json < draft.md
```

Save refuses an observed revision conflict. Each exchange retains a receipt,
submitted draft and displaced file under the checkout's Git metadata. Add
`--recoveries` to `task file` to inspect retained versions and late writes.
Ordinary content reads skip history inspection; Save inspects only its new
exchange. `task changes` returns the recovery root directory. Recovery remains after restart, deletion or rename.
Nothing automatically restores or deletes these versions. Open-descriptor writers
can still change a displaced file later. This preserves recovery without claiming
a compare-and-swap or power-loss guarantee. Files must be complete UTF-8 within
1 MB; Save rejects symlink paths and Git metadata.

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

Headless OpenCode launches retain the native conversation across automatic
provider retries. Flow steps use the native request's recorded completion;
usage includes its separate assistant calls. A failed saved-conversation lookup
reports the failure with the existing identity intact. A disconnected client
can leave native completion unknown; inspect retained evidence before retrying
an operation that may already have taken effect.

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
lf flow feature                    # no Task required
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
`lf flow feature` can create a taskless invocation with the same captured graph,
recovery, and human review boundaries. No planning record is required.
Independent `lf --wave designer : "Review the plan"` creates a Wave-attributed
AgentSession without Flow membership. See [Flow decisions and recovery](#flow-decisions-and-recovery)
for the protocol and current verification limits, and [authoring](authoring.md)
for composition limitations.

### Choose a stopping point

```bash
lf code                  # implement and simplify locally
lf queue                 # simplify, rebase, reconcile and verify; no PR publication
lf pursue                # iterate, publish and review the demo; no landing
lf feature               # kickoff and design review, pursue, queue, land and complete
```

For an existing design, kickoff turns accepted intent into the implementation
plan; it remains the first step of feature. For a small edit, use code. For work
intended to land after its reviews, use feature. Design and launch-plan remain
skills for shaping intent and arranging authorized execution.

### Builtin Flows

| Flow | Steps |
|------|-------|
| `code` | implement → compress |
| `feature` | task-design → pursue → queue → op: pr land -c (default Task Flow) |
| `task-design` | kickoff → human review-design |
| `pursue` | repeat implement → compress → refresh → loop-decide, then pr-publish, human demo and a deciding return to implementation |
| `refresh` | op: rebase → realign |
| `ship` | gate → op: pr land -c |
| `ship-demo` | gate → human demo → op: pr land -c |
| `deploy` | gate → op: pr land |
| `incident` | unbreak → 5whys → launch-plan |
| `queue` | compress → refresh → gate |
| `vsm-operate` | s1 → s2 → s3 → s4 → s5 |

`refresh` runs the existing `lf rebase` operation, including its branch push
with a lease, then realign reconciles the plan, code, and identified Wave memory.
It does not publish. In pursue, the publication step invokes `lf pr publish`
after convergence; that command consumes prepared copy or generates it with
the pr-message template.

The former pair/build/slice/design-and-ship shortcuts are retired. Choose the
endpoint above instead of an alias. Task-specific clarify/pursue/mutate and
iterate are folded into kickoff, implement, realign and the explicit decision/
publication steps. Research also handles conversational exploration. Realign
owns memory curation; update-wave and record-learnings are retired.

Wave/repository operation replaces the old garden and S2–S5 scan/assess/mutate
pipelines. No report file, chord configuration or governance pass gates Task work.
Existing captured Flows keep their pinned instructions; update custom Flow
references deliberately. Skill export prunes generated retired entries and
preserves personal skills. Source edits do not change installed exports.

Default-branch refresh is safe from sibling worktrees: it stashes dirty edits
on the checked-out default branch, syncs, then restores them — unless they
collide with paths the sync rewrote, in which case they stay in a
`sync_main: auto-stash` stash so a sync can never silently revert just-landed
work.

Flow authoring — `op:` steps, `xor` branching, routers — is covered in
[Authoring](authoring.md).

## Running Waves and Tasks

```bash
lf task checkout DES-123                             # Task Work + worktree, no execution
lf task create --wave designer --title "Dark mode" --notes "Keep contrast readable"
lf task create --run --wave <wave> --title "fix the flaky chord-timeout test"
pbpaste | lf task create --run --wave incidents
lf task run DES-123 --directive "fix the parser before the docs"
lf task run DES-124 --stack-on DES-123
lf task run DES-125 --flow incident
lf -m claude task run DES-126    # retain this agent for every Flow step
lf task status                 # Task on the checked-out branch
lf task status DES-123
lf task run DES-123                              # drive the saved Flow
lf --as wave:product : "Which KR owns this?"      # ordinary agent perspective
lf ask "Review this proof with me"                    # block on a human session
lf session list --json                                # unresolved human Sessions
lf session open <session-id> --json                  # prepare the exact boundary
lf session complete <session-id>                     # return review or Ask feedback
lf session rename <session-id> "Release notes"        # suggestions preserve human names
lf task edit DES-123 --title "Rename the flag" --notes "Preserve existing options"
lf task comment DES-123 "take the smaller approach"
lf task comment DES-123 --json                       # read the complete thread
lf task interrupt DES-123                             # end the active turn
lf task wait DES-123
lf task run DES-123 --reason "provider credentials repaired"
lf task restart DES-123 "Reconcile the new runtime research"
lf task restart DES-123 --flow feature                 # replace the pinned Flow; rejected before any checkpoint if unusable
lf flow list --json                                  # every Flow with the topology it would pin
lf task status task_... --json                  # stable Work projection
lf wave place wave_... home_...                 # move idle Wave Work to a Home
lf wave relocate wave_... --name platform       # rename a Wave
lf wave relocate wave_... --repo ../moved-repo  # repair or move its repository
lf --wave designer : "scan the runtime"             # independent Wave conversation
```

`session open --json` prepares a review; execute its returned `open_argv` in the
terminal to enter the conversation. That command carries the executable, Home
and database together so changing the selected installation does not redirect
the saved handoff. Early startup failures identify the attempted executable and
owning data, and the opening command's journal retains the failure after a
retry. Opening or retrying a review does not complete it. Session listing still
reads the selected installation's store; save the handoff before switching.

`lf --wave <wave> wave/operate` makes one finite planning pass over the chapter
and Tasks. Every Wave reads its Project in the repository's current Chapter.
Task Work has stable identities and small state:
`ready`, `done`, or `abandoned`. Process liveness, Task condition, Sessions,
PR state, FlowSession, and command/conversation history stay separate. `task checkout` ensures the
Project and Task Work records, one stable Task worktree, and its first serial PR
identity without starting execution. `task run` resumes the saved Flow or starts
one when none is active. Selecting a different `--flow` replaces the unclaimed
managed invocation; stop an active worker before changing its Flow. Other Flows
attributed to the Task keep their own invocations.
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
`task run` continues an existing Flow until human input, a blocker, interruption,
or completion; repeated calls report the active driver. A human review waits
for its exact Session to be completed. Each autonomous boundary selects an AgentSession from durable
Task facts; retry preserves its native history. No resident Task operator is required.
Continuation preserves the Work, selected Flow, Steers, worktree, branch, and PR.
`task comment ISSUE TEXT` posts a Linear Task comment and never starts a worker. Direct Linear
comments enter the same delivery path. Only Task advancers consume steering;
independent `--task` or `--as` conversations do not. `task run` selects the Flow when
execution should begin. Wave guidance is extra input to `wave/operate`.
Wave names are repository-scoped. Relocation requires the UUID because the
repository and name may both change; it preserves authored Wave files, journal,
PM binding, Work state, and Home placement. Home-local command and conversation evidence remains on
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
→ refresh → loop-decide. Iterate returns to implement;
Advance publishes the PR, then reaches `demo human:true`.
Already-captured invocations keep their own graph and review boundaries. Completing demo supplies feedback to a
second loop-decide, whose edge can also return to implement. `pursue` starts at
implementation; `task-design` finishes after its design review. PR delivery
and Task completion remain explicit operations.

A Task retains `-m` supplied to `task run`, `create --run`, or `restart`.
The chosen agent overrides every step's frontmatter. With no Task choice, the
step's agent/default_agent applies, then checkout configuration. Omitting `-m`
keeps the saved choice; `task status` shows the saved choice (`null` in JSON means default). A live conversation keeps
its captured agent; changing it immediately uses interrupt then `task run` with `-m`.


### Flow decisions and recovery

```sh
lf task run DES-123 --flow feature  # create a Task-owned invocation
lf task run DES-123             # drive its captured graph
lf task run DES-123 --reason "provider credentials repaired"
lf task restart DES-123 --flow incident
```

Use `lf flow resume INVOCATION --retry` or `lf task run DES-123 --retry`
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
Compress simplifies code related to the change. Refresh rebases, then runs
realign. Realign edits the plan, corrects clear code mismatches, and reads and
updates the identified Wave's memory using what the work has taught us. Accepted
requirements and unresolved evidence stay visible; no per-pass report or
replacement count is required. Loop-decide reads the current work and evidence
to decide whether to continue. Pursue publishes after convergence, before demo.

Concept-review is interactive, on request or inside unblock, and does not own
navigation. Blocked ends the deciding turn and requests feedback. Meaningful learning
counts as progress; repeating a failure without new evidence calls for help.

A blocked result keys one Ask to its exact captured event and Flow position;
recovery reuses that question and its saved completion. Its Session
runs `unblock`, using concept-review with the human by default or addressing a
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

An invocation keeps its definition, cursor, return counts and runtime children
in the current Home's SQLite store. Continuation uses those captured facts even
if source definitions disappear. Completion grants no implicit merge or
Task-completion authority. Before launch the Task page shows its Flow template;
after launch it shows the expanded invocation.

Composed templates expand before execution. Taking an Iterate edge starts a
child FlowSession for that pass. Retry keeps that child; another Iterate starts
the next pass. Resuming the root follows its current child, including a pending
review. Membership names a node and iteration tuple in the
captured graph; an independent conversation about the same Task does not become
a Flow step. Graph keys, current/completed nodes, return edges and Session
membership use captured numeric node IDs, local to that FlowSession. Authored
occurrence names remain labels; nested containment comes from the graph.

A boundary can fail several times before succeeding. AgentSession history keeps
each provider start, result and usage receipt; the Flow consumes only its exact
selected successful completion. A mechanical boundary runs in its own child
`lf` process and records start and outcome in Flow history. Resume waits for a
surviving step and consumes its result once. Task stop includes the selected step
even when its driver has exited. An earlier success, helper completion or stale writer cannot
advance the current selection. Missing capture data is reported without replacing
it with today's catalog; missing external-effect evidence requires inspection.

A `human:true` step appears as a Flow Session. The reviewer saves feedback and
revised artifacts, then calls `lf session ready "feedback and remaining work"`.
The human ends the conversation with `lf session complete <session-id>`.
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

Every Task-owned PR keeps its authored title, naming the benefit of that PR.
Loopflow places the canonical Task name and Linear link after the opening
summary and refreshes merge consequences from durable state on publication,
submission, and landing. If the cached PM snapshot has no provider URL, run
`lf wave sync --wave <wave>` before publishing.

Task launch also resolves the execution boundary the work needs: the assigned
worktree, Loopflow's pinned planning store, and network access for delivery.
Headless Tasks require a managed Codex or Claude account with usable
credentials. The shared launch path reserves the conversation and immutable
launch inputs before starting the provider. Causal ancestry is evidence; it
does not reserve Task Work or grant mutation authority.

If a provider returns normally after a permission, control-authority, or
network command failure, Loopflow records the exact command blocker as a
non-resumable Task failure. Status assigns the next move to the User with
`no_action`; missing-process reconciliation and automatic recovery do not
replace or repeat it. Correct the capability, then run
`lf task run ID --reason "<what changed>"` to create a fresh input boundary.

Worktree safety comes from short OS-held mutation locks and prepared Git state,
not conversation identity. Commit, restart checkpointing, rebase, and land serialize
their exact critical sections and refuse a conflicting prepared state. An
active rebase additionally owns one exact operation id so its recovery child
can continue that sequencer without authorizing any other Git mutation. A
surviving unclaimed provider PID has no mutation or signal authority.

A skill that needs another Work's perspective launches an independent conversation directly:

```bash
lf --batch --as wave:<name> : "Which proof matters?"
```

A headless agent that needs a decision from the user runs `lf ask "<request>"`. Loopflow
starts an interactive AgentSession in the caller's exact checkout and waits. The session
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
direction to the blocked caller for reassessment. A Flow decision
with `blocked` uses this path and retains the Ask result across driver recovery.

A human Flow step uses the same Session surface. Its saved invocation owns
the exact boundary and captured Skill:

```bash
lf session list --json                              # unresolved Sessions
lf session open <session-id> --json                 # prepare/recover attachment
lf session ready "Feedback and remaining work"      # agent state; stays visible
lf session complete <session-id>                    # human ends the review
```

Flow review AgentSessions retain their Task ancestry and exact Flow membership. Closing, provider exit, or agent readiness never
completes a review; the saved boundary remains waiting. Loopflow.app lists Sessions
and exposes Complete for ready reviews. Selecting a Session already open in the app returns to its
live terminal. If its client is active elsewhere, **Move here** explicitly
stops that client and resumes provider-native history in the selected pane.
Closing a pane keeps the live terminal available in the Sessions list. Complete
finishes the conversation. Flow reviews return feedback to the next step;
Asks return feedback to their blocked caller.

Sessions may use a detached PTY cradle to let the first provider client
start before the desktop is present. That cradle is not Session identity,
readiness storage, liveness authority, or the attachment surface. The AgentSession owns resolution and native conversation identity; the provider
owns its native storage format. Current wire fields, including `run_id`, remain
listed in the cutover status until Rust and Swift consumers migrate together.
Use the returned Session ID and `open_argv`; never derive identity from a name or
path. Listing is passive and does not prepare or launch a provider.

```sh
lf task checkout DES-124 --stack-on DES-123
# In DES-124's existing checkout:
lf rebase --plan
lf rebase
```

`--stack-on` places a new Task worktree on another Task's published PR. For an
already-prepared root Task, it selects that parent while preserving the checkout,
branch, recorded fork commit, published PR and execution history. Selection does
not rebase Git or retarget GitHub; `lf rebase` integrates the parent, and the next
`lf pr publish` updates the existing PR's base. Same-parent retries keep the
original parent PR identity even after its Task opens another PR. Stop an active
Task worker through the existing Task recovery controls before changing its
parent; cancel pending delivery first. Changing an existing parent is a separate
reparenting decision and is refused here. After the parent merges, `lf rebase`
collapses the child onto `main`. The Tasks keep separate identities and worktrees.
Tmux remains process containment, not product identity or advancement authority.

## Saved Flows

```bash
lf flow list --sessions --json --limit 100
lf flow list --sessions --for-task INF-123 --managed true --json
lf flow list --sessions --parent FLOW_SESSION --json
lf flow show --sessions --json FLOW_SESSION
```

List saved progress without starting work. `--after` accepts the previous page's
`next` ID with the same filters. `--all` includes other repositories and unknown
historical repository evidence. `--state` selects current, completed or replaced
records; `--search` matches a literal name or identity. Detail reads the captured
graph even if its template or checkout is gone. A runtime child's parent differs
from the root FlowSession selected by its Task.

`lf flow list --json` and `lf flow show TEMPLATE` still inspect reusable templates.

## Sessions

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
name, feedback and native history. The new driver receives write authority; the
old client can remain a passive display. Restart explicitly replaces the exact
conversation owner. It preserves recorded history and does not kill a shared
engine or its other conversations. Unsubmitted editor text requires its own
surface-preservation proof.

Explicit `--task`, `--wave` or `--as` selects ancestry at launch; a registered
Task checkout supplies it when no explicit selector is present. A conversation
can remain unbound. CLI states the permanent bind target and writes; Desktop confirms it. Same-target bind is a no-op; there is no reassignment or unbind.
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

```bash
lf session import --dry-run
lf session import
```

One-time import preserves old interactive/Ask/review conversations, headless
history, captures, outcomes, usage and unknown evidence. Original identities and
repeated attempts survive; unrelated conversations never merge by title or path.
Ordinary reads use SQLite and neither import nor fall back to files. Import
reports conflicts and unresolved evidence, supports interruption/retry, and does
not infer an actual lf process from an old provider-launch record alone.
SQL agent inputs retain their recorded conversation even when an old input link
is absent. A standalone agent input keeps its original input ID as its Session
selector; recorded command outcome does not become a successful native turn.
Named mechanical boundaries with a captured Flow retain their SQL evidence in
Flow history. The forward migration retains unresolved original SQL in immutable
`import_evidence`, preserving unknown conversation membership. Captured inputs
belong to Session history; the input catalog and Run lifecycle table are removed. Import reports each unresolved input
in `failed`, retaining its original evidence. An operation counts as preserved only when its complete SQL
evidence, captured boundary and recorded completion match Flow history.
`lf runs INPUT --json` reads retained input history even when its manifest is
missing; exact Session IDs select current input. Prefixes must be unambiguous.

## Placing Work and Reaching Homes

```bash
lf home id --json
lf home observe <home-id> ssh://jack@mini.local
lf wave place <wave-id> <home-id>
lf ssh <home-id> wave status shipper --json
lf ssh <home-id> --wave shipper wave/operate
```

Placement selects an execution destination. It does not create a process or
transfer local signal authority.

`lf ssh <HomeId>` resolves the observed route and makes the target prove its
identity. Everything after the target is normal `lf` syntax. Foreground commands
can use origin and target accounts; detached processes scrub forwarded authority.

## Talking with Wave context

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

## Reading This Home

```bash
lf wave list --json                    # every durable Wave and its Home/runtime evidence
lf wave list --current --json          # current Waves, including stopped ones
lf wave status <wave> --json         # Work, Runs, conditions, and live metric_portfolio
lf roadmap --json               # current plan plus that portfolio on every Wave
lf activity                     # durable Work changes, newest first
lf activity --task INF-123 --json # filter before the bounded typed snapshot
lf runs                         # recent Home-local Run records
lf runs --active --json          # live Sessions and observation gaps
lf runs --active --watch --json  # retain discovery and stream snapshots (macOS)
lf runs --active --task LOO-291  # exact Task attribution, independent of checkout
lf runs --project parser        # one Project's Runs, filtered before the result cap
lf runs --parent run_ab12 --json # every direct child Run, uncapped
lf runs run_ab12                 # inspect one Run by unambiguous prefix
lf runs run_ab12 --final         # print the last durable provider conclusion
lf runs run_ab12 --events        # print its retained event stream
lf usage --project parser        # direct Run usage for one Project
lf usage --task INF-123 --json   # direct Run evidence for one Task
lf session list                  # Sessions, Work paths, actions and unavailable reasons
lf session open sess_ab12         # resume an open conversation after provider exit
lf session open sess_ab12 --try   # let the provider arbitrate an active session
lf session open sess_ab12 --replace # stop Loopflow's client, then continue here
lf session open sess_ab12 --json --replace # prepare a takeover command without stopping it yet
lf session complete sess_ab12     # finish it; provider history remains resumable
lf replay run_ab12               # launch that request as a child Run
lf usage --days 30              # direct provider-authored usage per Run
lf usage --days 0 --json        # all Session history; zero means all time
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

Continue a conversation with `lf session connect SESSION`. The obsolete
`lf runs INPUT --resume` entry is removed; `lf runs` only inspects history.
`lf replay INPUT` separately launches its immutable recorded request.

Session replacement and native client stopping require readable process evidence.
If inspection fails, retry after it is available; the command leaves termination
unconfirmed and preserves native history. Saved Ask or Flow feedback survives a
cleanup failure, which is reported separately from completing the review.
Native client publication and stopping share exact launch exclusion, so stopping waits
for an in-flight launcher to publish its client before inspecting it. A Task
with confirmed deletion cannot start an Ask or resume a native Session. Exact
`runs --active --task ISSUE` still inspects its retained process evidence.

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
disposable data directories still work. The snapshot does not copy Run bundles
or account files, and does not transfer the launching Session's execution authority.
An explicit private `LF_HOME` also selects the observation and Run destination,
even when the Session supplied installed control paths. Child processes use
that same data directory; re-entering it preserves its own Run context.
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
The historical command and JSON names remain during the coordinated wire cutover.

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

The `--parent` drill resolves one exact Run and returns all direct children
without the seven-day presentation cap. The one-Run `--final` read projects the
last durable provider conclusion from normalized conversation events in Session
history. Final and event reads retain their input order after import and work
without the old artifact directory. Summary and usage reads exclude conversation
text before loading payloads. Records
without a phase receipt are labeled and expose streamed prose from their last
completed provider turn. It does not parse vendor output or invent a conclusion
for an unsettled Run.
Replay uses the immutable prompt, agent/model, non-secret provider account ID,
and tool boundary recorded before spawn; it never reconstructs those inputs
from current planning or prompt configuration. Managed Claude/Codex replay
resolves that account ID through the current Home's deterministic credential
directory or an explicit forwarded lease; replay uses the same validated Run
creation path as any other launch.
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
continuation does not freeze a snapshot across imports or changing outcomes.

Search matches literal command text across argv elements, ignoring ASCII case;
`%` and `_` are ordinary characters. The original stored command remains in JSON.
Malformed historical command text remains searchable as recorded. Exact lookup
accepts a full ID or an unambiguous prefix. An `unknown` outcome means no terminal
observation, and says nothing about whether the process is alive. Discovery reads
bounded command rows without loading conversation captures or transcripts.

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

`lf usage` reads the same Home-local Session history as `lf runs`, newest first.
`--days` defaults to 30; zero selects all retained history. Original manifest/SQL time selects imported history; new captures use their
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
These projections have no resumable lifecycle. Rust and Swift use the same shape;
`RunSnapshot`, string `subjects`, and input-level `outcome`/`ended` are removed.

Cumulative counters are reduced once per stream. Omitted counters stay unknown,
final receipts and evidence gaps remain explicit. Native thread/turn correlation
prevents counting recorder checkpoints twice. Without a retained baseline, usage
reports the observed suffix as partial. A native completion supplies neither
missing usage nor a command outcome.

Recent `lf runs` summaries select their budget before reading history payloads,
retaining every eligible unfinished entry. Exact Task and caller drills have no
presentation cap. Original `run_` selectors remain valid; a Session selector
reads its current captured event. Unknown historical SQL remains import evidence,
not an invented conversation. Full populated-import and configured-provider
acceptance remain cutover obligations.

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

### Launch Claude, Codex, or OpenCode interactively

```bash
lf design                 # direct TTY → uses session.launch (default: tui)
lf gate --tui             # force a terminal handoff for a normally-headless skill
lf : "fix the bug" --ide -m codex   # force the Codex app instead
```

`--tui` opens Claude, Codex, or OpenCode in the terminal. `--ide` opens Claude
or Codex in its app. Both override the repo default. Set `session.launch: ide`
in `.lf/config.yaml` to make the app the default for direct interactive
skills. Automated flow nodes and `--batch` remain headless.

Inside a Task's worktree, a launch without `--task`/`--wave`/`--as` belongs to
that Task: its Session lists the Task as Work and `lf runs --task` finds it.
The checked-out branch identifies the Task; tracking `origin/main` or a stack
parent does not change ownership. An explicit selection still wins, and an
unregistered branch uses an inherited explicit selection when present, otherwise stays unbound.

### External skills

```bash
lf npx/vercel-labs/deep-research   # fetch + run from the npx skills catalog
lf npx/explain-code                # already-cached skill (no network)
```

`npx/` uses `.agents/skills/` in the current repo as a cache. Use `npx/<owner>/<repo>` when you know the package name; cached or searchable skills can often be run as `npx/<name>`. On a cache miss, Loopflow runs `npx skills add` first, then falls back to `npx skills find` when it needs a package hint. The core `task/` / `project/` / `wave/` / `ops/` catalogs are always available, and the legacy `rams/rams` alias still works when `~/.claude/commands/rams.md` is installed.

## PR Operations

The publish/submit/arm/land contract every launched agent receives is
`rust/loopflow/src/engine/builtins/LOOPFLOW.md` — that file is canonical for
agent-facing semantics; this section is the user reference.

### lf pr publish

Push and create or refresh a ready PR, then print its state and URL. Existing
drafts become ready for review. Opens no
browser — this is the headless publication command agents use.

```bash
lf pr publish
lf pr publish --title "area: short title" --body "## Summary ..."
lf -m codex pr publish        # one-off agent override for copy generation
```

An existing GitHub PR can be attached to its Task even when local publication
history is absent. If draft promotion fails, its identity remains attached and
retryable; reviewer copy is recorded after promotion succeeds.

When `-m` is omitted, copy generation uses `agent:` from `.lf/config.yaml` or
`~/.lf/config.yaml`. Use the `pr` ops skill to generate `--title`/`--body`
with agent judgment. When task gate has written cached PR copy, publication
consumes it and removes the gate-owned PR copy files before its first
commit or push. Other `scratch/` state remains untouched. Publication never
fetches to integrate, rebases, rewrites Task stack metadata, or launches
conflict recovery. A PR may remain behind its base until `lf rebase`, `lf gate`,
`lf pr submit`, `lf pr arm`, or `lf pr land` owns integration. Push or GitHub failure returns
an error and presents nothing.

### lf pr open

Push and create or update a draft PR, then open its GitHub page. Existing drafts
stay drafts; existing ready PRs stay ready. Opening the page does not mark a
draft ready. If launching the browser fails, the PR still exists and its URL
is printed.

Use `publish`, `submit`, `arm`, or `land` to mark a draft ready. A headless Flow
`op: pr open` uses the same draft policy without opening a browser.

### lf pr submit

Prepare the exact PR head, assign it to you, and stop for your merge click.
Nothing merges automatically. Task and non-Task branches use the same command.

```bash
lf pr submit
```

`submit`, `arm`, and `land` keep the published title and body when the remote
and local heads match. Explicit copy or valid gate output overrides that copy;
unpublished changes still generate a fresh description. Task merge consequences
are updated regardless of where the copy came from.

Inside a managed Task worktree, `submit` records a user-owned exact-head merge
request in the Task PR state. It does not advance or consult the Task's Flow
invocation. Use `-c` to complete the Task after merge or `--next <slug>` to
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

Preparing a replacement head disables pending auto-merge or removes the PR
from GitHub's merge queue before pushing, then arms the updated head.

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
another one. This is a delivery-state decision and needs no invocation claim.

Submit, arm, and land clear `scratch/`, preserve a recovery ref, collapse the
authored range to one tree-identical commit, replay that commit onto the pinned
target, verify it, and push once. A range with one linear commit keeps that commit
unless the target changes. Ordinary `lf rebase` keeps commit history.

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

Rebase publishes the resulting branch to `origin` with a lease. If the remote
branch was deleted, it recreates it even when local tracking is stale. Unseen
remote changes or a branch recreated during the push still reject publication.

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

Before upgrading a Home that still runs the retired `lfd` service, stop and
uninstall it with that older release's `lfd uninstall` command. New installations
do not manage or require a daemon; promotion does not stop an existing one.

Updates the installed CLI and macOS application through verified
release downloads and the existing promotion transaction. Requires no Git
repository, source checkout, Python, uv, or Homebrew. An already-current,
complete release skips asset downloads; missing or stale artifacts are repaired.
Use `lf rebase` for checkout updates and your project's own tools for dependency
setup.

Published installation selects the published data directory. When this differs
from the current selection, promotion prints both database paths and the retained
installation ID. Tasks and history in the previous database stay there;
installation does not transfer them. Use the retained development restoration below to return.

Run installation outside Task execution. Tasks and their descendants cannot
promote, update, roll back, or recover the machine installation, including after
switching to private branch data. Read-only candidate preflight remains usable.

To restore retained development data while promoting a local build:

```bash
local-bin/lf install promote --from-build local-bin/lf --reuse-home local-<id> \
  --cli-target ~/.local/bin/lf \
  --app-source local-bin/Loopflow.app --app-target /Applications/Loopflow.app --preview
```

Remove `--preview` to apply. `--reuse-home` reads the prior installation receipt
and preserves that installation's planning, Tasks and execution history. `--fresh`
instead forks published data into a new development data directory; it does not
carry history from another development installation.

When a release contains the exact draft SQL already applied in a retained database,
local promotion preserves that data and adopts the release receipt without
rerunning the SQL. Its preview checks the draft order, checksums and resulting
schema. Changed or unmatched drafts still require explicit recovery.

An incompatible development database reports its path and applied draft names,
IDs and checksums. Keep that database and its WAL. Recovery advice verifies
retained artifact bytes and runs the retained CLI's read-only exact-store
preflight before naming a compatible executable/database pair. If none passes, the
error names the missing evidence. A retained database preserves its own history;
switching to it does not transfer or repair private branch writes. Perform any
installation change outside Task execution.

For an older `lf` whose install command requires a source checkout, upgrade once
with the external installer, then use `lf install` for subsequent updates:

```bash
curl -fsSL https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh | sh
```

The external installer verifies release assets and enters the same promotion
transaction. The hidden `scripts/install.py refresh` entrypoint remains for older
installed CLIs: it delegates to the release installer without recursing into the
old CLI. Keep it until those installed callers can upgrade without it. The old
`pull-local-bin.sh` entrypoint has been removed.

The scheduled job invokes the installed `lf install`, with stable tool paths and
logs at `~/Library/Logs/Loopflow/refresh.log`. It runs at login and on the selected
cadence: weekly on Monday at 09:00 (the default), daily at 09:00, hourly on the
hour, or every five minutes on clock multiples of five. All times are local;
launchd coalesces sleeping calendar intervals into one run at wake. Rerun
`lf install schedule` to update an existing job and remove its old source-checkout
dependency. Failed downloads or promotion remain nonzero and can be retried.
Linux supports `lf install`; automatic scheduling currently requires macOS.

`lf catalog`, authentication, Home identity, and machine inspection also
work outside repositories. `lf auth route show` displays defaults there;
`lf auth route set --repo owner/name` selects a repository explicitly. Inside a
repository, catalog and listing commands use its context. Outside, `lf wave list`,
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
lf pr checks                      # CI status for the current branch
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

Run `lf wt prune --dry-run` to inspect candidates, then `lf wt prune` to remove
eligible worktrees. There is no automatic service sweep.

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

## Planning commands

Read and edit a Wave's Linear planning state. Each Wave maps to an Initiative;
its Projects and Tasks map to Linear Projects and Issues. `sync` refreshes the
local planning projection. Current Project statuses determine the shared chapter
name; no Chapter identity or packet is stored locally.

```bash
lf wave list                                # linked waves and task counts
lf wave connect --wave designer --team-key DSG   # connect Wave; establish repo Team once
lf wave sync --wave designer                  # refresh SQLite from Linear
lf doctor --planning                           # report drift without writing
lf wave status designer                  # read shared planning
lf wave status designer --no-sync        # cache-only agent/app read
lf wave update-plan --wave designer --plan plan.json
lf repo new-chapter 2026-10 --dry-run --json
lf repo new-chapter 2026-10 --json
lf task create --wave designer --title "Dark mode"
lf task edit 1207... --title "Refine dark mode"
lf task comment 1207... --json       # read the complete comment thread
lf task complete 1207... --summary "Dark mode delivered"
lf wave rename designer --title "Designer"   # rename the Initiative
lf repo reteam                            # dry-run the repository-wide Team migration
lf repo reteam --apply                    # migrate when no Task worker can write old ids
```

`update-plan` replaces one Wave's current Project content. `repo new-chapter`
converges every Wave on the explicit name using fresh Linear statuses. It takes
no plan packet. Started unfinished Tasks retain identity, checkout, PR and saved
Flow; proven untouched backlog is canceled. See [Waves](waves.md#the-planning-model)
for preview, adoption and partial-rotation retry rules. Completed Projects retain
historical plans in Linear.

`lf task delete ISSUE` deletes registered or planning-only Tasks from Linear and
reconciles their local record. Completed work keeps its outcome and terminal time;
unfinished work is retired without recording success. Authored files and retained
PRs survive. If an effect is incomplete, the error gives the same command to retry.
Deletion does not certify process termination. Confirmed removals leave Task lists
and shared planning views; historical accounting remains available.

For retained Tasks whose removal is confirmed, `lf task status ISSUE` reads saved
history without reconciling PRs or completion. Checkout-default status requires an
explicit identifier for that history. Deleted Tasks cannot be selected for new
execution; recorded work keeps its original Task attribution.

Task completion refuses an issue already canceled or marked duplicate in Linear.
Registered Tasks must also pass the delivery gate. If a provider request or refresh
fails, registered completion retains pending writeback; repeat the command to
reconcile it without changing the original completion time. A later provider
conflict preserves recorded Done history and reports the pending conflict.

Connect Linear first with `doppler run -- lf auth connect linear`. `lf wave connect` pins the Initiative
into `GOAL.md` and the repository Team into `.lf/config.yaml`. Every Wave in
that repository reuses the Team and Task prefix (`LOO-1`, `LOO-2`); Initiatives
and Project membership decide which Wave owns a Task. `wave connect --all` discovers
nested `GOAL.md` files recursively and initializes them against the same Team.
When no Initiative is pinned, connect links one exact title match, creates one
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
canonical slug. `lf wave status WAVE` reads cached planning without contacting Linear.
Use `lf wave status WAVE --sync` to refresh before reading; the refresh has a
five-second deadline and reports failure when fresh planning is unavailable.

Fresh PM operations renew expiring Linear credentials automatically and store
the rotated access/refresh pair together. Temporary endpoint failures get one
retry within the read deadline; `--sync` reports failure if it cannot obtain a
fresh snapshot. Retry a temporary failure with `lf wave sync --wave <wave>`.
Reconnect with `doppler run -- lf auth connect linear` only when the error identifies a
missing credential or unusable refresh grant/client configuration. A timeout
during persistence can leave its outcome pending; the next read checks the
stored credential before attempting another exchange.

`lf repo reteam` migrates every linked Wave onto the repository Team. It
**defaults to a dry run** and only mutates with `--apply`; it defers an issue
while a Task worker can still write its old identifier. Completed issues move too.
Loopflow first attaches the destination Team to every Project, comments and
moves Issues by UUID, narrows Projects to exactly that Team, repairs Wave-path
titles, verifies every association, refreshes every snapshot, and only then
removes legacy Wave Team fields. Interrupted runs keep a legacy sentinel and
resume without duplicating comments or moves.

## lf release

Mechanical release subcommands; `lf release run` is the full workflow.

```bash
lf release run patch          # full release workflow
lf release run minor          # close/reuse a patch, then publish the cycle milestone
lf release check              # exact commits in the target range
lf release notes 1.2.3        # narrative notes from decisions + commits + PRs
lf release notes 0.13.0 --preview  # print cycle notes since v0.12.0; no release writes
lf release notes 0.12.0 --preview  # existing version: end at v0.12.0, not today's HEAD
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
consumes the prepared bytes from an exact-tag worktree. For patch releases,
no merged changes is a successful no-op. A minor completes a closing patch only
when changes remain, otherwise reuses the latest patch. Its notes span the
preceding `.0` tag; both releases share a product snapshot. Retrying an interrupted
minor finishes its recorded pair. An incomplete latest tag resumes; it never cuts a newer tag
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
