# The Agent API

`lf` is the API agents call to launch, steer, and observe other agents. There
is no SDK and no server. The verbs are the same binary humans type; every read
surface takes `--json`; and every launched agent receives the operating
contract (`LOOPFLOW.md`) in its context, so it already knows these verbs when
it starts.

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

A Wave, Project, or Task agent launched by Loopflow is an internal participant.
It receives `LOOPFLOW.md`; typed Work observations and Feedback carry
coordination across the parent relationship.

A Wave directing a task is the internal case:

```bash
lf task run INF-123                                  # start a durable Task Session
lf task steer INF-123 "take the smaller approach"    # redirect its active turn
lf task status INF-123 --json                        # inspect durable state
lf task wait INF-123 --until terminal                # block until it settles
```

## The nouns

Delegation follows one supervision path: **Wave → Project → Task**. Each is
stable Work with an Epoch and at most one active Run. A Wave coordinates and
remembers; a Project pursues measurable KRs; only Task Work owns a worktree,
and every file-writing change happens there, advancing through serial PRs to
`main`. Tasks are Linear issues — durable delegated work starts from an
existing issue, so the roadmap is the queue.

## Delegate

```bash
lf task run INF-123                          # run an existing Linear issue
lf task start <project-id> "add passkeys"    # create the issue, then run it
pbpaste | lf task start <project-id>         # report from stdin; first line is the title
lf task run INF-124 --stack-on INF-123       # dependent work before the parent PR merges
lf task run INF-125 --headless               # no interactive surface
lf project run <project-id>                  # start the supervising Project Session
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

Direction is appended before Loopflow attempts delivery. The same Steer stays
available to the next execution boundary whether a provider accepts a live
send, rejects it, or races the current Turn:

```bash
lf task steer INF-123 "support passkeys too"       # durable direction
lf task interrupt INF-123                          # end the current boundary only
lf task steer INF-123 "stop; make this a smaller PR"
```

The Task and Project wrappers resolve familiar Linear ids. Parent Runs use the
same stable Work control surface:

```bash
lf work status task task_... --json
lf work steer task task_... "show the failing fixture" --json
lf work continue task task_...                      # continue past current Feedback
```

A Steer receipt reports immutable delivery attempts, not incorporation. A
later successful boundary's Basis is the proof that the direction was applied.
`interrupt` carries no replacement text and does not end the Run.

Work survives its provider process. `lf task resume INF-123 --model codex`
selects another provider without losing durable direction, the worktree, or
the PR chain; `lf task recover` restarts abandoned Task pursuit on the same
worktree.

`lf project` carries the same control verbs one level up: `steer`, `interrupt`,
`wait`, `resume`, and `attach`.

## Remember, with receipts

```bash
lf memory add "workers report via stream" --receipt chat_turn:turn-3
lf receipt show chat_turn:turn-3 --json
lf receipt show pr:acme/app#42 --json
```

`lf memory add` records a durable wave learning bound to evidence. A receipt
token (`kind:reference` — `chat_turn:`, `run:`, `pr:`, …) resolves to the
canonical local record, so a claim in memory can always be drilled to what
actually happened. `wave/<name>/MEMORY.md` is server-owned; agents write
through the API, never the file.

## Ship

Three commitment levels, all headless — pick by how done the work is and who
lands it:

```bash
lf pr publish    # make work visible mid-stream; the agent's default verb
lf pr submit     # done, a human clicks merge
lf pr land       # done, loopflow lands it hands-off; -c completes the Task
```

`lf pr open` is the one presenting verb — it opens a browser. Agents reach for
it only when a human asked to see the PR.

## Observe

Every read the conducting surfaces offer is `--json`:

```bash
lf ls --json                # every durable Wave and its Home/runtime evidence
lf status <wave> --json     # live Project → Task hierarchy
lf roadmap --json           # every open Task across every wave
lf runs --task INF-123 --json
lf trace <exec-id> --json
```

`lf ls` is the registry plane, `lf status` is the focused operational view,
and `lf roadmap` joins the current Linear plan to that runtime truth. Agents
consume those projections; they do not rebuild the joins.

Wave lifecycle uses the same durable `WorkStatus` as Project and Task work:
`ready`, `running`, `waiting`, `done`, or `abandoned`. `live` stays separate —
it is reachability evidence, not lifecycle state.

[Conducting →](conducting.md) covers the full monitoring surface. The Mac app is
built on exactly these calls — it keeps no second database.

## The contract

Every launched agent gets `LOOPFLOW.md` — the operating contract — in context
(opt out with `--no-loopflow`). Its spine:

- Route git, worktrees, and PRs through `lf`; never raw `git worktree`.
- Execute here first; delegation must make the problem smaller.
- Checkpoint and proceed: don't ask permission for reversible work.
- Answer humans in turn text; use typed Work observations and explicit
  Feedback points for parent/child coordination.
- Write repo-specific learnings into `.lf/` and commit them with the work.

Source: `rust/loopflow/src/engine/builtins/LOOPFLOW.md`.

## Next

[Conducting →](conducting.md) · [Waves →](waves.md) · [`lf` reference](lf.md)
