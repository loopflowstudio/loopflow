# Loopflow

Arrange LLMs to code in harmony.

**macOS only** (for now)

## Installation

```bash
pip install loopflow
lf meta install  # installs Node.js (via Homebrew) and Claude Code
```

## How It Works

Loopflow builds prompts for Claude from two sources:

1. **Repository docs** (`.md` files at repo root) - guidance that applies to all tasks
2. **Task definitions** (`.lf/` directory) - specific instructions for each task

### Step 1: Write Documentation for Claude

Put `.md` files at your repo root. Claude reads all of them before every task. Use these to communicate:

- What the project does and how it's structured
- Code style, conventions, and patterns to follow
- How you want Claude to think and communicate
- Development workflow and practices

Write them for Claude, not just humans. For example:

```markdown
# STYLE.md

Use descriptive variable names. Prefer early returns over nested conditionals.
Keep functions under 30 lines. Don't add docstrings to obvious functions.
```

Common files: `README.md`, `STYLE.md`, `VOICE.md`, `CONTRIBUTING.md` - but name them whatever makes sense.

### Step 2: Define Tasks

Create task files in `.lf/`:

```
.lf/
├── review.lf
├── implement.lf
└── commit.lf
```

Each file contains instructions for that task. For example:

```markdown
# .lf/review.lf

Review the code for bugs, style issues, and potential improvements.
Be direct. If something is wrong, say so.
```

### Step 3: Run Tasks

```bash
lf review                    # Run the review task
lf implement                 # Run the implement task
lf my-custom-task            # Run any task you've defined
```

Tasks can take an argument (a primary input file):

```bash
lf implement dd/auth.md      # explicit design doc path
lf review src/api.py         # review this specific file
```

Or use the branch-based workflow (recommended):

```bash
lf wt create auth-feature    # create worktree at .lf/worktrees/auth-feature/
cd .lf/worktrees/auth-feature
lf design                    # writes to auth-feature.md at repo root
lf implement                 # design doc auto-included in prompt
```

Add context files with `-c`:

```bash
lf implement -c src/models.py -c src/api.py
```

Or run an inline prompt without a task file:

```bash
lf : "fix the typo in README"           # Quick inline prompt
lf : "add tests for the parser" -c src/parser.py
```

## Commands

### Tasks

Run tasks from `.lf/` by name:

```bash
lf review        # → .lf/review.lf
lf implement     # → .lf/implement.lf
lf whatever      # → .lf/whatever.lf
```

### Worktrees (`lf wt`)

| Command | Description |
|---------|-------------|
| `lf wt create <name>` | Create worktree and branch, open IDEs |
| `lf wt open <name>` | Open IDEs at existing worktree |
| `lf wt list` | List worktrees with status |
| `lf wt clean` | Remove worktrees for branches no longer on origin |

### Pull Requests (`lf pr`)

| Command | Description |
|---------|-------------|
| `lf pr create` | Create a GitHub PR for current branch |
| `lf pr land [-m msg]` | Squash-merge current branch to main |

### Setup (`lf meta`)

| Command | Description |
|---------|-------------|
| `lf meta install` | Install dependencies based on config (macOS) |
| `lf meta doctor` | Check dependencies based on config |
| `lf meta version` | Show version |

### Inline Prompts

```bash
lf : "fix the typo in README"           # Quick inline prompt
lf : "add tests for the parser" -c src/parser.py
```

## Options

| Option | Description |
|--------|-------------|
| `-p, --print` | Run non-interactively (batch mode) |
| `-c, --context FILE` | Add context files (repeatable) |
| `-b, --branch NAME` | Create worktree and run task there |

**Note:** Batch mode (`-p`) automatically runs with `--dangerously-skip-permissions` since there's no way to approve permissions interactively.

## Pipelines

Chain tasks into named sequences. Define them in `.lf/config.yaml`:

```yaml
pipelines:
  ship:
    - implement
    - review
    - rebase
    - test
    - draft_commit
```

Run with:

```bash
lf ship             # runs implement -> review -> rebase -> test -> draft_commit
```

- Design doc (`<branch>.md`) is auto-included in prompt
- Each task runs in batch mode with streaming output
- Each task commits its changes before the next task starts
- macOS notification when pipeline finishes

### Auto-Push and PR Creation

Enable automatic push and PR creation per-pipeline:

```yaml
push: true          # auto-push when upstream exists
pr: false           # don't open PRs by default

pipelines:
  ship:
    tasks:
      - implement
      - review
      - draft_commit
    pr: true        # this pipeline opens a PR
```

Or use flags:

```bash
lf ship --pr                 # run pipeline and open PR
lf wt create feature-x && cd .lf/worktrees/feature-x && lf ship
```

Behavior:
- `push: true` - auto-push after each commit if branch tracks a remote
- `pr: true` - open draft PR when pipeline completes (implies push)
- Pipeline settings override global config
- Use `lf wt create` to create a worktree before running
- PR uses `gh` CLI (`--fill --draft`)

## Configuration

Create `.lf/config.yaml` for repo-wide settings:

```yaml
# Skip permission prompts in interactive mode (default: false)
dangerously_skip_permissions: true

# Auto-push/PR defaults (default: false)
push: true
pr: false

# IDE settings for lf wt create/open (all default: true)
ide:
  warp: true
  cursor: true
  workspace: myproject.code-workspace  # optional: explicit workspace file

pipelines:
  ship:
    tasks:
      - implement
      - review
      - rebase
      - test
      - draft_commit
    pr: true  # override: this pipeline opens PRs
```

## Prompt Structure

Loopflow assembles prompts with `<lf:tag>` delimiters:

```
<lf:docs>
<lf:README>...</lf:README>
<lf:STYLE>...</lf:STYLE>
</lf:docs>

<lf:task:review>...</lf:task:review>

<lf:input path="design.md">...</lf:input>

<lf:files>
<lf:file path="src/api.py">...</lf:file>
</lf:files>
```
