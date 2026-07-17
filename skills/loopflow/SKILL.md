---
name: loopflow
description: Operate a repository through loopflow (`lf`) — persistent waves, delegated task sessions, PRs, and the agent bus. Use when a repo contains `.lf/` or `wave/`, or when the human mentions loopflow, waves, or `lf`.
---

# Operating Through Loopflow

<!-- Published form of the injected operating contract at
     rust/loopflow/src/engine/builtins/LOOPFLOW.md — keep aligned when
     that file changes. Agents launched BY loopflow receive the contract
     automatically; this skill teaches agents that arrived on their own. -->

Loopflow is one binary, `lf`: the command humans type and the API agents call
to launch, steer, and observe other agents. It owns git, worktrees,
delegation, and release plumbing in repos that use it. Route those operations
through `lf`, not around it — doing them by hand breaks worktree placement,
release state, and session context.

Check availability; install only if the human asks:

```bash
lf --version || echo "not installed"
# install: curl -fsSL https://loopflow.studio/install.sh | sh && lf init
```

## Git, Worktrees, GitHub → `lf`

```bash
lf commit -m "message" -p            # commit and push
lf pr publish --title "..."         # push + create/update PR, print state+URL (no browser)
lf pr submit                         # done; a human clicks merge
lf pr land                           # done; loopflow lands it hands-off
lf pr land -c                        # land and complete the owning Task
lf rebase --plan                     # show strategy; bare `lf rebase` applies it
lf task run CHILD --stack-on PARENT  # dependent Task, separate worktree
```

Three commitment levels: **publish** (work in flight — the default "make a
PR" verb), **submit** (done, a human lands it), **land** (done, loopflow
lands it). `lf pr open` opens a browser — only when a human asked to see the
PR.

Stay in the worktree loopflow placed for this run. Never use raw
`git worktree`; the sibling naming convention (`<repo>.<name>`) is
load-bearing.

## Execute Here First

The current process and worktree are the default execution surface. Do the
assigned work here with direct reads, edits, commands, and tests.

Delegation must make the problem smaller: delegate only a strict subset that
can finish independently; never hand off the whole seed or the one blocker
between you and completion.

Use `lf task`, `lf project`, `lf wave`, and `lf pm` only when the active skill
or the human explicitly asks for orchestration. Do not inspect planning state,
guess a Wave, start a server, or repair auth as a prerequisite for ordinary
implementation. Durable delegated work starts from an existing Linear task:

```bash
lf task run <issue-id>                       # durable Task Session, own worktree
lf task steer <issue-id> "smaller approach"  # redirect its active turn
lf task receipt <cmd-id> --until incorporated --timeout 30s --json
lf task wait <issue-id> --until terminal
```

When work feels slow or stuck, run `lf top` before guessing — it shows
last-hour provider throughput and live processes.

## Speak

`lf chat` is the human surface; agents never post there. Report on the bus
only when the prompt establishes an exact wave or channel — never guess one:

```bash
lf radio pub --channel <channel> "landed PR #91, tests green"
lf memory add "<durable learning>" --receipt pr:owner/repo#42
```

Outside any wave, a publish prints a drop note and exits 0, so these verbs
are safe in every prompt. `wave/<name>/MEMORY.md` is server-owned — write
through `lf memory add`, never the file.

## Where To Write

- `scratch/<branch>.md` — design doc for the current work
- `scratch/questions.md` — open questions, blockers, assumptions
- Code — the actual work

## Checkpoint And Proceed

Do not ask permission for reversible work: editing files, sketching code,
running local builds and tests. Tree dirty? `lf commit -m "checkpoint: <state>"`
first. Still ask before pushing, opening or closing PRs, sending messages,
calling external APIs with side effects, or destructive operations.

## Docs

Raw markdown, agent-ready: index at https://loopflow.studio/llms.txt, full
corpus at https://loopflow.studio/llms-full.txt, any page at
`https://loopflow.studio/docs/<slug>.md` (agent-api, waves, conducting, lf,
authoring, architecture, config, troubleshooting).
