# The Agent API

`lf` is the API agents call to launch, steer, and observe other agents. There
is no SDK and no central server. The verbs are the same binary humans type;
every read surface takes `--json`; and every launched agent receives the
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
kind as the Mac app. It may inspect status and use `lf chat` when the person
asks it to converse with a Wave.

A Loopflow-launched Wave or Task agent is an internal participant.
It receives `LOOPFLOW.md`; typed Work observations carry durable coordination,
while `lf ask` parks the Run at a durable human session in its own checkout.

A Wave directing a task is the internal case:

```bash
lf task prepare INF-123                              # tracked Work, no execution
lf --task INF-123 research "write scratch/api.md"    # independent bounded Run
lf task run INF-123                                  # start built-in Task automation
lf task steer INF-123 "take the smaller approach"    # post a Linear Task comment
lf task status INF-123 --json                        # inspect durable state
lf task wait INF-123 --until terminal                # block until it settles
```

## The nouns

Tracked Work follows **Wave → Task**. These are stable planning
records, not a process hierarchy. A Wave coordinates and remembers; a Project
pursues measurable KRs; only Task Work owns a worktree, and every file-writing
change happens there on its one active remote branch and PR to `main`. Work is `ready`,
`done`, or `abandoned`; process liveness, Task condition, and Sessions are
separate evidence.

A **Run** is different: one append-only Home-local record of one harness launch.
A Work may produce many Runs, and a Run may merely name Work as its subject.
Run identity does not reserve Work, authorize a worktree mutation, or prove a
process signal-safe. Tasks are Linear issues, so durable delegated work starts
from an existing issue and the roadmap remains the queue.

Only Task Work persists a selected Flow and advances it one exact boundary at a
time. Project operation is a finite `wave/operate` Run over current facts.
An agent may also compose `lf task prepare`, a `--task`/`--wave`
skill Run, Work input, and delivery commands itself. Attribution grants context,
not permission to move a Task's Flow position.

## Delegate

```bash
lf task prepare INF-123                      # ensure Work and worktree only
lf task run INF-123                          # run an existing Linear issue
lf task start --wave <wave> "add passkeys"    # create the issue, then run it
pbpaste | lf task start --wave <wave>         # report from stdin; first line is the title
lf task run INF-124 --stack-on INF-123       # dependent work before the parent PR merges
```

The contract every agent runs under: **delegation must make the problem
smaller**. Delegate only a strict subset that can finish independently; never
hand off the whole seed, and never delegate the one blocker between you and
completion — resolve that inline. The current process and worktree are the
default execution surface.

`--stack-on` forks the child's worktree from the parent Task's active PR and
records the fork commit; the child's PR targets the parent's branch until
merge, then replays only child-authored commits onto `main`.

## Steer

```bash
lf task steer INF-123 "keep the public API"          # post a Linear Task comment
lf --wave <wave> wave/operate "prioritize the parser"
lf --wave <wave> wave/operate "reassess Project priorities"
```

Comment on the Linear Task directly, or use `task steer`. Both reach only the
worker advancing that Task. Independent Runs sharing its worktree or using
`--task`/`--as` do not subscribe to steering. With no active worker, comments
wait for explicit advancement; steering never starts execution.

Linear comments are the authored record. Workers refresh comments while running
and before starting a Skill; local events cache their delivery. The command
receipt confirms publication to Linear. Run traces distinguish input included
in the starting prompt from input accepted by the live provider transport;
neither proves the model followed it. Provider scheduling determines when a
live correction is consumed.

`lf task interrupt INF-123` appends a durable interrupt comment;
the active Task worker observes it and ends the current provider turn so the
next boundary re-reads direction. With no live worker it remains durable input.
Generic `lf work interrupt` refuses because it does not publish an exact process
owner. Loopflow never guesses signal authority from a Run id, Work id, PID, or
tmux name. Project operations are ordinary finite Runs; they have no resident
process to interrupt, resume, wait for, or attach to.

Work survives its provider process. `lf task resume INF-123` starts a fresh
boundary without losing durable direction, the worktree, or the Task PR. `lf
task run` never reopens terminal Work: a person can use
`lf task recover` to restart an abandoned Task on the same worktree, while a
completed Task requires a new Linear task.

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

## Ship

Four commitment levels, all headless — pick by how done the work is and who
lands it:

```bash
lf pr publish    # make work visible mid-stream; the agent's default verb
lf pr submit     # done, a human clicks merge
lf pr arm        # request exact-head auto-merge and return
lf pr land       # watch, repair CI, and return only after merge
```

`lf pr open` is the one presenting verb — it opens a browser. Agents reach for
it only when a human asked to see the PR.

## Observe

Every read the conducting surfaces offer is `--json`:

```bash
lf ls --json                # every durable Wave and its Home/runtime evidence
lf status <wave> --json     # hierarchy plus one Rust-derived metric_portfolio
lf roadmap --json           # every Wave repeats that required portfolio envelope
lf activity --task INF-123 --json
lf runs --project parser --json
lf runs --task INF-123 --json
lf usage --days 30 --json   # direct RunSnapshot evidence, newest first
lf usage --task INF-123 --json # the same evidence drilled to one Task
lf ps --json                # one OS-live process frame
```

`lf ls` is the registry plane, `lf status` is the focused operational view,
and `lf roadmap` joins the current Linear plan to that runtime truth.
`lf activity` is the ordered durable history; each item reuses `WorkRef` and
carries one typed fact with its Run, Task PR, or Steer evidence. Agents consume
those projections; they do not rebuild the joins.

All of these reads are local to the executing Home. Use `lf ssh <home-id> ...`
to execute the same read remotely. `lf runs` and `lf usage` scan that Home's
Run records; they do not query a central execution service.

[Conducting →](conducting.md) covers the full monitoring surface. The Mac app is
built on exactly these calls — it keeps no second database.

## The contract

Every launched agent gets `LOOPFLOW.md` — the operating contract — in context
(opt out with `--no-loopflow`). Its spine:

- Route git, worktrees, and PRs through `lf`; never raw `git worktree`.
- Execute here first; delegation must make the problem smaller.
- Checkpoint and proceed: don't ask permission for reversible work.
- Answer present humans in turn text; use typed Work observations for durable
  coordination, ordinary `lf --as` Runs for another agent perspective, and
  `lf ask` only for a new human session.
- Write repo-specific learnings into `.lf/` and commit them with the work.

Source: `rust/loopflow/src/engine/builtins/LOOPFLOW.md`.

## Next

[Conducting →](conducting.md) · [Waves →](waves.md) · [`lf` reference](lf.md)
