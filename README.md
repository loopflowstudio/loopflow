# Loopflow

Arrange LLMs to code in harmony.

**macOS only** (for now)

## Installation

```bash
pip install loopflow
lf install  # installs Node.js (via Homebrew) and Claude Code
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
lf implement design.md       # design.md becomes the "task input"
lf review src/api.py         # review this specific file
```

Add context files with `-c`:

```bash
lf implement design.md -c src/models.py -c src/api.py
```

Create and track a new branch with `-b`:

```bash
lf implement -b feature-name design.md
```

## Commands

**Built-in commands:**

| Command | Description |
|---------|-------------|
| `lf install` | Install Node.js and Claude Code (macOS) |
| `lf doctor` | Check dependencies |
| `lf version` | Show version |
| `lf land [-m msg]` | Squash-merge current branch to main |

**Task commands** - anything else runs a task from `.lf/`:

```bash
lf review        # → .lf/review.lf
lf implement     # → .lf/implement.lf
lf whatever      # → .lf/whatever.lf
```

## Options

| Option | Description |
|--------|-------------|
| `-p, --print` | Run non-interactively (batch mode) |
| `-c, --context FILE` | Add context files (repeatable) |
| `-b, --branch NAME` | Create and track new branch |

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
lf ship design.md   # runs implement -> review -> rebase -> test -> draft_commit
```

- First task gets the argument (`design.md`)
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
lf ship --pr design.md       # run pipeline and open PR
lf ship -b feature-x design.md  # create branch, run pipeline
```

Behavior:
- `push: true` - auto-push after each commit if branch tracks a remote
- `pr: true` - open draft PR when pipeline completes (implies push)
- Pipeline settings override global config
- Use `-b` to create and track a new branch before running
- PR uses `gh` CLI (`--fill --draft`)

## Configuration

Create `.lf/config.yaml` for repo-wide settings:

```yaml
# Skip permission prompts in interactive mode (default: false)
dangerously_skip_permissions: true

# Auto-push/PR defaults (default: false)
push: true
pr: false

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
