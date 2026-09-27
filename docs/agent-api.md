# The Agent API

Use this page when asking an AI coding tool to operate Loopflow. It uses the
same commands that supply the Mac app's status and controls.

An [API](glossary.md#borrowed-from-software-engineering) is an interface one
program calls. Here that interface is `lf`, the command-line program; there
is no separate SDK (a library to install in code). Read commands accept
`--json` to return structured data. Each agent launched by Loopflow receives
`LOOPFLOW.md`, its operating instructions, in its
[context](glossary.md#loopflows-words).

## Install the external skill

A [harness](glossary.md#loopflows-words) is the coding tool that runs the AI,
such as Codex or Claude Code. Install the operating skill in a tool started
outside Loopflow so it knows which commands to use:

```bash
npx skills add loopflowstudio/loopflow --skill loopflow -g -y
```

The installed skill teaches the coding tool to call `lf`; Loopflow still performs the operations and keeps their records.

## Two caller authorities

An external harness opened by a person is a Loopflow **User**, the same caller
kind as the Mac app. It may inspect status and use `lf chat` when the person
asks it to converse with a Wave.

A Loopflow-launched Wave or Task agent is an internal participant. Its
instructions and assigned Work determine which operations it may perform.
Recorded status carries coordination between agents. `lf ask` opens a
[Session](glossary.md#loopflows-words), a conversation with a person in the
same checkout, and waits for that person to complete it.

A [Wave](glossary.md#loopflows-words) keeps working toward an objective;
a Task is one piece of that work. For an internal caller directing a Task,
replace `INF-123` below with an existing Linear issue identifier:

```bash
lf task prepare INF-123                              # tracked Work, no execution
lf --task INF-123 research "write scratch/api.md"    # independent bounded Run
lf task run INF-123                                  # start built-in Task automation
lf task steer INF-123 "take the smaller approach"    # post a Linear Task comment
lf task status INF-123 --json                        # inspect durable state
lf task wait INF-123 --until terminal                # block until it settles
```

## The nouns

Tracked [Work](glossary.md#loopflows-words) follows **Wave → Task**. These
are planning records that survive process restarts. A Wave coordinates and
remembers; its one internal Project holds the current chapter's Tasks, KRs,
and metric targets. Task Work owns a stable
[worktree](glossary.md#borrowed-from-software-engineering), a checkout for its
edits, and one active branch and pull request at a time. A PR proposes changes
for merge to `main`, the shared finished version.

Work can be `ready` (unfinished), `done`, or `abandoned`. Whether a process is
running, what a Task needs, and which Sessions remain open are separate facts.

A [Run](glossary.md#loopflows-words) records one launch of a coding tool on one
Home, the computer that ran it. Entries accumulate without rewriting history.
Work may produce many Runs; a Run may just name that Work as its subject.
Naming Work does not reserve it, grant permission to edit its worktree, or
establish which process can safely be stopped. Tasks are Linear issues, so
saved delegated work begins with an issue and appears on the roadmap.

Only the Task worker advances the Task's saved Flow, one step at a time.
`wave/operate` is a finite planning Run over a Wave's current chapter and Tasks.
An agent may also use `lf task prepare`, run a skill with `--task` or `--wave`,
add direction, and use delivery commands. Associating a Run with a Task gives
it context; it does not grant control of that Task's Flow position. A Flow is the ordered
steps for the work. A KR states a chapter outcome and the evidence needed to
judge it; a metric is a repeated measurement, with a target for the chapter.

## Delegate

Start separate work only when authorized and when an independent part makes
the original task smaller. Replace `<wave>` with a Wave name. `pbpaste` reads
the macOS clipboard; the pipe passes that text to the next command's standard
input, the text a command receives from another program.

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
lf --wave <wave> wave/operate "reassess chapter priorities"
```

Comment on the Linear Task directly, or use `task steer`. Both reach only the
worker advancing that Task. Independent Runs sharing its worktree or using
`--task`/`--as` do not subscribe to steering. With no active worker, comments
wait for explicit advancement; steering never starts execution.

Linear comments keep the direction. Workers read them while running and before
starting a skill; local events record delivery. The command receipt confirms
that Linear received the comment. Run records distinguish text included at
launch from text accepted by the running coding tool. Neither proves that the
model followed the instruction; the coding tool decides when to read it.

`lf task interrupt INF-123` appends a durable interrupt comment;
the active Task worker observes it and ends the current provider turn so the
next boundary re-reads direction. With no live worker it remains durable input.
Generic `lf work interrupt` refuses because it does not publish an exact process
owner. Loopflow never guesses signal authority from a Run id, Work id, PID, or
tmux name. A `wave/operate` planning Run ends after its pass; it is separate from the
Wave listener that keeps receiving messages.

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

Pass immediate planning direction to `wave/operate`. Edit the Wave's goal
when that direction should apply to future turns too.

## Memory

`wave/<name>/MEMORY.md` is the Wave's durable memory. Read or edit it through
the ordinary repository workflow; the file is truth, running Wave or not, and
there is no separate CLI or server surface for it.

## Ship

Choose the delivery operation explicitly. Publishing uploads a branch and PR
for review; submitting prepares it for a person's merge click; auto-merge asks
GitHub to merge the chosen commit when its requirements pass. A completed
review does not itself authorize any of these external actions.

All four commands can run without opening a browser:

```bash
lf pr publish    # make work visible mid-stream; the agent's default verb
lf pr submit     # done, a person clicks merge
lf pr arm        # request exact-head auto-merge and return
lf pr land       # watch, repair CI, and return only after merge
```

`lf pr open` also opens the PR in a browser. Use it when the person asked to
see the PR. CI means the automatic checks that must pass before merge.

## Observe

Use [JSON](glossary.md#borrowed-from-software-engineering) output when another
program needs to read the result. Replace `<wave>` with the Wave name:

```bash
lf ls --json                # every durable Wave and its Home/runtime evidence
lf status <wave> --json     # chapter, Tasks, and measurements in metric_portfolio
lf roadmap --json           # current plan and each Wave's measurements
lf activity --task INF-123 --json
lf runs --project parser --json
lf runs --task INF-123 --json
lf usage --days 30 --json   # direct RunSnapshot evidence, newest first
lf usage --task INF-123 --json # the same evidence drilled to one Task
lf ps --json                # one OS-live process frame
```

`lf ls` lists registered Waves. `lf status` focuses on one Wave;
`lf roadmap` combines the current Linear plan with execution evidence.
`metric_portfolio` contains dated measurements and their targets, including
missing or unavailable evidence. `lf activity` lists recorded changes in order;
its `WorkRef` field identifies the Work each fact concerns. Use these results
rather than trying to infer shared state from processes or worktree files.

`RunSnapshot` means a Run's recorded facts. A process is a running program;
`lf ps` reads the operating system's current process list. All of these reads
are local to the executing Home. Use `lf ssh <home-id> ...`
to execute the same read on another machine over SSH. `lf runs` and `lf usage` scan that Home's
Run records; they do not query a central execution service.

[Conducting →](conducting.md) covers the full monitoring surface. The Mac app is
built on exactly these calls — it keeps no second database.

## The contract

Every launched agent gets `LOOPFLOW.md` — the operating contract — in context
(opt out with `--no-loopflow`). Its main instructions:

- Route git, worktrees, and PRs through `lf`; never raw `git worktree`.
- Execute here first; delegation must make the problem smaller.
- Checkpoint and proceed: don't ask permission for reversible work.
- Answer the user in the current conversation; use typed Work observations for durable
  coordination, ordinary `lf --as` Runs for another agent perspective, and
  `lf ask` only for a new session.
- Keep repeatable instructions in the skill that uses them, repository
  conventions in the agent guide, configuration in `.lf/config.yaml`, and
  Wave decisions in `wave/<name>/MEMORY.md`. Commit them with the work.

Source: `rust/loopflow/src/engine/builtins/LOOPFLOW.md`.

## Next

[Conducting →](conducting.md) · [Waves →](waves.md) · [`lf` reference](lf.md)
