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
asks it to converse with or steer a Wave.

A Loopflow-launched Wave, Project, or Task agent is an internal participant.
It receives `LOOPFLOW.md`; typed Work observations and Ask carry
coordination across the parent relationship.

A Wave directing a task is the internal case:

```bash
lf task prepare INF-123                              # tracked Work, no controller
lf --task INF-123 research "write scratch/api.md"    # independent bounded Run
lf task run INF-123                                  # start built-in Task automation
lf task steer INF-123 "take the smaller approach"    # queue durable direction
lf task status INF-123 --json                        # inspect durable state
lf task wait INF-123 --until terminal                # block until it settles
```

## The nouns

Tracked Work follows **Wave → Project → Task**. These are stable planning
records, not a process hierarchy. A Wave coordinates and remembers; a Project
pursues measurable KRs; only Task Work owns a worktree, and every file-writing
change happens there, advancing through serial PRs to `main`. Work is `ready`,
`done`, or `abandoned`; process liveness and attention are separate evidence.

A **Run** is different: one append-only Home-local record of one harness launch.
A Work may produce many Runs, and a Run may merely name Work as its subject.
Run identity does not reserve Work, authorize a worktree mutation, or prove a
process signal-safe. Tasks are Linear issues, so durable delegated work starts
from an existing issue and the roadmap remains the queue.

Built-in Wave, Project, and Task controllers form a layer above these records.
An agent may instead compose `lf task prepare`, a `--task`/`--project`/`--wave`
skill Run, Work input, and delivery commands itself; it need not install a
controller to act for Work.

## Delegate

```bash
lf task prepare INF-123                      # ensure Work and worktree only
lf task run INF-123                          # run an existing Linear issue
lf task start <project-id> "add passkeys"    # create the issue, then run it
pbpaste | lf task start <project-id>         # report from stdin; first line is the title
lf task run INF-124 --stack-on INF-123       # dependent work before the parent PR merges
lf project run <project-id>                  # start the supervising Project Work
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

Task and Project Steer appends direction to durable Work before doing anything
else. If the Work has a controller, a stopped controller is relaunched and a
running controller reads the new direction at its next boundary. Controller-free
Work simply retains the direction for the next bounded Run or future controller.
Steer does not inject text into an active provider turn:

```bash
lf task steer INF-123 "support passkeys too"       # durable direction
lf task steer INF-123 "stop; make this a smaller PR"
```

The Task and Project wrappers resolve familiar Linear ids. Agents can also use
the stable Work control surface directly:

```bash
lf work status task task_... --json
lf work steer task task_... "show the failing fixture" --json
lf ask list --json                                    # pending parent requests
lf ask open ask_...                                    # open one exact Ask session
```

A Steer receipt proves that the direction was stored, not that a provider read
or applied it. Generic `lf work interrupt`, `lf task interrupt`, and
`lf project interrupt` refuse because these controllers do not publish an exact
process owner. Loopflow never guesses signal authority from a Run id, Work id,
PID, or tmux name. There is currently no supported cross-process CLI for
immediate Task cancellation. Project Work alone has `lf project attach <id>`
for an operator who needs the provider's native controls.

Work survives its provider process. `lf task resume INF-123 --model codex`
selects another provider without losing durable direction, the worktree, or
the PR chain. `lf task run` never reopens terminal Work: a person can use
`lf task recover` to restart an abandoned Task on the same worktree, while a
completed Task requires a new Linear task.

Automated Task commit, PR, and completion commands also re-check current PM
ownership. If Linear moved the issue to another Project, the Task operation
fails closed before a commit, push, publication, merge request, rotation, or
completion; a person retains explicit authority to inspect and remediate the
preserved Work.

`lf project` carries the same durable `steer`, `wait`, `resume`, and `attach`
controls one level up.

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
- Answer humans in turn text; use typed Work observations and explicit
  Ask exchanges for parent/child coordination.
- Write repo-specific learnings into `.lf/` and commit them with the work.

Source: `rust/loopflow/src/engine/builtins/LOOPFLOW.md`.

## Next

[Conducting →](conducting.md) · [Waves →](waves.md) · [`lf` reference](lf.md)
