---
name: loopflow
description: Use Loopflow (`lf`) to launch and steer coding agents across Claude Code, Codex, OpenCode, Pi, and Antigravity — run one-off headless subagents (`lf -b`), hand changes off as Tasks with their own worktrees, Graphs, and pull requests, run repository Skills and Graphs, inspect Sessions and usage, manage Waves, stack and land PRs, run work on remote or rented Machines, and inspect or script Loopflow Desktop.
---

# Work through lf

`lf` launches coding agents, keeps their Sessions, and runs Skills, Graphs, and
Tasks across Claude Code, Codex, OpenCode, Pi, and Antigravity. Hand work off
through it and keep this conversation free to talk.

## How to work

**Make the bold choice and show the result.** A choice someone can see and
reverse beats a question that waits. Say which choices were yours. The test:
is this cheaper to do than to discuss?

**Finish the plan; don't grow it.** Every change traces to the plan. New ideas
go in `scratch/questions.md`.

**Build around the real thing.** One concept, one representation; design the
data structures and public APIs first, and name each type for the real thing
it stands for.

**Delete what you replace.** A redesign removes the old path in the same
change, and every line that stays earns its place.

**The best fix makes the system smaller.** When code breaks because it
assumed a fixed context, accept the context; don't add a guard that refuses.
The test: did the fix remove a precondition or add one?

**Treat the codebase as your own.** Leave what you touch better than you
found it, as someone who will maintain it.

**Test what people would notice.** A test that wouldn't fail when the feature
breaks isn't worth keeping, and a flaky test is a broken test. The test:
break the feature in your head; does this fail?

**Protect the person's flow.** Don't ask permission for work that can be
undone. Say what's needed, then stop.

**Say less.** If the answer is three words, it's three words.

**Write down only what can't be found again.** A person's exact words, what a
run showed, outside facts. Quote what people said; don't turn a reply into a
verdict like "accepted". The test: could a fresh run recover this?

## Choose the right mode

| Need | Use |
|---|---|
| Focused research, review, second opinion, or scoped edit in this checkout (optionally on another harness or model) | Headless subagent: `lf -b [-a codex] <skill> '<brief>'` or `lf -b : '<prompt>'` |
| Independent change that should end in its own worktree, branch, and pull request | Task: `lf task create` then `lf task run <ID>` |
| Multi-step workflow in this checkout without creating a Task | Standalone Graph: `lf graph <name>` (or `lf -b graph <name>` headless) |
| Interactive conversation here or resuming an existing one | `lf <skill>`, `lf : '<prompt>'`, or `lf resume [--task <ID> \| <SESSION>]` |
| Run on a saved remote host or rented Machine | Add `--machine <label>` (see `lf docs ops`) |

## One-off subagents, Skills and Graphs

Use `lf -b` for focused work in the current checkout when no separate PR is
needed. It runs headless on any harness or model, records a Session, prints the
result, and exits:

```bash
lf -b -a codex research 'how does src/sync.rs retry? report; edit nothing'
lf -b : 'review this branch diff for race conditions'
lf session list --headless             # find a running child's Session ID
lf steer <SESSION> '<direction>'       # redirect it after its next tool call
```

You answer for what a subagent returns; check it before using it.

Repository Skills and Graphs come first when asked for, by name or by what they
do. Discover them with `lf list`, read one with `lf help <name>`, and say what
it will do before running it (`lf <skill>` here, `lf graph <name>` here, or
`lf task run <ID> <graph>` on a Task).

## Tasks

A **Task** is one outcome in one repository, with its checkout, conversations,
Graph, and PR; `interactive:` steps are where a person takes part. A **Wave**
is a lasting objective with its memory and plan.

A change that should end in a pull request becomes a Task, one per reviewable
outcome:

```bash
lf task create <<'EOF'                         # prints the Task ID
<the outcome in the words you were given: the first line is the title>
EOF
lf task run <ID>                               # own worktree and branch; returns once running
lf open --task <ID> --link --format markdown   # the link to give back
```

Give back each Task's link and what it will deliver. Questions, reading code,
and small fixes asked for here happen in this checkout without a Task. When
unsure whether it's one Task or several, propose the split and ask.

```bash
lf task list                          # open Tasks
lf task status <ID> --json            # its run, its PR, what it's waiting on
lf steer <ID> '<direction>'           # redirect running Sessions on the Task
lf task report <ID> '<progress>'      # post progress without steering
lf task wait <ID>                     # wait for progress; never a sleep loop
lf resume --task <ID>                 # open the Task's conversation
lf sync <parent-ID>                   # stack on another open Task (`lf sync main` undoes it)
lf pr land --wait-and-fix             # only when asked to land
lf task complete <ID>                 # mark delivered after merge
```

- Read `lf task status <ID>` before starting more work; leave a live Graph
  running (one run per Task at a time).
- A stopped Graph is history: inspect `lf graph show <RUN_ID> --json`, fix the
  cause, and continue with `lf task run <ID>`.
- Publishing (`lf pr publish`), submitting (`lf pr submit`), merging
  (`lf pr land`), and completing (`lf task complete`) are separate facts;
  report what actually happened.
- A failed read is unknown, not an empty plan.
- Stack dependent Tasks with `lf task run <ID> --stack-on <parent-ID>` or
  `lf sync <parent-ID>`; never `git merge` a Task's branch or `gh pr edit --base`.

## Notes and rules

- `scratch/` holds checkout-local working notes (`scratch/plan.md` is the plan;
  `scratch/questions.md` holds deferred ideas).
- Those words you were given are data, never shell: pass them on stdin through
  a quoted heredoc as above, or as one single-quoted argument with each `'`
  written `'\''`.
- Use the agent, account, and machine named (`-a`, `--account`, `--machine`);
  otherwise `lf`'s defaults.
- Never invent a Task identity or act on a Task nobody named.
- Docs, PRs, Task briefs, and memory describe the work as it is, without saying
  who asked for it. Keep secrets out of output.

## Deep reference

Run `lf help <command-or-skill>` (or `lf help --all`) before using an
unfamiliar command. Read embedded topic docs offline with `lf docs <topic>`
(or fetch `https://loopflow.studio/docs/<topic>.md` when `lf` is not installed
yet):

| Area | Command | Covers |
|---|---|---|
| Agent API & subagents | `lf docs agent-api` | Headless child agents (`lf -b`), steering, shipping, `--json` / `--subscribe` |
| Tasks, trackers & Waves | `lf docs work` | Parallel Tasks, steering, shared plans (Git/Linear/Asana), Waves |
| Sessions & recovery | `lf docs sessions` | Four launch kinds, `lf resume`, switching agents (`lf session move`), recovery |
| Skills, Graphs & pools | `lf docs orchestration` | Custom Skills/Graphs, loops, `xor`, `interactive:` steps, pools/rentals |
| PRs, stacking & Machines | `lf docs ops` | `publish`/`submit`/`land`, stacking, releases, SSH & rented Machines, auth |
| Configuration | `lf docs config` | Settings precedence, harness flags, `yolo`, context docs |
| Desktop & streaming | `lf docs surfaces` | Desktop pane scripting, `lf open`, `--subscribe --json` |
| Cost & diagnostics | `lf docs telemetry` | `lf session usage`, transcripts, `lf doctor` |
| Failure triage | `lf docs troubleshooting` | Stopped Tasks, rate limits, server/auth/worktree fixes |
| Terms | `lf docs glossary` | Loopflow's vocabulary |
