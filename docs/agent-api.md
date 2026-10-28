# The Agent API

`lf` is the API agents call to launch, steer, and observe other agents. There
is no SDK. The verbs are the same binary you use; every read surface takes
`--json`; and every launched agent receives the
launcher contract (`LOOPFLOW.md`) in its context. Work-associated launches also
receive the `work` skill body. Read `lf help skill work` before using Task, Wave or
Project APIs in an existing conversation. Choose the commands and Skills that
fit the requested work. Work context does not select a Graph or authorize Wave
memory maintenance; the selected Loopflow Graph owns that procedure.

## Inspect instruction selection

```bash
lf history show SESSION --instructions
lf history show SESSION --instructions --json
```

The retained input records why launcher guidance, Work guidance and selected
skills were supplied. It also records automatic opt-out, missing Work association,
and deduplication when `work` was selected as a skill. Use `--input INPUT` to
inspect an earlier input belonging to that Session. These are recorded decisions,
not a reconstruction from the repository's current state. Old captures may have
less detailed reasons; missing captures report unavailable evidence. Native
provider instructions not attributed in the input remain outside this view.

## Install the external skill

Give an agent harness the same operating contract when it was not launched by
Loopflow:

```bash
npx skills add loopflowstudio/loopflow --skill loopflow -g -y
```

The installed skill teaches the harness to use `lf`; it does not implement a
second client, store, or transport.

## Two caller authorities

An external harness opened by a person is a Loopflow **User**, the same caller
kind as the Mac app. It may inspect status and start a Session when the person
asks it to converse with a Wave.

A Loopflow-launched Wave or Task agent is an internal participant.
It receives `LOOPFLOW.md` and work guidance; typed Work observations carry durable coordination,
and headless failures remain visible in ordinary output and recorded outcomes.

A Wave directing a task is the internal case:

```bash
lf checkout INF-123                              # tracked Work, no execution
lf --task INF-123 research "write scratch/api.md"    # independent conversation
lf task run INF-123                                  # start built-in Task automation
lf comment INF-123 "take the smaller approach"    # post a Task comment
lf task status INF-123 --json                        # inspect durable state
lf task wait INF-123 --until terminal                   # block until it settles
```

## The owners

Tracked Work follows Wave → Task in navigation. A Wave owns its objective and
memory; its current Project owns Tasks, KRs and targets.
A Task owns its checkout and its PR history, with one current PR. Every conversation and Graph attributed
to it is equally its work; none is privileged.

Process records an actual lf command process. LfSession keeps a continuable
conversation, whether interactive or headless. A Graph is one lf process and
the step processes it starts. Provider turns and retries remain history
inside the conversation; they do not create another generic execution object.

A child command records its causal parent and, when issued by an agent, its
calling conversation and AgentProcess. That evidence grants neither
process-control authority nor permission to move a Graph cursor. An attachment
handoff retains conversation identity while fencing the old writer.
Passive observation acquires no claim. See the [contract and cutover status](architecture/reference-cutover.md#cutover-status)
for the remaining reader, wire and lifecycle conversion.

## Delegate

```bash
lf checkout INF-123                    # ensure Work and worktree only
lf task run INF-123                    # place an existing Task, start its Graph, return once it runs
lf task create --wave <wave> --title "add passkeys"  # file the issue; run it with `lf task run`
pbpaste | lf task create --wave <wave>               # report from stdin; first line is the title
lf task run INF-124 --stack-on INF-123 # dependent work before the parent PR merges
```

The contract every agent runs under: **delegation must make the problem
smaller**. Delegate only a strict subset that can finish independently; never
hand off the whole seed, and never delegate the one blocker between you and
completion — resolve that inline. The current process and worktree are the
default execution surface.

`--stack-on` forks the child's worktree from the parent Task's active PR, records
the fork commit, and inherits the parent's files. Scratch follows the same Git
merge behavior as other content, including edits to existing notes. The child's PR targets the parent's branch until
merge, then `lf sync` merges main using the recorded fork as the comparison base.
Child edits survive the parent's squash landing without replay.

## Steer

```bash
lf comment INF-123 "keep the public API"          # save a Task steer
lf comment INF-123 --steer "keep the public API"  # save a steer from an agent Session
lf session steers SESSION --json                 # inspect delivery
lf session steers SESSION --retry STEER_ID        # retry one delivery
lf --wave <wave> wave-operate "prioritize the parser"
lf --wave <wave> wave-operate "recover the failed parser Graph"
```

Save a Task comment locally; planning sync shares it, and Linear comments arrive as steers.
Each associated Session receives acquired steers independently, including
conversations associated through the checkout or `--task`. Repository and Wave
steers apply downward through the Work tree; direct Session steers apply only
to that Session. Repository and Wave steers currently remain Machine-local.

Delivery occurs at the next supported provider boundary. Idle Sessions stay idle.
Each new Session receives applicable steers at launch; continuing or compacting
the same Session preserves its delivery history. The context stays bounded, with
complete sources available for excerpts.

A saved steer is not proof of delivery. A completed provider write is not proof
the model received or followed it. Uncertain writes remain visible and require
explicit retry; retry saves another attempt without starting a turn.

Keep routine agent progress in working notes and the Session response. `task comment`
from an agent Session marks its comment as progress, excluded from steers. Use
`--steer` only to save a deliberate steer. Direct
participant comments and explicit `--steer` remain steers, even through
the same account. Other integrations should mark progress with
`<!-- loopflow-progress:<source-id> -->`; historical unmarked comments remain
eligible because their authorship cannot be inferred safely.

`lf interrupt INF-123` records a separate control request. An attached Task
command observes it and interrupts its provider turn; steer delivery retains
its independent Session history.
Loopflow never guesses signal authority from a conversation ID, Work ID, PID, or
tmux name. Project operations are ordinary finite conversations; they have no long-running
process to interrupt, resume, wait for, or attach to.

Work survives its provider process. A Graph whose process died leaves its Processes
as history; nothing resumes it. Inspect
`lf task status ISSUE` and `lf graph show ID --processes --json`, then launch
fresh work with `lf task run INF-123`. The Task keeps its durable
direction, worktree and PR. `task run` never reopens terminal Work; create a
new Task for new work.

Automated Task commit, PR, and completion commands also re-check current PM
ownership. If Linear moved the issue to another Project, the Task operation
fails closed before a commit, push, publication, merge request, rotation, or
completion; a person retains explicit authority to inspect and remediate the
preserved Work.

Guide Project and Wave operations through extra instructions to their operate
skills. Edit their definition or goal when the guidance should persist.

## Memory

`.lf/wave/<name>/MEMORY.md` is the Wave's durable memory. Read it for context;
a selected Loopflow Graph maintains it through ordinary file edits. Standalone
Work use keeps proposed changes in scratch. The file remains the source of
truth; there is no separate memory CLI or server surface.

Remove registered or planning-only Tasks with `lf task delete ISSUE`. Repeat the same command
after an incomplete operation, even after planning refresh or chapter replacement.
Missing provider data does not confirm deletion.
Deletion cancels unfinished placed work or cleans completed delivery before trashing
the issue. Live or unresolved execution and dirty checkouts block it; a stopped
Graph is history and blocks nothing. Task, PR and
Run history remain readable; confirmed provider trash makes retries idempotent.

## Ship

Three commitment levels, all headless — pick by how done the work is and who
lands it:

```bash
lf pr publish                          # make work visible mid-stream; the agent's default verb
lf submit                              # done, a person clicks merge
lf land                                 # watch CI, repair failures, and finish merged
```

`lf pr open` is the one presenting verb — it opens a browser. Agents reach for
it only when a person asked to see the PR.

## Observe

Every read the conducting surfaces offer is `--json`:

```bash
lf wave list --json                    # every durable Wave and its Machine/runtime evidence
lf wave status <wave> --json           # hierarchy plus one Rust-derived metric_portfolio
lf wave show --json                      # every Wave repeats that required portfolio envelope
lf history --task INF-123 --json
lf usage --project parser --json
lf history list --task INF-123 --json
lf usage --days 30 --json   # recorded provider usage, newest first
lf usage --task INF-123 --json # the same evidence drilled to one Task
lf usage --weekly --json   # context cost and turn time by week since 2026-09-30
lf usage --binds --json    # Task and Wave totals under prospective and post-hoc bind attribution
lf ps --json                # one OS-live process frame
```

`lf wave list` is the registry plane, `lf wave status` is the focused operational view,
and `lf wave show` joins the current plan to that runtime truth.
`lf history` is the ordered durable history; each item reuses `WorkRef` and
carries one typed fact with its execution, Task PR, or Steer evidence. Agents consume
those projections; they do not rebuild the joins.

All of these reads are local to the executing Machine. Use `lf --machine <machine-id> ...`
to execute the same read remotely. The historical `lf history show` interface and `lf usage` read that Machine's evidence;
their transitional wire shape is recorded in the cutover status. They do not
query a central execution service.

[Telemetry →](telemetry.md) covers the full monitoring surface. The Mac app is
built on exactly these calls — it keeps no second database.

## The contract

Loopflow launches receive a small launcher contract: work in the supplied
checkout, follow the requested tools and repository conventions, and answer in
the current Session. Standalone native skills retain their own guidance.

Task- and Wave-associated launches also receive Work guidance. Read
`lf help skill work` when entering Work APIs from an existing conversation.
Association supplies context and ownership; the selected skill or Graph supplies
the procedure. Keep repeatable instructions with that skill and repository rules
in its agent guide.
