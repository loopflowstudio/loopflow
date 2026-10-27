# README: explain a Task first (Jack Heart, 2026-10-26)

Jack: explain per-Task first. `lf` is a task system, but an lf Task is special:
you get a workflow and a session, plus accounts and history (Session history,
Graph history). Spell out that the Task's agent Session is one persistent chat
thread where everything goes: one context. Alongside it, the workflow defines the
background steps that happen as you progress through the Task's stages
(designing, implementing, …). Then you can organize Tasks into Waves, which
define defaults for all of these.

Apply to `README.md`: replace the opening paragraph and steps 1–3 with the text
below; keep "What else it does" and everything after. Keep every command true on
this branch (check `docs/lf-reference.md`).

---

`lf` is how your agent orchestra conducts itself. It's a task system for coding
agents: you say what you want, and each Task gets its own conversation, its own
workflow and its own pull request. Organize Tasks into Waves to give each area of
work its own defaults. It works with Claude Code, Codex, OpenCode and Pi, on your
existing subscriptions, inside whatever terminal, multiplexer and tracker you
already use.

**1. A Task is a conversation with a workflow behind it.**

```bash
lf task create --title "fix the flaky retry test"   # prints the Task ID
lf -b task run <ID>                                   # its worktree, its agents, its pull request
lf resume --task <ID>                                 # its conversation, from any terminal
```

Each Task has:

- **One conversation.** A persistent chat thread with your agent where everything
  about the Task goes: you talk in it, reviews happen in it, results come back to
  it. One context, on whichever harness and login it runs; it outlives terminals
  and crashes.
- **A workflow.** The stages the Task moves through (design, build, demo, land)
  and the Graphs that run in the background between them, each in the Task's own
  worktree and branch. You take part at the stages; the steps in between run
  without you, or all of them do in `auto`.
- **Its own pull request, branch and tracker issue,** kept together.
- **Its history.** Every conversation, Graph run and step, and what each cost.

```bash
lf comment <ID> "keep the retry budget at 3"   # reaches the next step, in its context
lf task status <ID>                            # where it is in its workflow, and what it needs from you
lf history usage --task <ID>                   # what it cost
lf land --wait-and-fix                         # merge when checks pass; fix them if not
```

The last install line teaches your own coding agent to do all of this, so you
can just ask it: *"Split the auth cleanup into Tasks and run them with lf."*
[What the skill teaches](skills/loopflow/README.md).

**2. Write down how work runs.** A workflow's background steps are Graphs: a
Graph is a YAML list of Skills, and a Skill is a Markdown file, both in your
repository and reviewed like code. Each step can name its own agent.

```markdown
<!-- .lf/skills/audit.md -->
Audit auth changes on this branch. Fix missing validation and untested paths.
```

```yaml
# .lf/graphs/ship-api.yaml
- implement   # leaves commits on the branch
- audit
- gate        # leaves passing checks
```

```bash
lf audit                        # one Skill, here and now
lf -b task run <ID> ship-api    # one Task through the Graph
```

Loopflow ships reasonable workflows for software engineering, and they are
templates. Copy one with `lf graph customize pursue`, change it, or write down
the way you work and share it.

**3. Organize Tasks into Waves.** A Wave is an area of work with its own goal,
memory and plan, and its own defaults for every Task in it: the workflow, the
machine, and more as they land. Your repository is the Wave for Tasks without one.

```bash
lf wave workflow set auto infrastructure   # infrastructure Tasks go from request to merged PR without you
lf machine use mini infrastructure          # and run on your mini
```

**Without a Task,** `lf` still launches any agent or Skill directly:

```bash
lf : "fix the flaky retry test"            # an agent, on whichever of your logins has room
lf -a codex : "fix the flaky retry test"   # the same thing on Codex
lf debug -c                                # a Skill: paste an error, it finds and fixes the cause
lf resume --all                            # after a crash or reboot, every interrupted conversation comes back
```
