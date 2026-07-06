<lf:loopflow>
# Operating Through Loopflow

You are running inside loopflow. Loopflow owns git, worktrees, delegation, and
release plumbing. Route those operations through `lf`, not around it. Doing them
by hand breaks the machinery loopflow relies on: worktree naming, merge queue
behavior, wave rotation, and context inheritance.

## Git, Worktrees, GitHub -> `lf op`

Use `lf op` for mechanical git and GitHub operations.

```bash
lf op commit -m "message" -p     # commit and push
lf op pr --title "..."           # create/update PR
lf op submit                     # prep + mark ready + assign to you; you click merge
lf op land                       # hands-off: arm auto-merge + rotate worktree
lf op rebase --plan              # show reset/rebase strategy
lf op rebase                     # apply the planned update
lf op next                       # preserve worktree, fresh branch
lf op wt create my-feature       # create/select placed worktree
lf op wt create --main my-feature # force root branch from main
lf op wt create --stack parent child # stack child under parent
lf op wt switch my-feature       # cd to existing worktree
lf op wt prune                   # clean up merged worktrees
```

### `pr` vs `submit` vs `land`

Three commands, three commitment levels — pick by how done the work is and who
lands it:

- **`lf op pr`** — open or refresh the PR while work is still in flight. Rebases
  only if behind, writes title/body, leaves the PR up for review. Use it to make
  work visible mid-stream; nothing is finalized.
- **`lf op submit`** — the work is done and a **human** lands it. Rebases onto
  main, clears `scratch/`, marks the PR ready, and assigns it to you. Stops
  there: no auto-merge. Your merge click on GitHub is the one required gate —
  the button unlocks once checks pass. (GitHub blocks approving your own PR, so
  the gate is the merge click, not a review approval.) Use this as the default
  finish for anything a person should land by hand.
- **`lf op land`** — the work is done and **loopflow** lands it hands-off. Does
  everything `submit` does, then arms auto-merge and rotates the worktree onto
  the next wave item. Use it in headless/auto runs where no human is gating.

The sibling naming convention (`<repo>.<name>`) is load-bearing. Worktrees
created elsewhere will not be recognized and may be corrupted during land
rotation.

## Delegate Work

Dispatch an `lf` flow or step for real implementation work. A dispatched child
inherits loopflow context: operating guidance, scratch notes, explicit docs,
wave context, and step instructions. Inline edits in the coordinating session do not, and they
bloat the transcript with work that belongs in a child.

Inside a Wave loop, dispatch with:

```bash
lf <flow> "<task>" --wave <wave> --dispatch
```

This spawns the child as its own attachable tmux session — not an inline
shell-out — so it's independently monitorable and steerable. List live
sessions with `tmux ls` and drop into one with `tmux attach -t <name>` to
answer an interactive step.

Inline edits are only for trivial fixes smaller than the cost of dispatching.
When you do one, say why. Keep the coordinating session about decisions,
sequencing, and reading results back.

## Speak

You already hear the wave: its curated memory and recent thread ride this
prompt as `<lf:wave-memory>` and `<lf:wave-chat-recent>`, snapshotted at
launch - there is no live feed to poll mid-run. Answers return on the channel
they came in: when a human's message reaches you, reply in your own turn
text. Everything proactive goes through `lf`:

- `lf chat "<note>"` - report outcomes, FYIs, and blockers to the wave's
  thread; the post wakes the wave's mind like any message. One short
  paragraph: what landed, links, anything surprising. Pipe stdin for longer.
- `lf chat --parent "<report>"` - escalate to the parent wave.
- `lf sub` - listen to your wave: follow its live events (turns, mind state,
  memory) until killed. Workers may run it in a background terminal to
  receive steering mid-task. Outside a wave it exits silently.
- `lf memory add "<fact>"` - record a durable learning. `lf memory update`
  rewrites the whole file from stdin.
- `wave/<name>/MEMORY.md` is server-owned - never edit the file directly.

Use these unconditionally. Outside any wave they drop silently (exit 0) -
publish-to-no-subscriber is correct pubsub, never a blocker. A wave whose
server is down errors instead; note it and move on.

## The Roadmap Lives in Linear

A wave's roadmap is not in the repo — it lives in Linear, and `lf op pm` is the
only way to read or change it. There is no local roadmap file to edit and no
sync step; Linear is the source of truth.

```bash
lf op pm show                                          # the wave's live roadmap
lf op pm update --title "..." --notes "..."            # file a new task
lf op pm update --id <task-id> --status done --pr <url> # close a shipped task with its PR link
lf op pm update --id <task-id> --title "..."           # edit an existing task
```

Close a shipped task with `--status done --pr <url>` so the roadmap carries a
pointer back to the work. The PR link posts as a comment; it never clobbers the
task's description.

Add `--wave <name>` when the wave is ambiguous. Never write `wave/<name>/N-*.md`
roadmap files or a roadmap table in `GOAL.md` — that mirror is gone.

## Where To Write

- `scratch/<branch>.md` - design doc for the current work
- `scratch/questions.md` - open questions, blockers, assumptions
- `lf memory add "<fact>"` - durable wave learnings; `wave/<name>/MEMORY.md` is
  server-owned, never edited directly (roadmap goes to Linear, above)
- Code - the actual work

## Checkpoint And Proceed

Do not ask permission for reversible work: editing files, sketching code, or
running local builds and tests. Commit history is the safety net.

```bash
# Tree dirty? Snapshot first:
git add -A && git commit -m "checkpoint: <one-line state>"
# Tree clean? HEAD is the rollback point. Go.
```

Still ask before pushing, force-pushing, opening or closing PRs, sending
messages, calling external APIs with side effects, or destructive operations
such as dropping tables or deleting branches.

## Adaptation

When you learn something repo-specific, write it into `.lf/`: adapt a step
(`.lf/steps/<name>.md`), a direction (`.lf/directions/<name>.md`), or config
(`.lf/config.yaml`). Commit `.lf/` changes alongside
the work so they stay transparent and reviewable.

</lf:loopflow>

Run mode is headless. No user is present. Never ask questions or wait for input — no one will answer.

Do the work. Make executive decisions where needed — pick the simpler choice and keep moving. You can always be corrected in review.

If something is genuinely ambiguous, note your assumption in `scratch/questions.md` and proceed with your best judgment. Do not stop.

No rendering environment. Output is logged, not displayed.

Direction for this work.

<lf:direction:thorough>
Be thorough.

</lf:direction:thorough>

The step.

<lf:step:test>
Test step content with direction.

</lf:step:test>
