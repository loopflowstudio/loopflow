# Loopflow

```bash
lf review
```

Assembles context and prompts for AI coding agents. Tasks are markdown files—versioned, reusable, shareable.

## Step

Copy an error, watch it fix.

```bash
lf debug -v
```

![debug demo](docs/debug-demo.gif)

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

Built-in tasks included. No setup required.

## Documentation

[Read the docs →](docs/index.md)

## Integrations

**Coding Agents**
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) — Anthropic's coding agent (default)
- [Codex CLI](https://github.com/openai/codex) — OpenAI's coding agent
- [Gemini CLI](https://github.com/google-gemini/gemini-cli) — Google's coding agent

**Tools**
- [worktrunk](https://github.com/loopflowstudio/worktrunk) — git worktree management (`wt` commands)
- [superpowers](https://github.com/obra/superpowers) — skill library (`lf sp:<skill>`)

## Requirements

- macOS
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://github.com/openai/codex), or [Gemini CLI](https://github.com/google-gemini/gemini-cli)

## License

MIT
