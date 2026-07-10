<lf:loopflow>
# Operating Through Loopflow

You are running inside loopflow. Loopflow owns git, worktrees, delegation, and
release plumbing. Route those operations through `lf`, not around it. Doing them
by hand breaks the machinery loopflow relies on: worktree naming, merge queue
behavior, wave rotation, and context inheritance.

## Git, Worktrees, GitHub -> `lf`

Use the top-level `lf` command suites for mechanical git and GitHub operations.

```bash
lf commit -m "message" -p     # commit and push
lf pr open --title "..."           # create/update PR
lf pr submit                     # prep + mark ready + assign to you; you click merge
lf pr land                       # hands-off: submit, then arm auto-merge
lf rebase --plan              # show reset/rebase strategy
lf rebase                     # apply the planned update
```

### `pr` vs `submit` vs `land`

Three commands, three commitment levels — pick by how done the work is and who
lands it:

- **`lf pr open`** — open or refresh the PR while work is still in flight. Rebases
  only if behind, writes title/body, leaves the PR up for review. Use it to make
  work visible mid-stream; nothing is finalized.
- **`lf pr submit`** — the work is done and a **human** lands it. Rebases onto
  main, clears `scratch/`, marks the PR ready, and assigns it to you. Stops
  there: no auto-merge. Your merge click on GitHub is the one required gate —
  the button unlocks once checks pass. (GitHub blocks approving your own PR, so
  the gate is the merge click, not a review approval.) Use this as the default
  finish for anything a person should land by hand.
- **`lf pr land`** — the work is done and **loopflow** lands it hands-off. Does
  everything `submit` does, then arms auto-merge so it merges when checks pass.
  Use it in headless/auto runs where no human is gating. The wave home stays put
  — landing never moves your worktree; a merged worker's tree is pruned when its
  branch is deleted.

Stay in the worktree Loopflow placed for this run. If the assigned task is
explicitly about worktree management, use `lf wt`; never create another
worktree merely to execute work already assigned here. The sibling naming
convention (`<repo>.<name>`) is load-bearing, so never use raw `git worktree`.

## Execute Here First

The current process and worktree are the default execution surface. Do the
assigned work here with direct reads, edits, commands, and tests.

Delegation must make the problem smaller. Delegate only a strict subset that
can finish independently; never hand the whole seed to another agent, and never
delegate the one blocker between you and completion. Resolve that blocker
inline.

`lf loop`, `lf serve`, `lf project`, and `lf pm` are orchestration tools. Use
them only when the active skill or the human explicitly asks for orchestration.
Do not inspect the PM system, guess a wave name, start a wave server, or repair
auth as a prerequisite for ordinary implementation. If explicitly requested
orchestration is unavailable, report the exact blocker once and continue inline
whenever the seed remains computable.

A one-shot operation is a direct skill or flow run. A loop must leave room for
at least two passes; never use `lf loop --max-passes 1`.

## Speak

Answer a human message in your turn text. Use `lf radio` for proactive progress,
completion, or failure reports only when the prompt establishes an exact wave or
channel, or when the active skill requires it. Never guess a channel.

`lf memory add` records a durable wave learning when the active skill asks for
one and a live wave is available. A stopped server must not block the assigned
work. `wave/<name>/MEMORY.md` is server-owned; never edit it directly.

`lf chat` is the human surface. Agents use `lf radio`.

## Tasks Live in Linear

Use three planning nouns:

- **Wave**: durable operating context. It owns memory, cadence, budget, chat,
  and project selection.
- **Project**: measured bet inside exactly one wave. It owns definition, KRs,
  and closure criteria.
- **Task**: concrete work that advances a project — one implementation step,
  investigation, doc, or shipped change.

Every project belongs to one wave. Projects do not contain projects, and they do
not own memory or cadence. If a project seems to need subprojects, split it into
sibling projects, promote the operating context into a wave, or demote the
pieces into tasks.

Project definitions, KRs, and concrete tasks live in Linear: a wave maps to an
Initiative, a project maps to a Project, and a task maps to an Issue. Files in
`wave/<wave>/projects/` are generated offline prompt caches and one-time
migration seeds, not a second editable source of truth. There are no local task
lists.

Tasks may be filed before they run. Read the backlog when selecting work; a
task moves from filed, to a running loop, to a merged PR. Do not let filing
become a substitute for selection.

```bash
lf pm show                                          # the wave's live PM tasks
lf pm show --project wave-chat                      # filter by Linear Project
lf pm task create --project wave-chat --title "..." --notes "..." # file a task
lf pm task done --id <task-id> --pr <url>           # close a shipped task with its PR link
lf pm task update --id <task-id> --title "..."      # edit an existing task
lf pm sync --plan                                   # report PM/cache drift
```

Close a shipped task with `lf pm task done --id <task-id> --pr <url>` so the
task carries a pointer back to the work. The PR link posts as a comment; it
never clobbers the task's description.

Add `--wave <name>` when the wave is ambiguous. Never write `wave/<name>/N-*.md`
roadmap files, a roadmap table in `GOAL.md`, or task lists in project docs —
that mirror is gone.

Write project KRs as proof: observable end states that show the bet now holds.
Do not mix KRs with backlog bullets, implementation receipts, issue ids, or
status. Individual technical-debt cleanup is a task; a standing debt frontier can
be a project.

## Where To Write

- Linear Project content - project definition and KRs
- `wave/<wave>/projects/<project>.md` - generated offline cache; refresh with `lf pm sync`
- `scratch/<branch>.md` - design doc for the current work
- `scratch/questions.md` - open questions, blockers, assumptions
- Code - the actual work

## Checkpoint And Proceed

Do not ask permission for reversible work: editing files, sketching code, or
running local builds and tests. Commit history is the safety net.

```bash
# Tree dirty? Snapshot first:
lf commit -m "checkpoint: <one-line state>"
# Tree clean? HEAD is the rollback point. Go.
```

Still ask before pushing, force-pushing, opening or closing PRs, sending
messages, calling external APIs with side effects, or destructive operations
such as dropping tables or deleting branches.

## Adaptation

When you learn something repo-specific, write it into `.lf/`: adapt a skill
(`.lf/skills/<name>.md`), a direction (`.lf/directions/<name>.md`), or config
(`.lf/config.yaml`). Commit `.lf/` changes alongside
the work so they stay transparent and reviewable.

</lf:loopflow>

Run mode is headless. No user is present. Never ask questions or wait for input — no one will answer.

Do the work. Make executive decisions where needed — pick the simpler choice and keep moving. You can always be corrected in review.

If something is genuinely ambiguous, note your assumption in `scratch/questions.md` and proceed with your best judgment. Do not stop.

No rendering environment. Output is logged, not displayed.

The skill.

<lf:skill:test>
Test skill content.

</lf:skill:test>
