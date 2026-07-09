---
layout: default
title: Loopflow
---

# Loopflow

Orchestrate waves of autonomous work.

## Try it

```bash
curl -fsSL https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh | sh
git clone https://github.com/loopflowstudio/loopflow-demos
cd loopflow-demos/calculator
python -m pytest test_calc.py    # see the bug
# copy error to clipboard
lf debug -c                       # fix it
```

## Run a Wave

```bash
# author wave/engbot/GOAL.md, then:
lf wave engbot       # start the wave agent (Ctrl-C to stop)
tmux ls              # live sessions — the wave agent and its workers
tmux attach -t <name>  # watch one work
```

---

## The Journey

| Where you are | What to read |
|---|---|
| Just installed, want to try it | [Try It](#try-it) above, then [Get Started](getting-started.md) |
| Building features with skills and flows | [Get Started → Build Features](getting-started.md#build-features) |
| Ready to automate with waves | [Wave Authoring](wave-authoring.md) |
| Understanding how the pieces fit | [Architecture](architecture.md) |
| Running agents on a server | [Get Started → Go Remote](getting-started.md#go-remote) |

---

## Why Flows?

Skills are atomic. Flows are how work actually gets done.

**Linear flows** run a sequence of steps — each step names a skill, an op, or a flow — with automatic commits:
```
design → implement → polish
```

**Parallel flows** branch and join:
```
design ──┬──> impl-api ──┬──> integrate
         └──> impl-ui ───┘
```

**Fork** explores multiple approaches and synthesizes:
```
Fork ──┬──> impl (infra) ──┐
       ├──> impl (ux)    ──┼──> synthesize
       └──> impl (ceo)   ──┘
```

The synthesizer doesn't just pick a winner—it documents why approaches differed.

---

## The Model

| Atom | What it does | File |
|------|--------------|------|
| **Skill** | Runs a prompt with assembled context | `.lf/skills/*.md` |
| **Flow** | Chains skills together | `.lf/flows/*.yaml` |
| **Wave** | Durable operating context with memory, cadence, chat, and project selection | `wave/<name>/` |
| **Goal** | A wave's intent and loop prompt | `wave/<name>/GOAL.md` |
| **Project** | Measured bet inside exactly one wave | `wave/<name>/projects/*.md` |
| **Task** | Concrete work that advances a project | Linear via `lf op pm` |
| **Memory** | What a wave remembers between loops | `wave/<name>/MEMORY.md` |
| **Direction** | Shapes judgment and intent | `.lf/directions/*.md` |
| **Cron** | Scheduled supplementary flow | goal frontmatter |

A wave is a named agent with a goal. Everything that defines its durable
operating context — goal, memory, projects, routing judgment, crons — is
authored in the repo. Concrete tasks live in Linear. Crons live in `GOAL.md`
frontmatter and are fired by the wave's resident flowloop. lfd serves wave
status and live sessions to clients.

```markdown
---
workers: 2
---

## Objective

Run one loop iteration for this wave.

## Measures

- **Key Results**: backlog is empty.

## Process

Read the live tasks, pick the next useful move, and dispatch the appropriate flow.
```

---

## Skill

A markdown file that tells the coding agent what to do.

```markdown
# .lf/skills/audit.md

Audit auth changes on this branch.
Check for:
- Missing validation
- Confusing errors
- Gaps in tests

Fix any issues you find.
```

```bash
lf audit                      # run the skill
lf audit: focus on auth       # pass arguments
```

Skills run to completion. Built-ins: `debug`, `design`, `implement`, `gate`, `qa`, `lint`.

**Where to put skills:** `.lf/skills/` is canonical. Symlink for Claude Code compatibility:

```bash
ln -s ../.lf/skills .claude/commands
```

### Token compression

Use `token-compress` when a workflow needs to fit more context into a smaller budget without losing the important shape.

```bash
lf token-compress: Compress this release context to 1200 tokens
```

Compression is not truncation. A good compression pass preserves decisions, rationale, risks, open questions, names, dates, versions, paths, commands, URLs, and identifiers. It groups repetition before cutting. If the budget forces meaningful omissions, it says what was omitted.

Release systems, long-running waves, and handoffs should compress source material before summarizing it. Do not take the first N commits, first N lines, or latest N messages as a substitute for understanding the whole input.

---

## Flow

Chains skills together with commits between them.

```yaml
# .lf/flows/ship-api.yaml
- implement
- compress
- lint
- gate
```

```bash
lf ship-api
```

Or chain manually:

```bash
lf design: add auth && lf implement && lf gate
```

---

## Direction

Shapes how the coding agent judges and responds.

```markdown
# .lf/directions/ux.md

Optimize for user experience quality: visibility, feedback, consistency.

## Success

A design doc in scratch/ that another engineer could implement from.
```

```bash
lf gate --direction ux
lf gate --direction ux,clarity    # stack multiple
```

Directions compose. A `ux` direction sets user-facing intent. A `clarity`
direction adds code-model rigor. Stack them to get both.

---

## Docs

Repo docs are not auto-injected. A skill sees only the essentials (see [What's Auto-Included](#whats-auto-included)); prefetch anything else with `--docs` — a file, a glob, or a directory:

```bash
lf gate --docs VISUAL_DESIGN.md      # one doc
lf gate --docs 'docs/*.md'           # a glob
lf gate --docs swift/                # every .md under a directory
```

Point `--docs` at what a task actually needs, and let `AGENTS.md` point at the rest.

---

## Where Files Live

```
.lf/                      # Repo config and extensions
  config.yaml             # Model, context defaults
  skills/                  # Skill prompts (preferred)
  directions/             # Judgment and intent
  flows/                  # Flow definitions
.claude/commands/         # Skills (Claude Code compatible)
scratch/                  # PR scratchpad (cleared on merge)
wave/                     # Wave plans (persists)
~/.lf/                    # Global config and skills
```

### scratch/ vs wave/

| | scratch/ | wave/ |
|---|---|---|
| **Lifespan** | Dies with the PR | Lives forever |
| **Purpose** | Current work | Forward-looking plans |
| **Location** | Root only | Root + per-folder |
| **Example** | "Add auth" spec | "How auth should work long-term" |

`wave/<name>/` holds a wave's goal, memory, and plans. Root `wave/` is auto-included in every prompt.

### What's Auto-Included

Every skill sees your agent doc (`AGENTS.md` / `CLAUDE.md` / `STYLE.md`), `LOOPFLOW.md`, `scratch/`, and `wave/`. Nothing else — pull in extra docs with `--docs`, branch file bodies with `--diff-files`, and raw patches with `--diff`.

---

## Next

[Get Started →](getting-started.md) · [Wave Authoring →](wave-authoring.md) · [Waves →](waves.md)

## Reference

[`lf` commands](lf.md) · [`lf` operations](lfop.md) · [`lfd` commands](lfd.md) · [Configuration](config.md)
