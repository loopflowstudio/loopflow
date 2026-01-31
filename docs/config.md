---
layout: default
title: Configuration
---

# Configuration

Configure loopflow via CLI flags, global config (`~/.lf/config.yaml`), or repo config (`.lf/config.yaml`). Precedence: CLI flags > repo config > global config.

## Quick Reference

| Behavior | CLI Flag | Config |
|----------|----------|--------|
| Model | `-m claude:opus` | `agent_model: claude:opus` |
| Interactive mode | `-i` | frontmatter: `interactive: true` |
| Include lf docs | `--lfdocs` (default) | `lfdocs: true` |
| Include branch files | `--diff-files` (default) | `diff_files: true` |
| Include raw diff | `--diff` | `diff: true` |
| Include clipboard | `-c, --clipboard` | — |
| Area scope | `--area PATH` | `area: PATH` |
| Context files | — | `context: [FILE]` |
| Direction (judgment/intent) | `--direction NAME` | `direction: NAME` |
| Chrome automation | `--chrome` | `chrome: true` |
| Yolo mode (skip permissions) | — | `yolo: true` |

## Context Assembly

Every step gets context assembled automatically. Run any command to see the breakdown:

```
Tokens: 12,847

files          6,892 █████
  STYLE.md     2,854 ██
  README.md      988 █
  scratch/     3,050 ██
diff_files     4,721 ███
  src/auth.py  2,847 ██
  tests/       1,874 █
clipboard      1,234 █
```

The token breakdown shows what's included:

| Section | What it contains | Config |
|---------|------------------|--------|
| **files** | `roadmap/`, `scratch/`, root `.md` files | `lfdocs: true` (default) |
| **diff_files** | Files changed on this branch | `diff_files: true` (default) |
| **summary** | Token-limited codebase overviews | `summaries:` in config |
| **clipboard** | Pasted content (errors, context) | `-c` flag |

Defaults work well for most repos. Summaries require configuration.

## Config Files

**Global config** (`~/.lf/config.yaml`) sets user-wide defaults. **Repo config** (`.lf/config.yaml`) overrides for that repo.

For most settings, repo overrides global. For additive settings (`context`, `exclude`, `skill_sources`, `summaries`), lists combine from both.

```yaml
# ~/.lf/config.yaml (global)
agent_model: claude:opus
direction: product-engineer

# .lf/config.yaml (repo)
agent_model: codex        # overrides global
context:
  - docs/api.md           # combined with global context
```

Example repo config:

```yaml
agent_model: claude:opus
push: true

direction: product-engineer

context:
  - src/schema.py
  - docs/api.md

exclude:
  - "*.test.ts"
  - node_modules

branch_names:
  schema: "{user}.{name}.{date}_{ts}"
```

## Flows

Flows are stored as Python files in `.lf/flows/`:

```python
def flow():
    return Flow(["implement", "rebase", "polish", "draft_commit"])
```

---

## Options Reference

### LF Docs (files)

`roadmap/`, `scratch/`, and root markdown files.

| | |
|---|---|
| **CLI** | `--lfdocs` / `--no-lfdocs` |
| **Config** | `lfdocs: true` |
| **Default** | `true` (included) |

Includes README.md, STYLE.md, and similar guidance files. Also auto-includes `scratch/` (current PR scratchpad) and `roadmap/` (internal docs).

### Branch Files (diff_files)

Full content of files modified on the current branch.

| | |
|---|---|
| **CLI** | `--diff-files` / `--no-diff-files` |
| **Config** | `diff_files: true` |
| **Default** | `true` (included) |

This is how the coding agent sees your changes. It gets complete files, not just diffs.

### Summaries

Token-limited overviews of large directories. Configure paths and budget:

```yaml
summary_tokens: 25000

summaries:
  - path: src
  - path: lib
    tokens: 5000
```

Generate with `lf ops summarize`. Summaries give the coding agent codebase context without consuming the full token budget. Only summarize directories you're *not* including directly.

| | |
|---|---|
| **CLI** | `--no-summaries` |
| **Default** | included if configured |

### Clipboard

Paste content (errors, stack traces, context) into the prompt.

| | |
|---|---|
| **CLI** | `-c, --clipboard` |
| **Default** | not included |

Use `-c` when debugging: copy an error, then `lf debug -c`.

### Raw Diff

Include `git diff main...HEAD` output showing exact line changes.

| | |
|---|---|
| **CLI** | `--diff` / `--no-diff` |
| **Config** | `diff: true` |
| **Default** | `false` (not included) |

Use when you want the agent to see precisely what changed. Can combine with `--diff-files`.

### Context Files

Additional files always included in every step.

| | |
|---|---|
| **Config** | `context: [src/schema.py, docs/api.md]` |

Config sets baseline files for all steps.

### Exclude Patterns

Glob patterns to exclude from file listings.

| | |
|---|---|
| **Config** | `exclude: ["*.test.ts", node_modules, dist]` |

---

### Model

Default model for all steps.

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
| **Default** | auto for all steps |

Set a step's default mode in its frontmatter:

```yaml
---
interactive: true
---
# Your step prompt here
```

CLI flags override the frontmatter default.

### Direction

Directions shape judgment and intent—how the coding agent approaches work.

| | |
|---|---|
| **CLI** | `--direction designer` or `--direction product-engineer,designer` |
| **Config** | `direction: product-engineer` or `direction: [product-engineer, designer]` |

Direction files live in `.lf/directions/` as markdown. Built-in directions: `adapt`, `roadmap`, `ship`, `product-engineer`, `designer`, `infra-engineer`, `ceo`.

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

Skip permissions. Codex also disables sandboxing.

| | |
|---|---|
| **Config** | `yolo: true` |
| **Default** | `false` |

Claude: `--dangerously-skip-permissions`. Codex: `--dangerously-bypass-approvals-and-sandbox`. Gemini: `--yolo`.

### Autoprune

Automatically remove worktrees when their PRs merge. Requires `lfd` daemon running.

| | |
|---|---|
| **Config** | `autoprune: true` |
| **Default** | `false` |

With options:

```yaml
autoprune:
  enabled: true
  poll_interval_seconds: 60  # default
```

### IDE

Configure which tools open when launching sessions.

```yaml
ide:
  warp: true           # open Warp terminal
  cursor: true         # open Cursor editor
  workspace: app.code-workspace  # optional workspace file
```

### Branch Names

Customize branch naming for `lf ops wt create`.

```yaml
branch_names:
  schema: "{user}.{name}.{date}_{ts}"
```

Available placeholders:
- `{name}` — short name passed to `lf ops wt create`
- `{user}` — git `user.name` (sanitized)
- `{date}` — `YYYYMMDD`
- `{ts}` — `YYYYMMDD_HHMM`

### Summaries

Pre-generated codebase overviews for large repos.

```yaml
summary_tokens: 25000

summaries:
  - path: src
  - path: lib
    tokens: 5000
```

Generate with `lf ops summarize`.

### SkillRegistry

Enable the SkillRegistry directory as a remote skill source.

```yaml
skill_registry:
  enabled: true
  prefix: sr
  cache_ttl_seconds: 86400
```

Run registry skills by prefix:

```bash
lf sr:gog
```

Registry skills are cached under `~/.lf/skills/skillregistry/` with a `registry.json` index.

### Skill Sources

External skill libraries that extend loopflow with additional steps.

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
