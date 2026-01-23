# Loopflow

Arrange and conduct an agent orchestra.

## The Model

| Primitive | What it does |
|-----------|--------------|
| **Step** | Runs a prompt with assembled context |
| **Flow** | Chains steps together |
| **Voice** | Shapes judgment and perspective |
| **Area** | Focuses on part of the codebase |

| Agent Mode | Runs when |
|------------|-----------|
| **Loop** | Continuously until stopped |
| **Watch** | Paths change on main |
| **Cron** | On schedule |

An agent is **flow × area × voice**.

## Steps

```bash
lf debug -c
```

Assembles context (docs, style guides, branch diff) and runs the prompt. `-c` adds your clipboard.

```bash
lf review                 # run review.md
lf implement: add auth    # pass arguments
lf debug -i               # interactive (you guide)
lf polish -a              # autonomous (runs to completion)
```

Steps live in `.lf/steps/`.

## Flows

```bash
lf flow ship              # design → implement → polish
```

Or chain manually:

```bash
lf design: add user auth && lf implement && lf polish && lfops pr
```

Define flows in `.lf/flows/ship.py`:

```python
def flow():
    return Flow("design", "implement", "polish")
```

## Agents

```bash
lfd loop ship src/
```

Runs the `ship` flow on `src/` continuously, creating PRs until stopped.

```bash
lfd loop ship src/ -v architect    # add a voice
lfd watch ship docs/ -v writer     # run when docs/ changes
lfd status                         # see all agents
```

## Install

```bash
uv tool install loopflow
```

Built-in steps and flows included. `lf init` sets up Claude Code and preferences.

[Documentation →](docs/index.md)

## Integrations

**Coding Agents**
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) — Anthropic's coding agent (default)
- [Codex CLI](https://github.com/openai/codex) — OpenAI's coding agent
- [Gemini CLI](https://github.com/google-gemini/gemini-cli) — Google's coding agent

**Tools**
- [worktrunk](https://github.com/loopflowstudio/worktrunk) — git worktree management (`wt` commands)

**Skill Libraries**
- [superpowers](https://github.com/obra/superpowers) — prompt library (`lf sp:<skill>`)
- [SkillRegistry](https://skillregistry.io/) — remote skill directory (`lf sr:<skill>`)
- [rams](https://rams.ai) — accessibility and visual design review

## Requirements

- macOS
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://github.com/openai/codex), or [Gemini CLI](https://github.com/google-gemini/gemini-cli)

## License

MIT
