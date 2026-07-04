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
lf op land                       # submit to merge queue
lf op rebase                     # rebase onto main
lf op next                       # preserve worktree, fresh branch
lf op wt create my-feature       # sibling worktree ../<repo>.my-feature
lf op wt switch my-feature       # cd to existing worktree
lf op wt prune                   # clean up merged worktrees
```

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
lfq worker run <wave> --flow <flow> --task "<task>"
```

This spawns the child as its own attachable tmux session — not an inline
shell-out — so it's independently monitorable and steerable. Watch it with
`lfq sessions` (live sessions, needs-input flagged) and drop into one with
`lfq attach <id>` to answer an interactive step.

Inline edits are only for trivial fixes smaller than the cost of dispatching.
When you do one, say why. Keep the coordinating session about decisions,
sequencing, and reading results back.

## The Roadmap Lives in Asana

A wave's roadmap is not in the repo — it lives in Asana, and `lf op pm` is the
only way to read or change it. There is no local roadmap file to edit and no
sync step; Asana is the source of truth.

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
- `wave/<name>/MEMORY.md` - durable wave learnings (roadmap goes to Asana, above)
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

<lf:wave name="rust">
You are building toward the rust program of work.
Wave context is included in docs below.

## Wave memory

Persistent memory at wave/rust/MEMORY.md. Read it before every iteration; its current
contents, when any, ride this prompt's wave-memory section.
Keep it compact enough to include every iteration: correct stale entries,
add durable observations, and delete session-specific notes.

Suggested sections — Patterns, Preferences, Learnings — but add your own as needed.
- Patterns: codebase conventions, architecture, how things connect
- Preferences: user workflow, tool choices, communication norms
- Learnings: what worked, what failed, surprises

What belongs elsewhere:
- architectural decisions → wave docs or explicit docs
- design rationale → scratch/ or wave plan
- session-specific notes → nowhere (let them die)

How to update:
- Edit within sections. Don't rewrite the whole file.
- Correct or remove entries that are wrong or stale.
- Use absolute dates, not "today" or "recently".
- When a section grows large, promote stable entries to wave docs or explicit docs and trim.
</lf:wave>

<lf:wave-memory>
- Keep prompts concise and concrete.
- Prefer behavior-focused tests over mock wiring.
</lf:wave-memory>

Scratch design artifacts and working notes.

<lf:scratch>
<lf:file path="scratch/design.md">
# Design

Current design notes.

</lf:file>
</lf:scratch>

Reference files for this task. Includes parent documentation for context.
<lf:files>
<lf:file path="wave/rust/README.md">
# Rust Roadmap

Overview of Rust work.

</lf:file>
<lf:file path="README.md">
# Test Repo

Root readme.

</lf:file>
</lf:files>

The step.

<lf:step:test>
# Test step

Do the thing.

</lf:step:test>
