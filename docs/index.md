# Loopflow

```bash
curl -fsSL https://loopflow.studio/install.sh | sh
lf init
npx skills add loopflowstudio/loopflow --skill loopflow -g -y   # teach your coding agent to use lf
```

`lf` is the command you type *and* the CLI your coding agent uses to get work
done. Works with Claude Code, Codex, OpenCode and Pi, on your own logins.

**Work: say what you want, and it becomes Tasks that run without you.** Each
Task gets its own worktree, branch and pull request, so ten requests come back
as ten PRs you can review one at a time.

```bash
lf task create --title "rent CPU pods for templates with no GPU"   # prints the Task ID
lf -b task run <ID>
```

**Context: every agent starts where the last one left off.** Each launch carries
the Wave's memory, the Task's notes in `scratch/`, the docs you name and any new
comments, and gets them back after compaction.

```bash
lf comment <ID> "default to the smallest pod"    # reaches the run before its next step
lf gate --docs docs/runpod.md                    # name what it should read
```

**Orchestration: work finishes while you're doing something else.** Runs go to
the agent and machine you pick, Graphs carry them step to step, Waves keep their
Tasks moving, and landing repairs CI until the PR merges. Get as much as
possible off the main thread: plans into code, code into verified code.

```bash
lf --machine mini -b -a codex graph pursue   # a Graph, headless, on Codex, on your mini
lf land --wait-and-fix          # merge when checks pass; fix them if not
```

**Sessions: you never lose a conversation.** Every conversation outlives its
terminal, and new work starts on whichever of your logins has room. Close the
window and come back next week; the conversation and what it knows are still
there.

```bash
lf resume --task <ID>
```

**Skill Graphs: write down how you work.** Skills are Markdown and Graphs are
YAML, files in your repository reviewed like code.

```markdown
<!-- .lf/skills/audit.md -->
Audit auth changes on this branch. Fix missing validation and untested paths.
```

```yaml
# .lf/graphs/ship-api.yaml
- design      # leaves scratch/<task>.md
- implement   # leaves commits on the branch
- audit
- gate        # leaves passing checks
```

```bash
lf audit                 # run one Skill
lf -b graph ship-api     # run the Graph headless
```

Loopflow ships reasonable workflows for software engineering, and they are
templates. Copy one with `lf graph customize pursue`, change it, or write down
the way you work and share it.

## The pieces

```bash
lf -b implement
lf session list --interactive false --json
lf session connect SESSION
lf task run INF-123
```

A Skill supplies instructions; a Graph composes skills and mechanical operations.
A Task owns one change, its checkout and PRs. A Wave keeps the objective
and memory; its current Project holds the plan; the Wave owns its default Workflow.

Process records an actual lf command process. LfSession keeps the conversation,
including headless work. A running Graph is one lf process and the step processes
it starts. A conversation can outlive its command. A Graph whose
command stopped stays as history; fresh work is launched explicitly.

Read the [contract and cutover status](architecture-reference.md#cutover-status)
for the accepted model and remaining implementation, or the [Glossary](glossary.md)
for a term. The Mac app and CLI read the same owners.

## Read by area

| You want to | Read |
|---|---|
| Install it and try one piece of work | [Get Started](getting-started.md) |
| See what got done and what you can pick up | [Conducting](conducting.md) |
| Give it a goal to keep working on | [Waves](waves.md) |
| Share Tasks with coworkers or capture them in a meeting | [Plan and share Tasks](planning.md) |
| Change how it works | [Authoring](authoring.md) |
| Drive it from another AI agent | [The Agent API](agent-api.md) |
| Look up a word | [Glossary](glossary.md) |
| Look up a command | [`lf` reference](lf.md) |

## Shape one conversation

A skill is a markdown file that tells the coding agent what to do:

```markdown
# .lf/skills/audit.md

Audit auth changes on this branch.
Check for missing validation, confusing errors, gaps in tests.
Fix any issues you find.
```

```bash
lf audit                               # run the skill
lf audit: focus on auth                # pass arguments
lf : "fix the typo in README"          # or skip the file entirely
```

A graph chains skills with commits between them:

```yaml
# .lf/graphs/ship-api.yaml
- implement
- compress
- gate
```

```bash
lf ship-api
```

Built-ins cover the common ground: `debug`, `design`, `implement`, `compress`,
`gate`, `qa`, the `code` graph, and more. Repo skills in `.lf/skills/` override
and extend them. [Authoring](authoring.md) starts with a working skill and then
adds graph and goal structure only where it earns its place.

## Context

The provider loads `AGENTS.md` natively; Loopflow does not inject it again.
Every skill also sees `LOOPFLOW.md`, `scratch/`, and `.lf/wave/`.
Pull in more context explicitly:

```bash
lf gate --docs VISUAL_DESIGN.md        # one doc
lf gate --docs 'docs/*.md'             # a glob
lf gate --diff files                   # bodies of files changed on the branch
lf debug -c                            # the clipboard
lf token-compress --docs RELEASE_NOTES.md: fit this history into 2,000 tokens
```

Compression preserves decisions and evidence from the whole source.
Do not take the first N commits and call that the history.

## Where files live

```
.lf/                      # Repo config and extensions
  config.yaml             # Model, context defaults
  skills/                 # Skill prompts
  graphs/                  # Graph definitions
  wave/                   # Wave goals and memory (persists)
scratch/                  # working notes; default delivery clears added files
~/.lf/                    # Global config, skills, and the local store
  loopflow.db             # Local identity and execution history
```

`scratch/` dies with the PR; `.lf/wave/` lives forever. Design docs go in
`scratch/`, forward-looking plans in `.lf/wave/`.

## For agents

Every documentation URL serves HTML. The reviewed Markdown source remains
available to agents: append `.md` to the URL (`/docs/waves.md`) or request the
canonical URL with `Accept: text/markdown`. The curated index is
[/llms.txt](/llms.txt); the complete corpus in one file is
[/llms-full.txt](/llms-full.txt). Inside a Loopflow-launched conversation you already
carry the operating contract (`LOOPFLOW.md`) — these pages are the long form.

## Next

[Get Started →](getting-started.md) · [Waves →](waves.md) · [The Agent API →](agent-api.md) · [Conducting →](conducting.md)

## Reference

[Glossary](glossary.md) · [`lf` commands](lf.md) · [Authoring](authoring.md) · [Configuration](config.md) · [Subscriptions](subscriptions.md) · [Security](security.md) · [Troubleshooting](troubleshooting.md)
