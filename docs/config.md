---
layout: default
title: Configuration
---

# Configuration

Configure loopflow via CLI flags or `.lf/config.yaml`. CLI flags override config for that run.

## Quick Reference

| Behavior | CLI Flag | Config |
|----------|----------|--------|
| Model | `-m claude:opus` | `agent_model: claude:opus` |
| Interactive mode | `-i` | `interactive: [design]` |
| Include docs | `--docs` (default) | `docs: true` |
| Include branch files | `--diff-files` (default) | `diff_files: true` |
| Include raw diff | `--diff` | `diff: true` |
| Include clipboard | `-v, --paste` | — |
| Add context files | `-x FILE` | `context: [FILE]` |
| Voice/persona | `--voice NAME` | `voice: NAME` |
| Chrome automation | `--chrome` | `chrome: true` |
| Skip permissions | — | `yolo: true` |

## Defaults

Out of the box, every task includes:

| Context | Default | How to change |
|---------|---------|---------------|
| Repo docs (README, STYLE, etc.) | ✓ included | `--no-docs` or `docs: false` |
| Files changed on branch | ✓ included | `--no-diff-files` or `diff_files: false` |
| Raw git diff | not included | `--diff` or `diff: true` |
| Clipboard | not included | `-v` flag only |
| Summaries | if configured | `--no-summaries` or remove from config |

## Config File

Create `.lf/config.yaml` at your repo root:

```yaml
agent_model: claude:opus
push: true

interactive:
  - design
  - iterate

voice: architect

context:
  - src/schema.py
  - docs/api.md

exclude:
  - "*.test.ts"
  - node_modules
```

---

## Options Reference

### Model

Default model for all tasks.

| | |
|---|---|
| **CLI** | `lf review -m codex:o3` |
| **Config** | `agent_model: claude:opus` |
| **Default** | `claude:opus` |

Backends: `claude`, `codex`, `gemini`. Use `backend:variant` for specific models.

### Run Mode

Auto mode runs to completion. Interactive mode allows interruption and chat.

| | |
|---|---|
| **CLI** | `-i` (interactive), `-a` (auto) |
| **Config** | `interactive: [design, iterate]` |
| **Default** | auto for all tasks |

Tasks listed in `interactive` config default to interactive mode. CLI flags override.

### Branch Files (diff_files)

Include full content of files modified on the current branch.

| | |
|---|---|
| **CLI** | `--diff-files` / `--no-diff-files` |
| **Config** | `diff_files: true` |
| **Default** | `true` (included) |

This is how agents see your changes. They get complete files, not just diffs.

### Raw Diff

Include `git diff main...HEAD` output showing exact line changes.

| | |
|---|---|
| **CLI** | `--diff` / `--no-diff` |
| **Config** | `diff: true` |
| **Default** | `false` (not included) |

Use when you want the agent to see precisely what changed. Can combine with `--diff-files`.

### Docs

Include all `.md` files from repository root.

| | |
|---|---|
| **CLI** | `--docs` / `--no-docs` |
| **Config** | `docs: true` |
| **Default** | `true` (included) |

README, STYLE, and other markdown files provide project context.

### Context Files

Additional files always included in every task.

| | |
|---|---|
| **CLI** | `-x FILE` (repeatable) |
| **Config** | `context: [src/schema.py, docs/api.md]` |

CLI adds to config; config sets baseline for all tasks.

### Exclude Patterns

Glob patterns to exclude from file listings.

| | |
|---|---|
| **Config** | `exclude: ["*.test.ts", node_modules, dist]` |

### Voice

Personas that shape how the agent responds.

| | |
|---|---|
| **CLI** | `--voice concise` or `--voice architect,concise` |
| **Config** | `voice: architect` or `voice: [architect, concise]` |

Voice files live in `.lf/voices/` as markdown.

### Chrome

Enable browser automation for Claude Code.

| | |
|---|---|
| **CLI** | `--chrome` / `--no-chrome` |
| **Config** | `chrome: true` |
| **Default** | `false` |

Requires the [Chrome extension](https://chromewebstore.google.com/detail/claude-browser-tool/gfbkicmkbhdjacjmfjffcldkdopkfjgk) and a paid Claude plan.

### Push

Auto-push after commits in auto mode.

| | |
|---|---|
| **Config** | `push: true` |
| **Default** | `false` |

### Yolo

Skip all permission prompts (Claude Code only).

| | |
|---|---|
| **Config** | `yolo: true` |
| **Default** | `false` |

Passes `--dangerously-skip-permissions`. Use with caution.

### IDE

Configure which tools open when launching sessions.

```yaml
ide:
  warp: true           # open Warp terminal
  cursor: true         # open Cursor editor
  workspace: app.code-workspace  # optional workspace file
```

### Summaries

Pre-generated codebase overviews for large repos.

```yaml
summary_tokens: 25000

summaries:
  - path: src
  - path: lib
    tokens: 5000
```

Generate with `lfops summarize`. Cached in `.lf/summaries/`.

### Skill Sources

External skill libraries that extend loopflow with additional tasks.

```yaml
skill_sources:
  - name: superpowers
    prefix: sp
    path: ~/.superpowers
```

| Field | Description |
|-------|-------------|
| `name` | Display name for the source |
| `prefix` | Prefix for invoking skills (e.g., `sp:brainstorm`) |
| `path` | Path to skill library (supports `~` expansion) |

After configuring, run skills with their prefix:

```bash
lf sp:brainstorm              # run superpowers brainstorm skill
lf sp:write-plan -m codex     # with a different model
```

**Auto-detection:** If `~/.superpowers` exists, it's automatically registered with prefix `sp` even without explicit config.

See [superpowers](https://github.com/obra/superpowers) for an example skill library.
