---
name: loopflow
description: Operate a repository through loopflow (`lf`) — persistent Wave, Project, and Task Work, PRs, and the agent bus. Use when a repo contains `.lf/` or `wave/`, or when the human mentions loopflow, waves, or `lf`.
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

## Caller Authority

An external harness opened by a person acts as a Loopflow **User**. It may read
status and use `lf chat` when the human asks it to inspect or steer a Wave. It
does not become a Wave, Project, or Task worker.

An agent launched by Loopflow is an internal participant. It receives
`LOOPFLOW.md` automatically, reports through an established `lf radio` channel,
and never impersonates the User in chat.

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
lf task status <issue-id> --json             # inspect durable state
lf task wait <issue-id> --until terminal
```

When work feels slow or stuck, run `lf top` before guessing — it shows
last-hour provider throughput and live processes.

## Inspect

When the human asks about Loopflow state, read the shared surfaces instead of
reconstructing it from processes, worktrees, or Linear:

```bash
lf ls --json              # every durable Wave and its Home/runtime evidence
lf status <wave> --json   # one Wave's Work hierarchy, Runs, and attention
lf roadmap --json         # current plan across Waves joined to runtime truth
```

These are read surfaces. `lf status` is the focused operational view;
`lf roadmap` is the planning overlay, not a second runtime model.

## Place And Run

Execution placement is durable state, not authored goal text. A Work names one
stable Home authority; the Home's SSH route may change without moving the Work.

```bash
lf home id                                      # this machine's HomeId
lf work place wave <wave-id> <home-id>          # only while no Run is live
lf start <wave>                                 # route to its placed Home
lf stop <wave>                                  # leave the Home keeper and siblings running
lf ssh <home-id> --remote-native -- lf status <wave> --json
```

Use `--remote-native` for durable remote lifecycle. It forwards no provider,
GitHub, PM, or secret authority; the remote Home uses its installed authority.
Use ordinary `lf ssh <host> -- <command>` only for foreground work that should
borrow the origin's short-lived credential lease.

## Speak

`lf chat` is the User surface. An external harness acting for the human may use
it; Loopflow-launched agents never post there. Internal participants report on
the bus only when the prompt establishes an exact wave or channel — never guess
one:

```bash
lf radio pub --channel <channel> "landed PR #91, tests green"
```

Outside any wave, a publish prints a drop note and exits 0. When the active
skill calls for a durable Wave learning, edit `wave/<name>/MEMORY.md` through
the ordinary repository workflow. Keep it curated rather than appending a
transcript. `update-wave` owns deliberate end-of-work memory curation; no live
Wave is required.

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
