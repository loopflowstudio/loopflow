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
lf task run LOO-438 pursue -a claude:opus
```

`--agent` / `-a` is global: place it before or after any command. It selects
`harness[:model]` for the entire invocation, overriding agents named in Tasks,
Flows and skills, including nested Flows and child `lf` commands. Without it,
definitions and defaults apply. Put literal skill arguments after `--`.
Resuming native history keeps its provider; changing providers requires a new Session.

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

```bash
lf portfolio --json                    # immediate main checkouts under ~/src
lf portfolio folder ~/projects         # change discovery and named-repo lookup root
lf portfolio manual                    # use only saved individual repositories
lf portfolio add ~/src/loopflow ~/other/nested/repo
lf portfolio remove ~/other/nested/repo # remove membership, keep files and Work
```

Portfolio is Machine-local. Folder discovery excludes linked worktrees; manual
additions normalize them to their main checkout. Switching modes keeps the inactive
selection. Missing repositories remain listed with an error and can be removed.
Opening a repository does not enroll it. Desktop's Portfolio window edits the same
selection, including an explicit import of old Desktop membership before shared
configuration exists. `lf portfolio import PATH…` performs that one-time import;
an existing shared selection takes precedence.

Read `lf repo identity` on one Machine, then use
`lf repo identity --bind repo_ID` on the peer, even when both already have Work.
Binding selects the shared repository identity and retains prior IDs as local
locators. Work IDs, provider lookup, pending edits and execution stay unchanged.
It does not connect, select, fetch or publish planning. An ID already locating
another checkout on the same Machine cannot be rebound to this one.
An explicit Task from another repository is rejected rather than silently retargeted.

[Select Git planning](#select-git-planning) to exchange planning. Import never
moves local Workflows or transfers a started Task to another Machine.

```bash
lf task admission TASK --json    # inspect its first-start coordinator and owner
lf task checkout TASK            # claim first execution before allocating files
```

A new Task keeps its creating Machine as its first-start coordinator. Checkout
and run request one permanent execution owner there. Concurrent requests and
retries recover the same owner; later delegation cannot transfer that reservation.
Planning exchanges the coordinator identity, never the execution claim. Both
Machines need their configured connections; an unavailable coordinator refuses
before checkout allocation. A confirmed claim needs no renewal. Historical shared
Tasks without an established coordinator still refuse; local absence is not a claim.
`task admission TASK --claim MACHINE_ID` explicitly reserves first execution
without preparing a checkout. There is no automatic expiry, takeover or failover.

`--task` and `--wt` select a location. `--wave` supplies context and identity;
it cannot override a Task's owning Wave. `task run` places the Task's
worktree, defaults to its Project's Flow, then starts
`lf --task ISSUE flow FLOW`: it prints the Flow's output and returns when the
Flow ends. A Flow that fails is started again from its first step, three
times at most; one that is blocked, interrupted or waiting on a landing is
not. Background it yourself when you will not wait. It never continues
an earlier Flow. `--reason` publishes direction to the Task first. Every
Flow process for a Task is equally its work.

`lf task run EXP-12 --name fix` selects its first checkout name. Names use the
same normalization as `lf wt create`; single words and longer names are accepted.

Flow output shows each step's position and name followed by the agent's messages.
Use `lf -v -b task run EXP-12 pursue` for context token accounting and INFO
diagnostics. Codex Sessions start through the provider protocol and attach the
native terminal. Prompts and instructions stay off the command line. Conversation
capture requires no hook approval; context refresh uses stable launch hooks.
Review new or changed declarations with Codex’s `/hooks`; headless runs skip
untrusted hooks. Loopflow never writes provider trust decisions.

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

A Flow whose process died leaves its Processes as history. No command resumes it. To change direction or recover:

```bash
lf task interrupt EXP-12             # end the active provider turn
lf task status EXP-12                # the latest Flow and all Task work
lf flow show ID --processes --json    # one Flow's steps, by its Flow process ID
lf -b task run EXP-12       # run fresh work
```

The first turn contains the skill followed by the request and tagged clipboard
text. Loopflow adds fixed instructions without replacing the provider's native prompt or repo guide.
Startup and compaction deliver current Wave memory and scratch as a capped
conversation block: whole files, marked start excerpts and readable source paths.
Compaction also restores the saved active skill. Task direction and summaries
remain complete private reference files. Codex terminal first turns travel through
its protocol after the terminal connects.
Claude reads its first turn from stdin, then uses the terminal for keyboard input.
OpenCode first turns still use argv; a first turn exceeding its argument cap is
refused before spawning the provider.
OpenCode launches and resumes print an `OpenCode diagnostics:` path before opening
the terminal. Full stderr is retained in that private `opencode.stderr.log` file
beside the Session's capture. Read or tail it from another terminal; native
errors and retries remain visible in OpenCode without raw logs covering input.

Interactive startup shows the current preparation stage and elapsed seconds.
The indicator clears before the native terminal opens; redirected output stays clean.

Wave goals use the same whole-file, excerpt and listing rules as memory.
Distinct source files remain separate even when their text matches.

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

Restore a lost Task branch only after locating its committed history:

```sh
lf task checkout EXP-12 --recover-from COMMIT
```

Recovery uses the recorded checkout and branch without starting a Flow or
changing Task identity. It never replaces an existing branch or checkout with
a different commit. Without an explicit recovery commit, a lost started branch
is not recreated from its old base.


Without a selected Wave, explicit creation uses `inbox`. A Task gets its
identity before placement; creation starts no agent and requires no Linear login.
Projects and comments stay in SQLite; Wave goals and memory live in the checkout.
Reads create nothing.

```bash
lf wave ensure parser
# Edit .lf/wave/parser/GOAL.md and .lf/wave/parser/MEMORY.md directly.
lf task create --wave parser --title "Retain escaped quotes"
```

Creation generates and saves a Task, then prints its ID. Every invocation creates
a distinct Task, even with the same title. Use the saved ID to inspect or edit it.
Task references accept unique UUID prefixes of at least four hex digits, bare or
prefixed with `lf-` or `task_` (for example, `abcd`, `lf-abcd`, `task_abcd`).
Hex digits are case-insensitive. Ambiguous prefixes list matching Task IDs;
add digits to select one. Full IDs and Linear ticket aliases still work.
Ticket aliases are case-insensitive: `loo-498`, `Loo-498`, and `LOO-498`
select the same Task. Output retains the recorded spelling.
Displayed local selectors start at seven digits and extend when needed; retain
full IDs in automation.

Task status, edits and comments resolve ticket aliases within the current repository,
including its worktrees and path aliases. An explicit full Task ID selects its
recorded repository; outside a repository, ticket aliases must be globally unique.

```bash
lf task edit <id> --rank 0 --assignee <person-id>
lf task edit <id> --unassign
lf wave edit-plan WAVE --name "Parser" --summary "Retain quoted input"
lf wave workflow set review WAVE --file /tmp/review.yaml
lf wave workflow list --wave WAVE --json
lf wave workflow source review WAVE
lf wave edit-plan WAVE --description "Preserve quoted text across machines."
lf wave update-plan --wave parser --plan /tmp/plan.json
```

Use `--project ID` instead of a Wave on `workflow show/set/source` or `edit-plan`
to address an exact retained chapter. An explicit `wave show WAVE` includes history;
`wave show --all` reads current Waves across repositories. Neither starts work.

Project names, summaries, Workflow selections and complete plan replacements save
locally, including during a Linear outage. Connected repositories report pending sync;
`lf wave workflow show WAVE --json` includes `sync`: pending changes,
uncertain attempts, errors and retained losing values. Project reads keep the local
`id` in both transports, with an optional `linear_id` binding; adding or removing
that binding does not change identity. Task edits and status use
the same receipt-backed presentation as Desktop.
Inbound refreshes preserve saves against unchanged baselines and adopt conflicting
Linear edits, retaining losing intentions in delivery history. An active Task Session
or Desktop connection exports saved Projects and delivers their fields. Lost creation
or Initiative-attachment replies retain uncertainty until exact readback.
Readback cannot protect against unseen concurrent provider writes.

Ranks are zero-based within a Project. Selected workflow definitions stay in SQLite
for every Wave; Desktop’s Customize and Edit controls use the same editor. Stored
definitions are imported by `lf wave ensure`; builtin definitions remain available.
Nested Waves use names such as `parser/tokenizer`. Edit `.lf/wave/<name>/GOAL.md`
and `.lf/wave/<name>/MEMORY.md` like any other file; the next launch reads that
checkout's files, including ancestor context. Creation writes missing files without
overwriting existing content. Registry renaming retains IDs and descendants;
move the authored directory separately to match its new address. Workflow imports
remain independent of goal and memory files.

Placement, native Sessions, workflow skills and GitHub delivery use the same Task
identity. `lf land` requests merge; `lf pr reconcile` observes delivery. After
merge, optional new work uses ordinary Task creation;
`lf task complete <task-id>` completes the source. A repository without a code
remote supports local work and refuses hosted landing. If it has no `main` branch,
new checkouts use its current branch. Local planning does not require Linear.

Delivery reconciliation skips deleted checkouts and retains their Task, PR and
merge intent. It continues checking available work without recreating directories
or treating missing local evidence as completion.

## Choose planning synchronization

Planning defaults to your Git ref on `origin`. Select a shared ref or Linear in
`.lf/config.yaml`, or set a personal default in `~/.lf/config.yaml`:

```yaml
planning:
  provider: git
  remote: origin
  shared: team
```

```yaml
planning:
  provider: linear
linear:
  team: <team-id>
```

Repository `planning` replaces the personal selection as a whole. Omit `shared`
for `refs/loopflow/planning/users/<user-key>`; shared names use
`refs/loopflow/planning/shared/<name>`. Existing `linear.team` selects Linear
when `planning` is omitted. Every machine uses the selected transport independently.
Linear unavailability leaves saves buffered locally; it never switches to Git. All Tasks in a Linear
repository are queued for publication, including Tasks created offline. Retry a
specific Task with `lf task sync <task>`: it publishes the saved Project/Task
identity before synchronizing state, fields, and comments. An uncertain creation remains
pending instead of silently creating a duplicate. To deliberately retry an
uncertain issue creation, use `lf task sync <task> --retry-creation`. This reuses
the saved issue ID and first checks for an existing issue; it can recreate a
previously deleted issue. Ordinary sync never makes that choice.
For an uncertain saved state or field update, `lf task sync <task> --retry-updates`
retries that saved intention after reading Linear again. Observed conflicts still
win; this flag does not retry deletion or allocate another issue.
`lf repo connect <wave>` saves the repository Team and selects Linear, overriding
a personal Git preference.

This build implements the enrollment baseline below. LOO-412 replaces it with one
configured transport per repository; that unmerged configuration path is not
included here. Manual per-Wave enrollment is superseded direction, and mixed
Linear/Git exchange is not a target. `lf planning status` and key recovery remain.

```bash
lf planning status --json
lf planning key                    # show this machine's personal planning key
lf planning key --recover <uuid>   # reuse it on a fresh second machine
```

The first personal Git connection creates the key. Recover the same key before
starting personal synchronization on another machine. Shared refs require no key.
Ref separation is not access control; choose a remote appropriate for the plan.
Configuration selects the repository's entire plan, including retained history.
Changing configuration retains old receipts and history; it does not undo earlier
publication or transfer uncertain provider attempts to another machine.

Commands save locally, then attempt synchronization. Interactive and headless
Sessions and Desktop's Work connection keep planning current while connected.
Cold Task lookup acquires planning before placement. There is no resident or
automatic turn retry. Status separates fetched revisions, retained imports and
pending, unconfirmed or confirmed publication. Retained alternatives can be copied
into ordinary edits. Endpoint URLs are omitted because they may contain credentials.

Only planning travels: Tasks, Project content, comments and planning disposition.
Workflows, Sessions, Processes, checkouts and machine control stay local. A completed
plan does not move another machine's Workflow or clean its checkout.

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
`lf task create --json` returns the saved Task, with its local `id`, `project_id`,
`title`, `description` and optional `linear_id` binding, not a Linear issue.
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
lf history show SESSION --context       # input cost by source
lf history show SESSION --instructions  # why guidance was included or omitted
lf connect                         # this repository's one ongoing conversation
lf connect --wave growth            # open a Wave's primary conversation
lf connect --task EXP-12 --prepare  # prepare a Task conversation without launching
lf --task EXP-12 skill task-session # another conversation about that Task
lf connect SESSION                 # continue this exact conversation
lf connect --fresh                 # fresh conversation for the same scope
lf context --task EXP-12 --json     # effective context limits, sources and usage
lf wt timing                        # how long real `lf wt list` runs took here
```

`lf connect` opens the repository, Wave or Task's ongoing conversation for this
checkout, creating one when absent. `resume` is an exact alias. Explicit `--repo`,
`--wave`, `--task`, or a Session ID overrides the checkout's scope. Unassociated
linked checkouts require an explicit target; ordinary `lf` starts a conversation
there. First creation uses the matching scope's session skill, including its
operating instructions. Reopening keeps those instructions and native history.

`connect --fresh` selects a new primary in the same scope and checkout. The previous
client and history survive. Each explicit fresh request creates another; ordinary
connect recovers the current primary after an uncertain reply. Fresh accepts scope
selection, not a Session ID. First-use trust or sign-in prompts stay open until
answered or canceled.

`connect --prepare` and `resume --prepare` print a terminal command without
launching the provider. Add `--json` for connection data. Remote preparation
returns an SSH command in either format.
Preparation cannot be combined with `--message`.

A Task keeps its selected primary. Opening an exact Session does not change it.
Without a primary, connect selects the most recently used eligible conversation, or creates one in
the Task's checkout when none exists. The others stay open, and listing Sessions
never picks or starts one.

Use `lf connect` in the Task checkout, or `lf connect SESSION`,
to reopen the same conversation after a failed terminal launch. Retained native
history survives retries. A recorded interactive provider observed dead can reopen
without manual repair; a live or unknown provider cannot be started twice.
Explicitly connecting to a replaced Session preserves its history and leaves
the successor as primary, even when the old launch never acquired native history.
Interactive Sessions with a retained execution owner reconnect to that terminal
without restarting the provider. For headless Claude and OpenCode, connect stops
the run and resumes its saved native conversation; it is not live takeover.
Codex retains its live connection. Missing process or conversation identity needs
recovery before a headless run can be stopped and resumed.

Session connect, rename and bind take the durable Session ID shown by
`lf session list`. Capture keys and history prefixes select retained inputs for
inspection and replay; they do not select these Session actions.

| Scope | One finite pass | Ongoing conversation |
| --- | --- | --- |
| Repository | `lf operate` | `lf connect` |
| Wave | `lf --wave growth wave-operate` | `lf connect --wave growth` |
| Task | `lf task-operate EXP-12` | `lf connect --task EXP-12` |

Each conversation is its scope's operator and carries the matching operate
procedure. It leaves running Flows alone, reads a stopped or failed Flow before
running its remaining work fresh, and names what waits on you, including a Task
at a workflow node. Unstarted backlog stays unstarted. A conversation acts
only during a turn: between turns, running Flows and any installed background
checks continue on their own.
New commands, hooks and Flow children select the current CLI through the stable
installed entry. Retry a failed command in the same Session and checkout.
Installation does not restart existing processes; they may fail naturally.
Clients launched before this behavior can retain old executable paths. After
their provider exits, use `lf connect SESSION` through the installed
`~/.local/bin/lf` to reopen the same native history with fresh hooks. Reattaching
to a live provider retains its environment. Inspect failed Flow history and
effects before selecting remaining work; do not repeat uncertain steers.

An open conversation keeps the instructions it launched with. To start a new
conversation with current instructions, use `lf connect --fresh` in its scope.

Monitor keeps live processes, recorded outcomes and missing observations distinct.
A process has a durable `id` and an optional Unix `pid`. Inspect by `id`; PIDs
can be reused. `parent_lf_process_id` names the recorded parent, and historical
rows without PID evidence keep `pid: null`.
Its overview explains each item's state and next action. A mechanical Process has
no provider conclusion. JSON reads emit one document; the active watch emits
newline-delimited snapshots. Progress and errors go to stderr.

`lf context` measures authored memory and scratch against size targets; [configure context budgets](config.md#context-budgets)
in existing personal, repository or Wave settings.

## Publish and finish

```bash
lf pr publish                      # push a ready PR
lf submit                          # prepare for a reviewer's merge click
lf arm                             # prepare and request auto-merge; return
lf land                            # record delivery and return
lf land --wait-and-fix              # wait for merge and repair failing CI
lf sync --plan                     # preview integration with main or stack parent
```

```bash
lf pr request-review -r alice -r acme/platform  # request teammates on this PR
lf pr reviews 42                               # inspect a teammate's PR
lf pr reviews --json                           # provider review data and commit IDs
lf pr checks --required                        # repository-required checks only
```

Review requests, assignments, approvals and merge permission are separate.
`publish` makes work reviewable; `request-review` requests or re-requests people
or teams on that existing PR. `submit` prepares a manual merge without changing
assignments or review requests. Teammates can review and merge on GitHub without
installing lf. `reviews` shows GitHub's aggregate decision and each submitted
review's commit; an older approval is not proof of approval of the current head.
Inline threads remain on the PR page. `--json` passes through GitHub CLI's review
fields, not a stored lf review model. Provider errors remain errors.

`checks` reports all checks by default, including external CI reported to GitHub.
`--required` selects the host's required checks. Exit 8 means pending; it is not
a test failure. `--logs` retrieves failed GitHub Actions logs; other CI retains
its check link. Local test success never substitutes for remote checks or review.

Auto-merge follows [repository merge policy](config.md#merge-policy) and leaves
protection and queue enforcement to GitHub. Manual review is a normal endpoint.

A Task shows its current PR and retains earlier PRs in history. Choose a delivery operation for the desired
endpoint. Submit, arm and land own preparation and integration; publish does not
sync unrelated upstream changes. Continued publication automatically carries work
past already-merged history, including squash merges. PR operations also work on
ordinary branches without creating a Task.
Arm and bare land return after recording delivery. `--wait-and-fix` observes and repairs
its PR every 15 seconds until verified merge, closure, a concrete blocker or
interruption, including with Desktop closed. Elapsed time never ends the wait or
triggers a rerun. Interruption leaves the intent intact. Repeating land for a merged
Task PR with no new work succeeds and reports whether Task completion remains.
With new work, the same command prepares its next PR.
`lf pr reconcile` checks recorded landings once. In a Task checkout it can also
recover an existing PR whose GitHub identity is missing, without publishing.
Ambiguous provider evidence remains unresolved. `lf ci watch` repairs actionable
CI failures while it runs.

```bash
lf land --wait-and-fix
# Optional: register more work with ordinary task create.
lf task complete EXP-12
```

Verified merge leaves an open Task available for optional follow-up work. New
outcomes are best captured as follow-up Tasks. Continuing in this checkout also
works: publish again to select a new PR while retaining the previous delivery.
A completed Task reopens when the new publication begins. No filing, disposition
or “none needed” record is required. Preserve useful findings in the PR before cleanup of added scratch files.

`ship` runs gate and waited landing. Reaching its Workflow endpoint requests
completion. Bare landing or a manual GitHub merge alone does not complete the
Task. If completion fails after arrival, retry completion without replaying gate
or re-arming the merged PR.

`lf task complete EXP-12` completes without moving the Workflow. Moving or
arriving at `end` records the movement, then requests completion. A failed
request shows **Completion pending**; retry `lf task complete EXP-12`, without
replaying the Flow. Completion requires a verified merge when the Task has a PR. Linear completion and reopening change
status without moving the Workflow or stopping Processes; reopening supersedes
an older completion request. `lf task reopen EXP-12` does the same locally,
including offline: it returns planning to unstarted and keeps the PR, checkout,
Started evidence and Workflow position. Restart the
Workflow separately when needed; reopening does not start work. A PR-less
Task can finish with research files or commits; they do not create a PR
requirement. Old Session turns and unknown process exits cannot veto completion.
Live controls and execution history remain intact; unsafe or occupied checkouts
are retained. An open or closed-unmerged PR still requires delivery or explicit
abandonment. Use another Task for simultaneously open dependent work:

```bash
lf checkout EXP-13 --stack-on EXP-12 --design scratch/child-design.md
lf --task EXP-13 pr publish
```

A published parent PR, including a draft, is enough to start and publish the
child. Prepare a child-specific design; checkout transfers it without overwriting
newer child work. `lf sync` follows parent updates and its squash merge while
preserving the child's identity, PR, Session and design.

## Schedule ordinary commands

```yaml
# .lf/config.yaml
crons:
  - name: daily-review
    command: [--wave, product, --batch, skill, wave/operate]
    schedule: 0 0 9 * * *
```

```bash
lf cron preflight                  # validate configuration and Machine placement
lf cron sync                       # explicitly install the configured list
lf cron list --wave product         # filter by the command's Work selector
lf cron list --task EXP-12
lf cron add daily-review --schedule daily -- --wave product --batch skill wave/operate
```

Commands use ordinary `lf` arguments, including `--wave` and `--task`.
Reading or cloning the repository installs nothing. An explicit `crons: []`
removes the repository's jobs when synced; an absent list leaves them alone.
Scope-filtered sync only changes matching jobs. Existing job names and Wave
selectors retain installed identities and receipt history.

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
watcher running during `land --wait-and-fix`: waited landing admits the same repair.
After bare land returns, failures wait for a watcher or another waited landing.
An idle check launches no provider. Existing work and GitHub merge requests
continue when the schedule is disabled or a Task is held.

Set repository defaults in `.lf/config.yaml`:

```yaml
automation: {retries: 1}
```

Use `lf task automate EXP-12 off` to hold future CI repair, and
`lf task automate EXP-12 on` to enable it again. These settings never resume a
Flow. One failed repair startup may retry; a completed blocked repair waits for
changed evidence.
Pending or missing CI checks keep waiting; elapsed time alone never starts repair.
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
current reference contents. Launch saves generated references; Wave and scratch
context can refresh during the conversation. Preview creates no reference files.
It never prepares a checkout, refreshes planning, writes storage or runs a provider.
A real launch refreshes Task direction; preview does not predict future hook input.
Task input lists changed paths with their committed, staged, unstaged and untracked
states. It includes a small text patch when available; omitted patches and exact
file bytes remain in the checkout. The inventory is not a content-hash receipt.

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
Task placement owns branch and base even without a PR.
Omit TASK to use `--task` or checkout inference. JSON separates `resolution`,
`action`, `impediments` and `unavailable`. Local path/branch proposals reserve
nothing: previews never fetch, allocate a checkout, take a lease, change a stack
or contact a provider. Missing branch recovery and finalization remain unperformed;
previews read admission without claiming it.

`task move TASK NODE --explain` and `task workflow restart TASK --explain`
validate the captured Workflow without moving it. JSON adds the intended `action`
(including the exact prior node/edge and reason), `impediments` and
`unavailable` to `resolution`. Restart means move to `start`, not run a Flow.
Moving to `end` records arrival before requesting completion. Settlement checks PR
gates and may clean an eligible checkout; preview performs none of these effects.
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
connection, takeover/try intent and the shared Session actions.
`connect --explain` and `resume --explain` resolve the same scope as opening,
including first creation and fresh intent; explicit `--task` or `--wave` overrides discovery. JSON adds `action`, `state`, `actions`, `impediments` and
`unavailable` to `resolution`. `connect --prepare` prepares launch
arguments without takeover, with or without `--json`. JSON changes representation only. Live
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
browser without replacing file selection or drafts. Bare Task opening acquires no
conversation or checkout, even when its execution location is unavailable.
Session and Changes requests still require that location. A `--diff` request without `--session` uses the Task's
ordinary primary-Session owner to choose/prepare its conversation.

JSON returns `opening`, not `usable`: macOS accepting a link proves no native
endpoint. `desktop list --json` includes each window's latest Task `opening`
reading and lookup failures. Page mounting and requested Session/Changes readiness
settle that request; connection/comparison failures remain visible. These receipts
are not configured/native acceptance, and `opening` must not authorize pane input.
Display opening stays on the Mac. For a recorded remote owner, the Task link carries
Machine and repository identity into the same repository window; Session and Files
operations run on that owner after a fresh location read. Missing owners never create
a local checkout or transfer execution. Bare `open` is ambiguous with `pr open`;
Sessions use `session connect`.

Desktop inspection reports registered repository/window identities, selection,
reading availability and retained Work pane trees with focus, zoom
and hidden panes. Each workspace reports its repository/Wave/Task key, title, open
state and selection; Machine/checkout locations belong to its pane contents. `task` forwards Rust's recommendation and `run_control`;
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
The outer checkout-slot `layouts` field is removed; `workspaces` retains pane trees.
Opening Work selects its named tab without starting a conversation or preparing a
checkout. Closing a tab hides its workspace; reopening restores panes, drafts and
zoom. Closing the last tab starts nothing. This retention lasts for the window,
not across app quit. Mutation receipts identify `work` and `pane`, not readiness.

Select Work and use pane IDs from the retained layout:

```sh
lf desktop list --json
lf --task LOO-427 desktop text -- 'add to the draft'
lf --task LOO-427 desktop key enter
lf --repo loopflow --task LOO-427 desktop hide --pane "$pane"
lf --task LOO-427 desktop restore --pane "$pane"
```

Without `--pane`, a command issued on a verified Desktop terminal's original
PTY selects that terminal within the chosen Work. Explicit `--pane` wins.
Otherwise one eligible pane is selected; ambiguity lists choices and changes
nothing. Redirected stdin, agent tool subprocesses without that PTY, and markers
inherited across SSH do not establish caller identity: use `--pane` when needed.
Selection never follows current focus. Text/key/read require an existing terminal surface. Restore without a pane
selects a unique hidden pane. Wave and Task scope select their Work workspace;
a repository without Task scope considers its retained workspaces. Unavailable Work
or no matching pane produces an error without opening a window or preparing Work.

Window, Work workspace, content and native-surface lifetimes are resolved internally
from one Desktop reading. Replacements before dispatch refuse the request instead
of following new focus. There is no public JSON target or surface-token argument.

```sh
lf --task LOO-427 desktop focus --pane "$pane"
lf --task LOO-427 desktop split --pane "$pane" --axis vertical
lf --task LOO-427 desktop move --pane "$pane" --destination "$other" --axis horizontal
lf --task LOO-427 desktop resize --pane "$pane" --toward "$other" --ratio 0.6
lf --task LOO-427 desktop zoom --pane "$pane"
lf --task LOO-427 desktop zoom --pane "$pane" --off
```

Split creates a shell beside the addressed pane on its execution Machine and checkout.
It preserves the selected pane; a missing remote location refuses before changing layout.

```sh
pane=$(lf --task LOO-427 desktop shell --pane "$anchor" --json | jq -r .pane)
lf --task LOO-427 desktop zoom --pane "$pane"
lf help desktop shell --json          # installed arguments and JSON result schema
lf help desktop --all --json          # all Desktop command contracts
```

Structured help derives syntax from Clap and Desktop output schemas from their
wire types. It describes the installed CLI without opening the app. A null
`result_schema` means that command has no schema advertised yet. Running-app
support remains in `desktop list`; current Work eligibility stays in its legal
actions. A successful shell receipt may precede its surface: inspect readiness
before input, and do not repeat creation after a lost reply.

Move and resize use two distinct panes in the same Machine/checkout. Vertical
places a pane to the right; horizontal below. Resize gives the selected side its
share (0.1–0.9). Split, move, resize, zoom, hide and restore retain drafts and
selection. Focus selects the addressed pane without changing Work or foregrounding
a window. Closing a view does not end a Session. Mutations print the affected
pane ID; `--json` adds its repository, window, Machine, checkout and observation
time. Split returns the new pane; companions return the created or reused pane.
A receipt confirms the operation, not terminal readiness or usable rendering. Inspect after a lost reply; split is not safe to replay.

```sh
lf --task LOO-427 desktop shell --pane "$pane"
lf --task LOO-427 desktop files --pane "$pane"
lf --task LOO-427 desktop flow-log --pane "$pane"
```

Companions fill an empty pane or open beside occupied content, retaining focus,
zoom and Work. Files/Flow-log reuse the Task's pane. Shell opens on the recorded
execution Machine and rechecks the checkout before launch; it never falls back to
a local shell. Repeating Shell creates another shell, so inspect after a lost reply.
Remote filesystem events update retained documents; disconnects retain drafts.

```sh
lf --task LOO-427 desktop read --pane "$pane" --region screen --max-bytes 65536 --json
lf --task LOO-427 desktop text --pane "$pane" -- 'literal text'
lf --task LOO-427 desktop key --pane "$pane" enter
```

Read regions are `screen`, `scrollback` and `selection`. Byte bounds are
1–1048576 (default 65536); truncation ends at a complete UTF-8 scalar. Selection
includes shell command blocks; empty selection succeeds. Reads never acquire a
client, send input, follow focus or clean up an exited surface. There is no
unbounded fallback. A surface disappearing after resolution remains unavailable.

Text inserts at the cursor without clearing drafts. Quotes, backslashes and
Unicode stay literal; control characters, including newline/Tab, are rejected.
Keys are separate: `enter`, `tab`, `escape`, `backspace`, `delete`, `left`, `right`,
`up`, `down`, `home`, `end`. Missing, exited or replaced surfaces reject input;
active IME composition must finish first. Input is not idempotent: never replay
an uncertain reply. Native/configured-SSH acceptance remains with the fixture demo.

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
`--available` narrows that selection to conversations available for your input. In Desktop,
a program's OSC 7501 report takes precedence: any blocked record means Available,
as does idle for an interactive Session. Working, done, error and explicit clear
suppress the quiet-time inference for that AgentProcess. A blocked child
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
lf connect                       # persistent repository conversation
lf connect --wave product         # separate persistent Wave conversation
```

Inside the conversation's persistent checkout:

```bash
lf commit -m "Record accepted decisions" .lf/wave/product/MEMORY.md
lf pr publish
```

`lf wt create planning --persistent` also creates or reuses an independent document
workspace; enter its printed path before editing.

Primary conversations reuse their respective worktrees, including after replacement.
They display a workspace without gaining Task membership. Moved checkouts are
rediscovered; missing checkouts recover committed branch state. Live conversations
keep their placement until their attached LfProcess is idle.

Scratch files are ordinary repository content during commit, sync and publication.
Selected-path commits preserve unrelated staged edits. Publication pushes the
committed range, leaving later local edits alone. Delivery and `lf scratch clear`
remove committed scratch additions relative to the Task's base, preserving
existing files, edits and renames. Inspect the complete range before publishing.

If a sync resolver creates a file where an untracked file was stashed, sync restores
the original and keeps the resolver's file beside it as `<name>.lf-sync-1` (or the
next unused number). Read both notes and reconcile them locally. Tracked conflicts
retain the recovery stash and report its identity; resolve them before restoring it.

Run `lf sync --plan` and `lf sync` to integrate upstream changes at a deliberate
maintenance boundary. Continued publication handles already-merged history. Network failures leave local work usable; conflicts stay visible
and use `lf sync --continue` or `lf sync --abort`. Persistent branches remain reusable
and survive automatic pruning. Memory updates do not require PRs or a schedule.

## Work on another machine

```bash
lf machine add mini --repo '~/src/project'
lf machine status mini
lf machine connect mini codex work@example.com
lf --machine mini --repo loopflow session list
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

```sh
lf repo identity                         # enroll before moving the directory
mv ../project ../renamed-project
cd ../renamed-project
lf repo identity                         # reconnect the same Work
```

The Git common directory retains a Machine-scoped locator. Reopening a moved
checkout repairs Git's linked-worktree pointers and updates current Wave, Task
and Session addresses together. Existing sibling worktree names and unpublished
edits stay in place. Process paths and provider history retain their original
values. Desktop reuses the repository's window and Session surfaces.

After upgrading, a retained local Task's linked checkout and branch can establish
the original Git directory automatically. Otherwise, establish its identity with
`lf repo identity --bind repo_ID` before moving it. If it has already moved, run
`lf repo identity --relocate repo_ID` from the new location. Ambiguous or reused
paths require explicit recovery. A copied Git directory cannot
replace its still-present source. Remote URLs and directory names never merge Work.
Open the moved checkout before reusing its old path. Experimental Machines keep separate locators; disposable data
clones observe copied addresses without claiming the source checkout.

Provider callbacks launched with this version retain their launch description
in the Machine and resolve moved references by repository identity. Already
running older providers can retain old hook paths; this does not rewrite their
environment or interrupt them. An installed Desktop update still requires the
supported installation handoff; source tests do not prove that live handoff.

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

### Pause a Task without discarding work

```sh
lf task pause LOO-389
lf task resume LOO-389
```

Paused Tasks stay in the Wave plan but leave the default active sidebar. Their
checkout, conversations and Workflow remain. Pause holds future Task runs and CI
repair; it does not stop an already-running process. Resume starts nothing.
In Desktop, use the Task header's context menu to pause or resume.
