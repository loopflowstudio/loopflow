# The Agent API

`lf` is the API agents call to launch, steer, and observe other agents. There
is no SDK. The verbs are the same binary you use; every read surface takes
`--json`; and every launched agent receives the
operating contract (`LOOPFLOW.md`) in its context, so it already knows these
verbs when it starts.

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
It receives `LOOPFLOW.md`; typed Work observations carry durable coordination,
while `lf ask` blocks its caller on a durable AgentSession in its own checkout.

A Wave directing a task is the internal case:

```bash
lf task checkout INF-123                              # tracked Work, no execution
lf --task INF-123 research "write scratch/api.md"    # independent conversation
lf --task INF-123 flow start                                  # start built-in Task automation
lf comment INF-123 "take the smaller approach"    # post a Linear Task comment
lf task status INF-123 --json                        # inspect durable state
lf wait INF-123 --until terminal                # block until it settles
```

## The owners

Tracked Work follows Wave → Task in navigation. A Wave owns its objective and
memory; its current Linear Project owns Tasks, KRs, targets and default Flow.
Task owns the checkout, serial PRs and one managed FlowSession selection. Other
conversations and Flows may carry Task attribution without acquiring that claim.

Exec records an actual lf command process. AgentSession keeps a continuable
conversation, whether interactive or headless. FlowSession captures one Flow
and consumes exact boundary results. Provider turns and retries remain history
inside the conversation; they do not create another generic execution object.

A child command records its causal parent and, when issued by an agent, its
calling conversation and provider generation. That evidence grants neither
process-control authority nor permission to move a Flow cursor. A driver handoff
retains conversation identity while fencing the old writer. Passive observation
acquires no claim. See the [contract and cutover status](architecture-reference.md#cutover-status)
for the remaining reader, wire and lifecycle conversion.

## Delegate

```bash
lf checkout INF-123                    # ensure Work and worktree only
lf --task INF-123 flow start                    # run an existing Linear issue
lf task create --run --wave <wave> --title "add passkeys" # create the issue, then run it
pbpaste | lf task create --run --wave <wave>         # report from stdin; first line is the title
lf --task INF-124 flow start --stack-on INF-123 # dependent work before the parent PR merges
```

The contract every agent runs under: **delegation must make the problem
smaller**. Delegate only a strict subset that can finish independently; never
hand off the whole seed, and never delegate the one blocker between you and
completion — resolve that inline. The current process and worktree are the
default execution surface.

`--stack-on` forks the child's worktree from the parent Task's active PR, records
the fork commit, and adds a `Clear inherited scratch` commit before execution.
Parent syncs keep the child's scratch, including its deletions. The parent retains
its own notes. The child's PR targets the parent's branch until
merge, then `lf task sync` merges main using the recorded fork as the comparison base.
Child edits survive the parent's squash landing without replay.

## Steer

```bash
lf task comment INF-123 "keep the public API"          # post a Linear Task comment
lf task comment INF-123 --steer "keep the public API"  # explicit direction from an agent
lf --wave <wave> wave/operate "prioritize the parser"
lf --wave <wave> wave/operate "reassess Project priorities"
```

Comment on the Linear Task directly, or use `task comment`. Both reach only the
worker advancing that Task. Independent conversations sharing its worktree or using
`--task` do not subscribe to steering. With no active worker, comments
wait for explicit advancement; steering never starts execution.

Linear comments are the authored record. Workers refresh comments while running
and before starting a Skill; local events cache their delivery. The command
receipt confirms publication to Linear. Conversation history distinguishes input included
in the starting prompt from input accepted by the live provider transport;
neither proves the model followed it. Provider scheduling determines when a
live correction is consumed.

On a repeated captured Flow step, workers seed only steer IDs newer than that
step's last successful Run inputs. Failed or interrupted attempts acknowledge
nothing. Each structural step and each new invocation has its own history;
unreceived late comments remain eligible. This records delivery, not proof that
the model followed the instruction.

Keep routine agent progress in working notes and the Run response. `task comment`
inside a Run marks its publication as progress, excluded from steers. Use
`--steer` only to deliver deliberate new direction. Direct
participant comments and explicit worker steering remain direction, even through
the same account. Other integrations should mark progress with
`<!-- loopflow-progress:<source-id> -->`; historical unmarked comments remain
eligible because their authorship cannot be inferred safely.

`lf task interrupt INF-123` appends a durable interrupt comment;
the active Task worker observes it and ends the current provider turn so the
next boundary re-reads direction. With no live worker it remains durable input.
Loopflow never guesses signal authority from a conversation ID, Work ID, PID, or
tmux name. Project operations are ordinary finite conversations; they have no resident
process to interrupt, resume, wait for, or attach to.

Work survives its provider process. `lf --task INF-123 flow start` continues the saved
Flow without losing durable direction, the worktree, or the Task PR. `lf flow start` never reopens terminal Work. Create a new Task for new work;
`lf task status ISSUE` retains deliberate historical lookup.

Automated Task commit, PR, and completion commands also re-check current PM
ownership. If Linear moved the issue to another Project, the Task operation
fails closed before a commit, push, publication, merge request, rotation, or
completion; a person retains explicit authority to inspect and remediate the
preserved Work.

Guide Project and Wave operations through extra instructions to their operate
skills. Edit their definition or goal when the guidance should persist.

## Memory

`wave/<name>/MEMORY.md` is the Wave's durable memory. Read or edit it through
the ordinary repository workflow; the file is truth, running Wave or not, and
there is no separate CLI or server surface for it.

Remove registered or planning-only Tasks with `lf task delete ISSUE`. Repeat the same command
after an incomplete operation, even after planning refresh or chapter replacement.
Missing provider data does not confirm deletion.
Deletion cancels unfinished placed work or cleans completed delivery before trashing
the issue. Live or unresolved workers and dirty checkouts block it. Task, PR and
Run history remain readable; confirmed provider trash makes retries idempotent.

## Ship

Three commitment levels, all headless — pick by how done the work is and who
lands it:

```bash
lf pr publish                          # make work visible mid-stream; the agent's default verb
lf submit                              # done, a person clicks merge
lf land                                 # request exact-head auto-merge and return
```

`lf pr open` is the one presenting verb — it opens a browser. Agents reach for
it only when a person asked to see the PR.

## Observe

Every read the conducting surfaces offer is `--json`:

```bash
lf wave list --json                    # every durable Wave and its Home/runtime evidence
lf wave status <wave> --json           # hierarchy plus one Rust-derived metric_portfolio
lf roadmap --json                      # every Wave repeats that required portfolio envelope
lf activity --task INF-123 --json
lf mon show --project parser --json
lf mon show --task INF-123 --json
lf usage --days 30 --json   # recorded provider usage, newest first
lf usage --task INF-123 --json # the same evidence drilled to one Task
lf ps --json                # one OS-live process frame
```

`lf wave list` is the registry plane, `lf wave status` is the focused operational view,
and `lf roadmap` joins the current Linear plan to that runtime truth.
`lf activity` is the ordered durable history; each item reuses `WorkRef` and
carries one typed fact with its execution, Task PR, or Steer evidence. Agents consume
those projections; they do not rebuild the joins.

All of these reads are local to the executing Home. Use `lf ssh <home-id> ...`
to execute the same read remotely. The historical `lf mon show` interface and `lf usage` read that Home's evidence;
their transitional wire shape is recorded in the cutover status. They do not
query a central execution service.

[Conducting →](conducting.md) covers the full monitoring surface. The Mac app is
built on exactly these calls — it keeps no second database.

## The contract

Every launched agent gets `LOOPFLOW.md` — the operating contract — in context
(opt out with `--no-loopflow`). Its spine:

- Route git, worktrees, and PRs through `lf`; never raw `git worktree`.
- Execute here first; delegation must make the problem smaller.
- Checkpoint and proceed: don't ask permission for reversible work.
- Answer the user in the current conversation; use typed Work observations for durable
  coordination, ordinary `lf --task` conversations for another agent perspective, and
  `lf ask` only for a new session.
- Keep repeatable instructions with their skill, repository rules in its agent guide,
  and durable Wave learning in its existing memory.

Source: `rust/loopflow/src/engine/builtins/LOOPFLOW.md`.

## Next

[Conducting →](conducting.md) · [Waves →](waves.md) · [`lf` reference](lf.md)
