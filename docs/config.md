---
layout: default
title: Configuration
---

# Configuration

Configure loopflow via CLI flags, global config (`~/.lf/config.yaml`), or repo config (`.lf/config.yaml`). Precedence: CLI flags > repo config > global config.

## Quick Reference

| Behavior | CLI Flag | Config |
|----------|----------|--------|
| Model | `-m claude:opus` | `agent: claude:opus` |
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
| Interactive launch surface | `--web` for headless steps | `session.launch: cli` |

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
| **files** | `wave/`, `scratch/`, root `.md` files | `lfdocs: true` (default) |
| **diff_files** | Files changed on this branch | `diff_files: true` (default) |
| **summary** | Token-limited codebase overviews | `summaries:` in config |
| **clipboard** | Pasted content (errors, context) | `-c` flag |

Defaults work well for most repos. Summaries require configuration.

## Config Files

**Global config** (`~/.lf/config.yaml`) sets user-wide defaults. **Repo config** (`.lf/config.yaml`) overrides for that repo.

For most settings, repo overrides global. For additive settings (`context`, `exclude`, `summaries`, `supported_harnesses`), lists combine from both.

```yaml
# ~/.lf/config.yaml (global)
agent: claude:opus
direction: clarity

# .lf/config.yaml (repo)
agent: codex        # overrides global
context:
  - docs/api.md           # combined with global context
```

Example repo config:

```yaml
agent: claude:opus

session:
  launch: cli

direction: clarity

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

Flows are YAML files in `.lf/flows/`:

```yaml
# .lf/flows/ship-api.yaml
- implement
- compress
- gate
```

---

## Options Reference

### LF Docs (files)

`wave/`, `scratch/`, and root markdown files.

| | |
|---|---|
| **CLI** | `--lfdocs` / `--no-lfdocs` |
| **Config** | `lfdocs: true` |
| **Default** | `true` (included) |

Includes README.md, STYLE.md, and similar guidance files. Also auto-includes `scratch/` (current PR scratchpad) and `wave/` (wave plans).

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

Summaries are daemon-managed context (`lfd`) for wave runs, not direct `lf` flags.

| | |
|---|---|
| **CLI (`lf`)** | none |
| **Daemon (`lfd`)** | auto-refreshed for waves |

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
| **CLI** | `lf gate -m codex:o3` |
| **Config** | `agent: claude:opus` (optional) |
| **Default** | unset (resolution falls back to step defaults, then `claude:opus`) |

Harnesses: `claude`, `codex`, `gemini`, `opencode`. Use `harness:model` for specific models.

Gemini is supported for direct `lf` commands. Session-based features (waves, `lfd`) require `claude`, `codex`, or `opencode`.

OpenCode passes model strings through to its own provider system:

```yaml
agent: opencode                          # use opencode's default model
agent: opencode:anthropic/claude-sonnet  # explicit model
agent: opencode:openai/gpt-4o            # any provider opencode supports
```

### Supported Harnesses

Optional list of harnesses exposed in Concerto's model picker and settings.

```yaml
supported_harnesses:
  - claude
  - codex
  - gemini
```

This list is additive across global and repo config.

### Run Mode

Batch mode runs to completion. Interactive mode allows interruption and chat.

| | |
|---|---|
| **CLI** | `-i` (interactive), `-b` (batch/headless) |
| **Default** | batch for all steps |

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
| **CLI** | `--direction ux` or `--direction ux,clarity` |
| **Config** | `direction: clarity` or `direction: [ux, clarity]` |

Direction files live in `.lf/directions/` as markdown. Built-in direction groups:
`infra`, `ux`, `craft`, `creativity`, `ceo`. Group members are also
available directly (for example `security`, `feedback`, `clarity`, `alive`).

### Chrome

Enable browser automation for Claude Code.

| | |
|---|---|
| **CLI** | `--chrome` / `--no-chrome` |
| **Config** | `chrome: true` |
| **Default** | `false` |

Requires the [Chrome extension](https://chromewebstore.google.com/detail/claude-browser-tool/gfbkicmkbhdjacjmfjffcldkdopkfjgk) and a paid Claude plan.

### Yolo

Skip permissions. Codex uses the danger-full-access sandbox.

| | |
|---|---|
| **Config** | `yolo: true` |
| **Default** | `false` |

Claude: `--dangerously-skip-permissions`. Codex: `--sandbox danger-full-access` with `approval_policy="never"`. Gemini: `--yolo`. OpenCode: `permission: "allow"` via `OPENCODE_CONFIG_CONTENT`.

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

### Session Launch

Pick where interactive handoffs open.

```yaml
session:
  launch: cli          # cli | ide
```

`cli` opens the vendor CLI/TUI in the current terminal. `ide` opens the Codex or
Claude app by URL scheme and falls back to `cli` if no app handles the link.
OpenCode is CLI-only.

### Branch Names

Customize branch naming for `lf op wt create`.

```yaml
branch_names:
  schema: "{user}.{name}.{date}_{ts}"
```

Available placeholders:
- `{name}` — short name passed to `lf op wt create`
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

### External Skills

Loopflow has two primary skill channels plus one compatibility shim. No config needed.

- **`gstack/<step>`** — bundled in the binary. Upstream is [garrytan/gstack](https://github.com/garrytan/gstack). Maintainers run the `refresh-gstack` step inside the loopflow repo to resync the bundled catalog; users just get the version their `lf` was built with.
- **`npx/<owner>/<repo>`** — fetched live via [`npx skills`](https://www.npmjs.com/package/skills) and cached under `.agents/skills/`. If the skill is already cached — or `npx skills find` can resolve it — `npx/<name>` often works too. This is the general escape hatch for third-party Claude Skill packages.
- **`rams/rams`** — legacy single-file compatibility shim. It resolves only when `~/.claude/commands/rams.md` exists.

```bash
lf gstack/office-hours                # bundled
lf npx/vercel-labs/deep-research      # live fetch, cached on first run
lf rams/rams                          # legacy compatibility alias, if installed
```

The older `skill_sources` config block and `~/.superpowers` auto-detection have been removed. If you were pointing at a local directory of skill prompts, place the files under `.lf/steps/<namespace>/<step>.md` (repo-local) or `~/.lf/steps/<namespace>/<step>.md` (user-global) and invoke them as `lf <namespace>/<step>`. Namespaced steps use `/`, not `:`.

### RLM (Recursive Language Model)

Configure recursive sub-agent behavior for processing large inputs.

```yaml
rlm_agent: claude:sonnet    # model for sub-agents (default: same as agent)
rlm_max_parallel: 10        # max concurrent sub-agents
rlm_max_depth: 3            # max recursion depth
```

| Field | Default | Description |
|-------|---------|-------------|
| `rlm_agent` | (agent) | Model for RLM sub-agents |
| `rlm_max_parallel` | 10 | Suggested max concurrent sub-agents |
| `rlm_max_depth` | 3 | Maximum recursion depth |

These values are propagated as environment variables (`RLM_MODEL`, `RLM_MAX_PARALLEL`, `RLM_MAX_DEPTH`) to sub-agents. See the bundled RLM instructions for usage patterns.
