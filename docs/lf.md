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
status and show never create Projects. Desktop prepares the Project on opening
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

## Run a Flow interactively

```bash
lf -i -a claude flow my-flow
lf -b -a claude flow my-flow # run every step headlessly
```

Each skill opens in the native agent conversation. Exit the conversation
successfully to advance to the next step; an interrupted or failed step stops
the Flow. Without either flag, an attached terminal selects interactive mode,
just like a standalone skill. Task Flow launches preserve the same choice.

## Select where work happens

```bash
lf --repo loopflow list             # ~/src/loopflow, without changing saved defaults
lf --repo ./another-checkout list
lf --task EXP-12 skill design       # contribute in the Task's checkout
lf --wt csv-export : "Add CSV export"
lf --wave exports : "Review the goal" # add context in the current directory
lf -b task run EXP-12       # place the Task, run its default
lf -b task run EXP-12 pursue # take the workflow edge that runs pursue
lf task run EXP-12 end       # take an edge that runs nothing
lf task move EXP-12 demo     # put the Task at a node, running nothing
lf --task EXP-12 flow incident # run a Flow without moving the Task
lf task run EXP-12 --reason "take the smaller approach"
```

`--repo NAME` selects `<repo_root>/NAME`; the Machine-local `~/.lf/config.yaml`
can set `repo_root: ~/projects` instead of the default `~/src`. Absolute paths,
`./` and `../` paths, and quoted `~/` paths select a checkout directly. No flag
keeps the current directory. Linked worktrees retain their checkout and share
canonical repository identity. Missing/non-Git paths fail without creating work.
Repository selection does not register a plan, join peers, or change saved defaults.
Read `lf repo identity` on one Machine, then use
`lf repo identity --bind repo_ID` on the peer, even when both already have Work.
Binding selects the shared repository identity and retains prior IDs as local
locators. Work IDs, provider lookup, pending edits and execution stay unchanged.
It does not connect, select, fetch or publish planning. An ID already locating
another checkout on the same Machine cannot be rebound to this one.
An explicit Task from another repository is rejected rather than silently retargeted.

[Select Git planning](#select-git-planning) to exchange planning. Import never
moves local Workflows or transfers a started Task to another Machine. Remote
first-start admission remains unavailable; absence on a peer is not a reservation.
Shared Tasks without a retained local checkout refuse first start. Local-only Tasks
and retained execution remain usable; planning saves and acquisition still work.

`--task` and `--wt` select a location. `--wave` supplies context and identity;
it cannot override a Task's owning Wave. `task run` places the Task's
worktree, defaults to its Project's Flow, then starts
`lf --task ISSUE flow FLOW`: it prints the Flow's output and returns when the
Flow ends. A Flow that fails is started again from its first step, three
times at most; one that is blocked, interrupted or waiting on a landing is
not. Background it yourself when you will not wait. It never continues
an earlier Flow. `--reason` publishes direction to the Task first. Every
Flow process for a Task is equally its work.

Flow output shows each step's position and name followed by the agent's messages.
Use `lf -v -b task run EXP-12 pursue` for context token accounting and INFO
diagnostics. Codex Session capture uses a temporary launch profile so terminal
wrapper hooks stay active. No hook setup or saved trust changes are required.

A Task takes up its Project's workflow and moves through its nodes.
Each `task run` takes one edge leaving the current node: the only one, or the
one whose Flow you name. At a node the Task waits on you in its conversation;
no command approves a node. A Flow that does not leave the node is refused
with the edges that do. A Flow that stops, or fails every attempt, leaves the Task on its edge until
you choose again or `lf task move EXP-12 <node>` puts it at a node outright. `lf task run EXP-12 code` takes up another workflow from
its start. See [workflows](authoring.md#workflows).

```bash
lf wave show product --json              # current and retained chapter planning
lf wave workflow list                 # Workflow definitions and validity
lf wave workflow set code WAVE     # selection for future Tasks
lf wave workflow source feature WAVE # read the definition
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

Claude and Codex terminal launches load the complete assembled context from a
system instructions file. The first user message is a short execution request;
Wave memory, scratch, diffs, clipboard, skills and Task briefs stay out of the
command-line argument. Provider refusals remain visible in the native terminal.

Selected Wave goals are supplied once as complete `GOAL.md` documents. Repeated
requests for the same document do not repeat its contents; distinct memory files
remain separate even when their text matches.

Launching a Flow for an existing Task uses valid cached planning regardless of age.
Known invalidation, removal, terminal state or ownership changes still block.
`--reason` requires successful Linear publication before the launch. Status
retains the planning observation's original age.

## Plan locally

```bash
lf task create --title "Fix the parser" --json
lf task edit <id> --notes "Preserve quoted input"
lf task comment <id> "Keep escaped quotes intact"
lf task checkout <id>
lf task run <id> research
```

Without a selected Wave, explicit creation uses `inbox`. A Task gets its
identity before placement; creation starts no agent and requires no Linear login.
Goals, memory, Projects and comments stay in SQLite. Reads create nothing.

```bash
lf wave ensure parser
lf wave edit parser --goal /tmp/goal.md --memory /tmp/memory.md
lf task create --wave parser --title "Retain escaped quotes"
```

Creation generates and saves a Task, then prints its ID. Every invocation creates
a distinct Task, even with the same title. Use the saved ID to inspect or edit it.
Task references accept unique UUID prefixes of at least four hex digits, bare or
prefixed with `lf-` or `task_` (for example, `abcd`, `lf-abcd`, `task_abcd`).
Hex digits are case-insensitive. Ambiguous prefixes list matching Task IDs;
add digits to select one. Full IDs and exact Linear ticket aliases still work.
Displayed local selectors start at seven digits and extend when needed; retain
full IDs in automation.

```bash
lf task edit <id> --rank 0 --assignee <person-id>
lf task edit <id> --unassign
lf wave edit-plan WAVE --name "Parser" --summary "Retain quoted input"
lf wave workflow set review WAVE --file /tmp/review.yaml
lf wave workflow list --wave WAVE --json
lf wave workflow source review WAVE
lf wave update-plan --wave parser --plan /tmp/plan.json
```

Use `--project ID` instead of a Wave on `workflow show/set/source` or `edit-plan`
to address an exact retained chapter. An explicit `wave show WAVE` includes history;
`wave show --all` reads current Waves across repositories. Neither starts work.

Project names, summaries, Workflow selections and complete plan replacements save
locally, including during a Linear outage. Connected repositories report pending sync;
`lf wave workflow show WAVE --json` includes `sync`: pending changes,
uncertain attempts, errors and retained losing values. Task edits and status use
the same receipt-backed presentation as Desktop.
Inbound refreshes preserve saves against unchanged baselines and adopt conflicting
Linear edits, retaining losing intentions in delivery history. An active Task Session
or Desktop connection exports saved Projects and delivers their fields. Lost creation
or Initiative-attachment replies retain uncertainty until exact readback.
Readback cannot protect against unseen concurrent provider writes.

Ranks are zero-based within a Project. Selected workflow definitions stay in SQLite
for every Wave; Desktop’s Customize and Edit controls use the same editor. Stored
definitions are imported by `lf wave ensure`; builtin definitions remain available.
Nested Waves use names such as `parser/tokenizer`; renaming retains their IDs,
descendants and saved documents. `lf wave ensure` imports existing Markdown and
Workflow sources without changing them. Repeating it preserves stored edits.
`lf wave edit` saves to SQLite in both connection modes.

Placement, native Sessions, workflow skills and GitHub delivery use the same Task
identity. `lf land -c` requests completion after a verified merge; `lf pr reconcile`
settles it. A repository without a code remote supports local work and refuses
hosted landing. Local planning does not require Linear at any of these boundaries.

## Select Git planning

```bash
lf planning key --new               # once; prints the user key to recover elsewhere
lf planning key --recover <uuid>    # on another machine, instead of --new
lf planning connect --remote plans # pin an explicitly chosen, controlled Git remote
lf planning use <destination-id>    # route future root Waves to the printed destination
lf planning select <destination-id> --wave <wave-uuid>
lf planning status --json
```

For retained duplicate provider identities, associate exact full IDs locally:

```bash
lf planning associate <incoming-work-id> --with <local-work-id> --linear <provider-id>
```

Both IDs remain intact. The incoming full ID resolves to existing local Work;
no execution or private history moves. **Joint planning projection is unfinished:**
association retains sharing/effect holds visible in status, not convergence.

Setup is local: these commands neither fetch nor publish planning. In local-only
repositories, interactive and headless sessions exchange selected planning, even
without a Task. Desktop's work connection does the same. Task saves and Project
edits commit locally, then attempt exchange before returning; failed exchange
leaves pending state, not a failed save. Machine dispatch publishes first; cold
Task resolution acquires before placement. No resident or turn retry runs.

Status separates fetched revisions, retained imports, pending local edits and
pending/unconfirmed/confirmed publication. Confirmation covers the recorded revision,
not future edits or held records. Each destination includes its sharing holds and
retained projection conflicts. An invalid local journal reports unknown pending
changes (`pending_local: null` in JSON), without hiding other destinations or
changing sync receipts. Status does not repair the journal. In Desktop, open **Git planning** in the
repository roadmap for the same destination, pending state and held records. Its
foreground reader updates these receipts without a manual refresh; a failed
reading keeps the last status visibly stale. Import retention is not convergence.
Git exchange
in Linear-connected repositories awaits complete provider acquisition and delivery
receipt integration; it reports that limitation without changing local plans.

Connection defaults to `refs/loopflow/planning/users/<uuid>`; `--shared <name>`
explicitly joins `refs/loopflow/planning/shared/<name>`. Joining selects no existing
work and does not change the destination for new Waves. Select existing Waves
explicitly; selection includes descendants and retained history. A private reference
holds that record and its dependents out of exchange. Selecting its referenced Wave
may release that history for sharing; moving back alone does not.

`use` affects future root Waves only. Children inherit their parent's selection,
not the currently active destination. `lf planning use local` leaves future roots
unshared without withdrawing already selected work. Existing membership survives
switches, reconnects and remote-alias edits. Selection is shared by this machine's
checkouts of the repository; execution stays local.

Ref separation is not privacy. Use a controlled remote; the code remote is never
selected implicitly. Status omits endpoint URLs because they may contain credentials.

## Connect planning and create work

```bash
lf list wave                       # authored goals, including unconnected Waves
lf account connect linear
lf repo connect --all --team-key EXP # connect goals and choose the Task prefix
lf task create --wave exports --title "Add CSV export"
lf checkout <task-id>               # use the ID printed at creation
lf task run <task-id>
lf wave show --json                   # plans and Tasks across Waves
lf wave status exports --json       # one Wave's detailed evidence
```

Connected planning needs a Linear login and repository Team. Connection names
missing access and the command to obtain it. Authored Waves remain discoverable
before connection. Planning setup is separate from direct local execution.

Task creation and edits save locally, including during a Linear outage. Creation
generates the Task ID; later edits, Project rotation and synchronization preserve it.
Connected repositories report pending sync. Field receipts retain titles, notes, assignment and ordering
with their provider baselines; observed conflicts adopt Linear and retire the losing
intention. An active Task connection delivers mapped titles, notes, assignment,
membership, deletion, Project fields and Task ordering after recovery. Reordering
saves one Project-wide intention; partial delivery and lost replies retain that identity. Deletion retains execution
and checkout history. Missing provider evidence keeps deletion pending; newer
observed Linear edits retire removal and restore planning visibility. The active
connection exports unmapped Projects and Tasks using their saved UUIDs; lost replies
never allocate replacement identities. Later local edits survive creation readback.
The local ID works before a Linear alias arrives.

```bash
lf task refile <task-id> --wave exports
```

Move unplaced work to the Wave's selected Project. Membership saves locally during
an outage and retains pending sync; recorded work keeps its owner. An active Task
Session or Desktop connection delivers the move when both Task and destination
Project have Linear mappings. The same active connection exports unmapped records.

## Inspect and continue

```bash
lf ps                              # one live process snapshot
lf top                             # refresh on a terminal
lf history                         # durable Work, delivery and steering facts
lf history list --json              # bounded Process history with a next cursor
lf mon active --watch --json        # NDJSON until stdin closes
lf history show SESSION --final     # provider conclusion
lf usage --days 30                  # measured consumption and missing evidence
lf usage --task LOO-265 --context   # each step's input by source, flagged over budget
lf history show SESSION --context   # one step: instructions, memory, scratch, goal, steers, carried, tools
lf session ensure                   # this repository's one ongoing conversation
lf session ensure -w growth         # a Wave's one ongoing conversation
lf session ensure --task EXP-12     # a Task's primary conversation
lf --task EXP-12 skill task-session # another conversation about that Task
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
| Wave | `lf --wave growth wave-operate` | `lf session ensure -w growth` |
| Task | `lf task-operate EXP-12` | `lf session ensure --task EXP-12` |

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
lf pr publish                         # push a ready PR
lf submit                          # prepare for a reviewer's merge click
lf arm                             # prepare and request auto-merge; return
lf land                            # record delivery and return
lf --task EXP-12 land               # complete after verified merge
lf sync --plan                     # preview integration with main or stack parent
lf wt create csv-export
lf check                   # inspect release eligibility
```

Choose one delivery operation for the desired endpoint. Submit, arm, and land
own preparation and integration; publish does not sync. PR operations work
on ordinary branches without creating a Task. Verified merge normally completes its Task. Arm and land return after recording delivery;
`lf pr reconcile` checks it once and settles verified merges. In a Task checkout,
it also recovers an existing PR whose GitHub identity is missing from the Task,
without publishing or rotating the branch. Multiple PRs
for the recorded branch remain unresolved. `lf ci watch`
starts a ci-fix when a recorded landing fails its required checks.

```bash
lf task follow-up EXP-12 --outcome 'Installed latency meets the budget' --evidence 'Warm p95 below 1s over 20 samples' --check-at 2026-10-09T17:00:00Z
lf land
lf task follow-up EXP-12 --clear 'Published v0.14: 20 samples, p95 0.8s'
```

Record accepted remaining work before delivery. Its outcome, evidence condition
and next check stay visible in Task status and Desktop. An overdue check calls
for evidence or a scope decision; time and green CI never establish production
success. `--next <slug>` keeps genuinely unfinished PR work open. Older keep-open
requests without a stated outcome surface for an explicit scope decision.

`lf task move EXP-12 end` records an explicit completion. Old Session turns,
reserved inputs and unknown process exits cannot veto it. Execution history and
live process controls remain intact; uncertain or occupied checkouts are retained.
`lf task abandon EXP-12` saves cancellation locally, including while Linear is
unavailable. Retry retains its decision and delivery identity. An active connection
delivers cancellation from that saved receipt; unavailable Linear states remain
pending. Open PRs and additional committed work still need delivery or explicit
abandonment.

Completing an uncached Linear issue first retains its Task locally, without a
checkout. The completion reason stays in Task history; retries reuse that identity
and preserve the recorded outcome while provider writeback is pending.

Completion, reopening and cancellation save locally before contacting Linear.
An active Task Session or Desktop connection delivers pending decisions independently
of comments and incoming planning. Status and Desktop retain pending synchronization feedback.

```bash
lf task sync EXP-12                     # attempt pending delivery once
```

Observed conflicts adopt Linear automatically. The losing local state or comment
remains in delivery history, and its delivery stops. An unchanged Linear baseline
preserves pending local saves. Comment threads display Linear's current text beside
the saved local body. Planning synchronization never moves the local Workflow.
The separate provider read and write cannot prevent an unseen concurrent edit.

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
watcher running: without it, failures are recorded and wait.
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
lf --account codex=work@ flow pursue
lf --only-account codex=work@ flow pursue
lf account route set codex work@ personal@
```

A preference permits fallback; a restriction limits spending. A Flow takes
one selection per provider and carries it through its child steps. Remote
launches install missing selected logins in the foreground and use the same
account on the target. Credentials remain resident there.
Missing or expired capacity remains unknown, never zero or unlimited.

## Discover commands

```bash
lf help
lf help --all
lf help feature
lf list skill
```

Use shortcuts: `lf land`, `lf ps`, and `lf history list`.
Omit owners or abbreviate command names when the result is unique. Ambiguous
shortcuts list their matching command paths. Exact commands take precedence
over authored definitions; `lf skill NAME` and `lf flow NAME` select a kind.

Help and catalog reads launch no agent. The [command reference](lf-reference.md)
names canonical owners and arguments. [Authoring](authoring.md) explains workflows;
[configuration](config.md) covers inherited launch defaults.

## Preview without launching

```sh
lf implement --context --json
lf flow pursue --context --json
lf implement --context --explain
lf task run LOO-427 --explain --json
lf --task LOO-427 desktop open --diff --explain --json
```

`--context` assembles a skill or inline request's input from local sources using
ordinary launch settings. JSON includes system/task prompts, native skill arguments,
budget accounting and complete bytes for excerpt sources that were **not written**.
It never prepares a checkout, refreshes planning, writes storage or runs a provider.
A real launch refreshes Task direction, so parity applies to the same snapshot.

For a Flow, preview shows the compiled graph and input states keyed by node.
Only the initial skill or router gets current-snapshot input. Later nodes and
repeat passes are unavailable: files, Work, feedback and branch choices may change.
Command nodes are labeled `not_agent`, never executed to discover later input.
This is a fresh-start preview, not a resumed execution. Native skill arguments and
router instructions use the same owners as launch. If an operator would prepare a
scope checkout, its input stays unavailable until previewed from that checkout;
CLI aliases and captured Flow steps use the same resolved-skill decision.

`--explain` reads selected Work without executing the invocation. Task commands,
Desktop opening, Session connection and exact Process history lookup are supported;
other command shapes fail before effects. Combine both flags for identity plus input.
`task run --explain` also reports the intended Workflow edge (or ad-hoc Flow),
selected execution Machine, observed impediments and missing evidence. Omit the Task
to use its checkout or `--task`. JSON separates `resolution`, `action`, `impediments`
and `unavailable`; no Workflow is taken up or moved. Started Tasks use recorded
execution; unstarted Tasks show effective delegation as a **proposed destination**,
not proof of peer-exclusive first start. Remote execution state is unavailable
without reading that Machine. Completion, future checkout and provider checks are
not fabricated; a real run checks them again.

`task checkout [TASK] --explain` reports a proposed preparation, reuse or restoration
of the recorded checkout, with the preparation owner's planning and path checks.
Omit TASK to use `--task` or checkout inference. JSON separates `resolution`,
`action`, `impediments` and `unavailable`. Local path/branch proposals reserve
nothing: previews never fetch, allocate a checkout, take a lease, change a stack
or contact a provider. Missing branch recovery and finalization remain unperformed;
shared first-start admission stays refused.

`task move TASK NODE --explain` and `task workflow restart TASK --explain`
validate the captured Workflow without moving it. JSON adds the intended `action`
(including the exact prior node/edge, reason and force choice), `impediments` and
`unavailable` to `resolution`. Restart means move to `start`, not run a Flow.
Moving to `end` describes completion and possible checkout cleanup but leaves PR
reconciliation and completion gates explicitly unperformed; it grants no permission.
Missing or remote execution evidence stays unavailable.

`task create/edit/comment/refile/save --explain` validates mutation input through the same
owners as saving. JSON separates `resolution`, intended `action`, `effects`,
`impediments` and `unavailable`. Creation reads the current Project and derives a
title from piped notes normally; new Task identity and execution stay unallocated.
Editing reports fields and the observed revision, preserving empty descriptions
and explicit unassignment. Comment explanation distinguishes appending from reading:
a connected thread read would refresh Linear, but preview never does.
Missing registry, cold acquisition, Wave/Project initialization, comment provenance
and synchronization delivery remain explicit uncertainties. No preview saves,
imports, registers, refreshes or synchronizes anything; execution validates again.
Refiling reports the destination Wave/current Project and recorded-work or Machine
placement restrictions without registering or locking anything. File-save preview
reads the draft from stdin, reports its byte count and expected revision, and uses
the save owner's path, text and pinned-file checks. It writes neither files nor
recovery receipts. Concurrent changes and publication permissions remain unreserved.
Other Task commands still explain identity only, not their mutation arguments.

`desktop open --explain` reports `resolution`, the proposed `url`, `impediments`
and `unavailable` evidence using the same Work and opening checks as launch.
It includes `--session` and `--diff`, and reports unsupported platforms without
opening the app. Display stays on the command's Machine, separate from Task
execution. The URL is not a window reservation, readiness receipt or input target;
installed-app and retained-window state remain uninspected.

`session connect SESSION --explain` reports start, native resume or possible live
connection, replacement/try intent and the shared Session actions.
`session resume --explain` selects the latest interactive conversation in the checkout, just like
ordinary resume. JSON adds `action`, `state`, `actions`, `impediments` and
`unavailable` to `resolution`. `session connect --json` normally prepares launch
arguments without takeover; its explanation preserves that distinction. Live
endpoints are not probed, unrecorded native conversations are not admitted, and
missing workspace/provider/account evidence stays explicit. Nothing acquires or
signals a client, prepares a checkout or launches a provider.

Use `lf --machine mini --repo project debug --context --explain --json` to preview
on an added Machine. The receiving CLI checks its identity read-only and resolves
its own repository, Work and input. Preview skips provider/account preparation,
planning refresh and Process recording on both Machines. Missing identity or
transport fails explicitly, with no local execution fallback; missing Work remains
unavailable evidence. This observes the selected Machine, not peer-exclusive start
permission. Flow-wide input is still undefined. Bare-agent and new operator-checkout
input is not predicted by creating Work. `lf context` remains the budget report.

## Desktop navigation

```sh
lf task run LOO-427 --explain --json
lf --explain                                # resolve the current checkout
lf session connect SESSION_ID --explain
lf history show PROCESS_LFID --explain
lf --task LOO-427 desktop open --diff --json  # conversation plus Changes
lf desktop open --session SESSION_ID --diff  # exact Task-associated Session
lf desktop list --json                       # read an already-running Mac app
```

Explanation reads local records without allocating a checkout, creating a plan,
starting a provider or contacting peers. Each identity is bound (with its source),
unbound or unavailable. Machine identifies the reader; Execution Machine comes
from recorded checkout evidence, never delegation substituted for a missing
execution observation. The timestamp dates this read, not planning freshness or
permission to start. Process checkout resolution describes current Work at that
location, not historical usage attribution. Ordinary `lf context` still reads
prompt budgets.

`desktop open` resolves Work before handing its repository-qualified link to the
Mac app. Without a Task it opens the current repository. `--session` selects an
existing Task-associated conversation; `--diff` reveals its retained Changes
browser without replacing file selection or drafts. Without `--session`, the
Task's ordinary primary-Session owner chooses/prepares its conversation.

JSON returns `opening`, not `usable`: macOS accepting a link proves no native
endpoint. `desktop list --json` includes each window's latest Task `opening`
reading and lookup failures. Native usability and connection/comparison failure
receipts are not yet composed; an `opening` receipt must not authorize pane input.
Display opening does not automatically route to the Task's execution Machine or
prepare its checkout in the CLI. Bare `open` is ambiguous with `pr open`;
Sessions use `session connect`.

Desktop inspection reports registered repository/window identities, selection,
reading availability and retained Machine/checkout pane trees with focus, zoom
and hidden panes. `task` forwards Rust's recommendation and `run_control`;
`session` forwards its actions, including unavailable reasons and takeover warnings.
Each has its own reading state: a Session failure cannot invalidate current Task
evidence, or vice versa. Saved, failed and absent subjects supply no actions.
Task readings retain roadmap-generation and condition-observation dates. Session
observation time is explicitly unavailable; the UI snapshot time is not source
freshness. Inspection neither focuses windows nor opens clients. Terminal panes include a `surface`
incarnation only when a native surface exists. It is separate from the pane's
content incarnation; inspection never creates a surface or cleans up an exited
child. Neither token authorizes terminal input.
It requires macOS and a running app with automation access; failure leaves
`lf task status <task>` available in the terminal.

Hide and restore an exact retained pane, without closing its Session or shell:

```sh
lf desktop list --json > /tmp/desktop.json
# Set pane to a pane ID from that reading.
target=$(jq -c --arg pane "$pane" '
  .windows[] as $w | $w.workspaces[] as $s |
  $s.layout | .. | objects | select(.pane? == $pane) |
  {repository: $w.repository, window: $w.window, machine_id: $s.machine_id,
   worktree: $s.worktree, pane, incarnation}
' /tmp/desktop.json)
lf desktop hide --target "$target" --json
lf desktop restore --target "$target" --json
```

The target stays attached to that window, Machine, checkout and content occurrence
while focus changes. Replaced windows/content reject the old target. Restore removes
only the hidden flag: it does not select a Task, leave zoom, focus a window or acquire
a client. Hidden shell commands and native surfaces remain retained. If a reply is
lost, inspect again; repeating hide or restore is safe.

Arrange those retained panes through the same exact targets:

```sh
lf desktop focus --target "$target"
lf desktop split --target "$target" --axis vertical
# Extract another exact target into $other from the same workspace reading.
lf desktop move --target "$target" --destination "$other" --axis horizontal
lf desktop resize --target "$target" --toward "$other" --ratio 0.6
lf desktop zoom --target "$target"
lf desktop zoom --target "$target" --off
```

Vertical puts the new or moved pane to the right; horizontal puts it below.
Split adds an empty pane. Move retains the original pane/content identity, within
one Machine/checkout only. Resize changes the divider separating the two targets;
0.6 gives the target's side 60%, regardless of its order (allowed range 0.1–0.9).
Both targets are checked before a move or resize changes anything.
Split, move, resize and zoom preserve pane selection and Work navigation. Focus
reveals/selects the exact pane in its retained workspace; it does not select
another Task or bring a background repository window forward. Zoom reveals its
target without selecting it; `--off` only unzooms that target. These operations
never close/reopen clients or write into a draft. Replies describe model state,
not a completed render. A lost split reply is not safe to retry blindly: inspect
first, since each split adds a pane.

Add companions without changing focus, zoom or Work selection:

```sh
lf desktop shell --target "$target"
lf desktop files --target "$target" --task "$task_id"
lf desktop flow-log --target "$target" --task "$task_id"
```

Shell adds a local shell (remote opening is unavailable). Files and Flow-log
reuse the Task's existing pane or add one beside the target; `--task` is its
Loopflow Task ID from inspection, and its recorded checkout must match.
Existing documents and terminal drafts stay retained. An empty target is filled;
occupied contents are never replaced. Repeating Files/Flow-log reveals the same
pane; repeating Shell adds another shell, so inspect after a lost reply.

Request text from that exact pane and native surface:

```sh
# Set surface to this pane's surface incarnation from the same inspection.
lf desktop read --target "$target" --surface "$surface" --region screen --max-bytes 65536 --json
```

Regions are `screen`, `scrollback` and `selection`; the byte limit is 1–1048576
(default 65536). The reply echoes the request, observation time and whether the
pane is collapsed or hidden by zoom. Available results carry `text` and
`truncated`; an empty string is successful empty output. Unavailable results carry
a reason instead of text. Replaced windows, content or native surfaces reject old
targets. Reads never follow focus, acquire a client or clean up an exited surface.

`screen` reads the current viewport; `scrollback` includes retained history;
`selection` reads the selected text or shell command block, or empty text when
nothing is selected. Extraction writes directly into fixed storage and ends at a
complete UTF-8 scalar. Absent surfaces return `missing_surface`, and nonterminal
panes `not_terminal`. JSON preserves these outcomes; text mode reports unavailable
reads as errors. There is no unbounded fallback. Reading alone never sends input.
Native pane acceptance and remote Session-plus-diff composition remain unproved.

Insert text into the exact surface, then submit deliberately:

```sh
lf desktop text --target "$target" --surface "$surface" -- 'literal text'
lf desktop key --target "$target" --surface "$surface" enter
```

Text inserts at the current cursor without clearing the existing draft. Quotes,
backslashes and Unicode are literal; control characters (including newline and
Tab) are rejected, never interpreted as keys. Key accepts `enter`, `tab`, `escape`,
`backspace`, `delete`, `left`, `right`, `up`, `down`, `home` and `end`.
Neither operation changes focus or opens a client. Missing, exited or replaced
surfaces reject input, as does an active IME composition. Complete that composition
first; the draft is not discarded. A reply reports dispatch, not child consumption
or Task completion. Input is not idempotent: after a lost reply, inspect before
sending more; never automatically repeat it. Native interaction proof remains
with the fixture demo.

```sh
open 'loopflow://task/LOO-303'
lf wave show --task LOO-303 --all --json
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
lf machine connect mini codex work@example.com
lf --machine mini --repo project session list
```

`--machine <label-or-id>` runs the entire command on that machine. Commands without
a Work or repository target use its saved repository; `machine add` sets that default.
Names and quoted `~/` paths resolve on the selected Machine, using its own
`repo_root`; they are not expanded on the caller. Explicit `./` paths are relative
to the remote login directory. `--forward-agent` requires `--machine`.

`lf repo identity` establishes or prints the selected repository plan's opaque ID
(`--json` returns `id` and `locators`, the retained Machine-local IDs). It also works before creating any Waves
or Tasks. `--repository ID` selects that registered plan's local path, including on
an explicitly selected Machine. Neither operation joins plans by code remote or
changes a Machine's saved repository.

Known Task launches resolve recorded checkout Machine before delegation. The
cross-machine path requires the same repository plan to be explicitly bound at
the destination. Peer selection/exchange and complete remote execution observation
are still under development; missing local execution is not proof of globally
unstarted work.

Save an SSH destination once, then use its label. List, rename and remove saved
connections with `lf machine list`, `lf machine rename mini builder` and
`lf machine remove builder`. Removing a connection leaves remote work running.
Interactive add offers to install a missing `lf`; an existing installation stays
untouched. Status reports connection failures and recovery commands without prompting.
Connections are reused for 60 idle seconds through Loopflow's private SSH socket.
See [machine connections](architecture/machines.md) for repository paths and version checks.
