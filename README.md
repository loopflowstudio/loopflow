# Loopflow

Arrange agents to code in harmony.

![Demo](docs/demo.gif)

```bash
# Copy an error to clipboard, then:
lf debug -v
```

Loopflow assembles context and prompts for AI coding agents. Tasks are markdown files—versioned, reusable, shareable.

## Install

```bash
pip install loopflow
lfops install
```

## Quick Start

```bash
wt switch --create my-feature
lf design: add user authentication
lf implement && lf polish && lf review
lfops pr
```

Or run agents in the background while you sleep:

```bash
lfd loop find-pmf
```

## Why Loopflow

**Prompts as artifacts**

```markdown
# .claude/commands/review.md
Review the diff on this branch. Fix any issues.
```

When something works, you can find it again.

**Context management**

```bash
lf review -c    # see what the agent sees
```

**Multi-model**

```bash
lf review -m codex    # same prompt, different backend
```

**Background agents**

```bash
lfd loop find-pmf    # define goals, review PRs when you wake
```

## Documentation

[Read the docs →](docs/index.md)

## Integrations

**Coding Agents**
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) — Anthropic's coding agent (default)
- [Codex CLI](https://github.com/openai/codex) — OpenAI's coding agent
- [Gemini CLI](https://github.com/google-gemini/gemini-cli) — Google's coding agent
- [RAMS](https://github.com/loopflowstudio/rams) — design review agent (global tasks via `~/.claude/commands/`)

**IDE & Terminals**
- [Cursor](https://cursor.sh) — AI-powered code editor
- [Warp](https://www.warp.dev) — modern terminal

**Worktrees & Skills**
- [worktrunk](https://github.com/loopflowstudio/worktrunk) — git worktree management (`wt` commands)
- [superpowers](https://github.com/obra/superpowers) — skill library (run via `lf sp:<skill>`)

## Requirements

macOS.

## License

MIT
