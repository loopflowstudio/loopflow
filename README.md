# Loopflow

```bash
lf review
```

Assembles context and prompts for AI coding agents. Steps are markdown files—versioned, reusable, shareable.

## Step

Copy an error, watch it fix.

```bash
lf debug -v

Tokens: 33,017

files          20,850 ████████████
  docs         17,978 ██████████
  STYLE.md      2,158 █
  README.md       475 ▏
docs           11,004 ██████
loopflow          657 ▏
task              506 ▏
clipboard         342 ▏
```

Loopflow assembles context from your repo—docs, style guides, the current diff—and packages it with a task prompt. The token profile shows what's going in. This context is what makes the agent effective.

Tasks are markdown files in `.claude/commands/` or `.lf/`. Run by name:

```bash
lf review                 # run review.md
lf implement: add auth    # pass arguments
```

## Flow

Design, implement, polish, ship.

```bash
lf design: add user auth
lf implement
lf polish
lfops pr
```

![workflow demo](docs/workflow-demo.gif)

## Loop

Define goals, review PRs when you wake.

```bash
lfd loop product-engineer
```

![loops demo](docs/loops-demo.gif)

## Install

```bash
uv tool install loopflow
```

Built-in tasks included. Works immediately with Claude Code.

Run `lf init` to install Claude Code, worktrunk, and configure preferences.

## Documentation

[Read the docs →](docs/index.md)

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
