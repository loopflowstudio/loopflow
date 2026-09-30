---
name: loopflow
description: Operate a repository through loopflow (`lf`) — persistent Wave, Project, and Task Work, PRs, and typed control. Use when a repo contains `.lf/` or `wave/`, or when the user mentions loopflow, waves, or `lf`.
---

# Operating Through Loopflow

<!-- Published form of the injected operating contract at
     rust/loopflow/src/engine/builtins/LOOPFLOW.md — keep aligned when
     that file changes. Agents launched BY loopflow receive the contract
     automatically; this skill teaches agents that arrived on their own. -->

Loopflow is one binary, `lf`: the CLI for daily work and the API agents call
to launch, steer, and observe other agents. It owns git, worktrees,
delegation, and release plumbing in repos that use it. Route those operations
through `lf`, not around it — doing them by hand breaks worktree placement,
release state, and Run authority.

Check availability; install only if the user asks:

```bash
lf --version || echo "not installed"
# install: curl -fsSL https://loopflow.studio/install.sh | sh && lf init
```

## Caller Authority

An external harness opened by a person acts as a Loopflow **User**. It may read
status and start a Session when the user asks it to inspect or steer a Wave. It
does not become a Wave, Project, or Task worker.

An agent launched by Loopflow is a Loopflow-launched internal participant. It
receives `LOOPFLOW.md` automatically, writes through its exact Work/Run
authority, and never impersonates the User in chat.

## Git, Worktrees, GitHub → `lf`

```bash
lf task commit -m "message" # commit locally
lf task pr open # push, prepare a draft, and open its review page
lf task pr publish --title "..."         # push + create/update PR, print state+URL (no browser)
lf task pr submit                         # done; the user clicks merge
lf task pr land                           # done; loopflow lands it hands-off
lf task pr land -c                        # land and complete the owning Task
lf task rebase --plan                     # show strategy; bare `lf task rebase` applies it
lf task run CHILD --stack-on PARENT  # dependent Task, separate worktree
```

**Publish** makes a PR ready for review without opening a browser. **Submit**
prepares it for the user's merge click; **arm/land** request auto-merge.
`lf task pr open` creates or updates a draft and opens its page when a person asks
to see it. Existing ready PRs stay ready; opening a draft does not publish it.
Publish/submit/arm/land make drafts ready.

Stay in the worktree loopflow placed for this run. Never use raw
`git worktree`; the sibling naming convention (`<repo>.<name>`) is
load-bearing.

## Execute Here First

The current process and worktree are the default execution surface. Do the
assigned work here with direct reads, edits, commands, and tests.

Delegation must make the problem smaller: delegate only a strict subset that
can finish independently; never hand off the whole seed or the one blocker
between you and completion.

Use `lf task`, `lf wave`, and `lf repo` only when the active skill
or the user explicitly asks for orchestration. Do not inspect planning state,
guess a Wave, start a server, or repair auth as a prerequisite for ordinary
implementation. Durable delegated work starts from an existing Linear task:

```bash
lf task run <issue-id>                       # durable Task Work, own worktree
lf task comment <issue-id> "smaller approach" # post direction for the Task advancer
lf task status <issue-id> --json             # inspect durable state
lf task wait <issue-id> --until terminal
```

When work feels slow or stuck, run `lf top` before guessing — it shows
last-hour provider throughput and live processes.

## Inspect

When the user asks about Loopflow state, read the shared surfaces instead of
reconstructing it from processes, worktrees, or Linear:

```bash
lf wave list --json              # every durable Wave and its Home/runtime evidence
lf wave status <wave> --json   # one Wave's Work hierarchy, Runs, and Task conditions
lf roadmap --json         # current plan across Waves joined to runtime truth
```

These are read surfaces. `lf wave status` is the focused operational view;
`lf roadmap` is the planning overlay, not a second runtime model.

## Place And Run

Execution placement is durable state, not authored goal text. A Work names one
stable Home authority; the Home's SSH route may change without moving the Work.

```bash
lf home id                                      # this machine's HomeId
lf wave place <wave-id> <home-id>          # only while no Run is live
lf --wave <wave> wave/operate                    # one finite pass here
lf ssh <home-id> status <wave> --json           # inspect it on that Home
lf ssh <home-id> --wave <wave> wave/operate       # one finite pass there
```

`lf ssh` runs only the target machine's `lf`; the inner `lf` and `--` separator
are implicit. Foreground commands can choose from origin-forwarded and
target-local subscription accounts. Durable processes scrub forwarded provider,
GitHub, PM, and secret authority before detaching and use credentials installed
on their machine.

## Speak

Use names in persisted Tasks, PRs, docs, memory, and summaries; use “you” in
session conversation. Stored transcripts keep their conversational wording;
summaries extracted from them use names. Use known preferred names and preserve
unknown attribution instead of guessing who made a request.

Answer the user's message in turn text. Tasks, Projects, and Waves communicate
through durable Task facts and targeted Ask Sessions.

Sessions are the conversation surface. Work Steer is the live correction path. When the
active skill calls for a durable Wave learning, edit `wave/<name>/MEMORY.md`
through the ordinary repository workflow. Keep it curated rather than appending
a transcript. `realign` reconciles memory with the plan and code; no live Wave
is required.

## Where To Write

- `scratch/<branch>.md` — design doc for the current work
- `scratch/questions.md` — open questions, blockers, assumptions
- Code — the actual work

## Checkpoint And Proceed

Do not ask permission for reversible work: editing files, sketching code,
running local builds and tests. Tree dirty? `lf task commit -m "checkpoint: <state>"`
first. Still ask before pushing, opening or closing PRs, sending messages,
calling external APIs with side effects, or destructive operations.

## Docs

Raw markdown, agent-ready: index at https://loopflow.studio/llms.txt, full
corpus at https://loopflow.studio/llms-full.txt, any page at
`https://loopflow.studio/docs/<slug>.md` (agent-api, waves, conducting, lf,
authoring, architecture, config, troubleshooting).
