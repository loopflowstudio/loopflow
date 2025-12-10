# Loopflow

Arrange LLMs to code in harmony.

## Installation

```bash
pip install loopflow
lf install  # installs Claude Code via npm
```

Requires Node.js for the `lf install` step. Alternatively, install [Claude Code](https://docs.anthropic.com/en/docs/claude-code) manually.

## Usage

```bash
lf <task> [arg] [-c context...]
```

### Examples

```bash
# Run a task
lf review
lf commit

# Task with input file
lf implement design.md

# Add context files
lf implement design.md -c src/api.py -c src/models.py

# Print mode: run non-interactively
lf review -p

# Check dependencies
lf doctor
```

## Project Structure

Loopflow reads from your repo:

```
.lf/
├── review.lf           # Task definitions
├── implement.lf
└── commit.lf

VOICE.md                # How Claude should think and work
STYLE.md                # Code style guide
README.md               # Project documentation
```

### Tasks

Task files define what Claude should do. Place them in `.lf/` with `.lf`, `.md`, or `.txt` extension.

### Task Arguments vs Context

- **Argument** (`lf implement design.md`): The primary input to the task. Appears prominently in the prompt as "Task input."
- **Context** (`-c file.py`): Supporting files. Appears as "Reference files" with parent documentation.

### VOICE.md

Defines Claude's voice—how it approaches problems, balances creativity with pragmatism, and communicates. This applies to all tasks.

## Context Gathering

Loopflow automatically gathers context for Claude:

1. **Repository docs** - All `.md` files at the repo root (README, STYLE, VOICE, etc.)
2. **Task argument** - The input file for the task (if provided)
3. **Task definition** - From `.lf/<task>.lf`
4. **Context files** - Any files specified with `-c`, plus their parent `.md` documentation

Output uses `<lf:tag>` delimiters for unambiguous parsing.
