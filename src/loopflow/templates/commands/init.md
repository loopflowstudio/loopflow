---
interactive: true
produces: installed dependencies, .lf/config.yaml
---
Guide the user through setting up loopflow in this repository.

## Prerequisites

This prompt assumes loopflow is already installed globally via `uv tool install loopflow`. It handles everything after that: installing Claude Code, worktrunk, configuring the repo, and optional extras.

## Phase 1: Check environment

First, verify this is macOS:
```bash
uname -s
```

If not Darwin, stop and explain: "Loopflow requires macOS. The Homebrew-based installation won't work on other platforms."

Run these checks and report status:
```bash
which brew    # Homebrew (needed for installs)
which npm     # Node.js (needed for Claude Code)
which claude  # Claude Code
which wt      # worktrunk
test -f .lf/config.yaml && echo "config exists"
test -d ~/.superpowers && echo "superpowers exists"
```

Print a summary:
```
Checking environment...
✓ Homebrew
✓ Node.js
✓ Claude Code
✗ worktrunk (required)
- superpowers (optional)

Config: not initialized
```

If everything required is installed and config exists, say "You're all set! Try: lf debug -v" and offer to install optional extras.

## Phase 2: Install missing required dependencies

If Homebrew is missing, stop: "Install Homebrew first: https://brew.sh"

If Node.js is missing:
- Ask: "Node.js is required for Claude Code. Install via Homebrew?"
- Yes: `brew install node`
- No: explain they need to install it manually

If Claude Code is missing:
- Ask: "Claude Code is required. Install it?"
- Yes: `npm install -g @anthropic-ai/claude-code`
- No: explain they can install it manually later with that command

If worktrunk is missing:
- Ask: "worktrunk is required for worktree management. Install it?"
- Yes: `brew install max-sixty/worktrunk/wt`
- No: explain they can install it manually later with that command

## Phase 3: Configure repository

Check if we're in a git repo:
```bash
git rev-parse --show-toplevel
```

If not a git repo, explain: "Run this from inside a git repository."

If no `.lf/config.yaml`:
- Ask: "Initialize loopflow in this repo? This creates .lf/config.yaml"
- Yes: create the config file (see template below)
- No: skip

Config template:
```yaml
# Loopflow configuration

# Model to use (backend or backend:variant)
agent_model: claude:opus

# Context: files/directories to include by default
context: "."

# Exclude: patterns to ignore
exclude:
  - "*.lock"
  - node_modules
  - .venv

# Skip permissions (only use in trusted repos)
yolo: false

# Push commits to origin automatically
push: false
```

## Phase 4: Optional extras

Ask about each category. Let the user pick multiple.

**Additional coding agents:**
"Want to install other coding agents? (Claude Code is already set up)"
- Codex CLI: `npm install -g @openai/codex`
- Gemini CLI: `npm install -g @google/gemini-cli`
- Neither (skip)

**Skill libraries:**
Check if ~/.superpowers exists. If not:
"Install superpowers skill library? Adds community prompts via `lf sp:` commands"
- Yes: `git clone https://github.com/obra/superpowers ~/.superpowers`
- No: skip

**IDE preferences:**
"Which tools do you use? (Select all that apply)"
- Warp terminal
- Cursor editor
- Other/skip

If they select Warp or Cursor, note this for the summary but don't modify config (IDE config is optional).

## Phase 5: Summary

Print what was done:
```
Setup complete!

✓ Node.js (already installed)
✓ Claude Code installed
✓ worktrunk installed
✓ .lf/config.yaml created
✓ superpowers installed

Ready! Try these commands:
  lf debug -v     # paste an error, watch it fix
  lf review       # review code on this branch
  lf --list       # see all available tasks
```

## Conversation style

- Short, clear prompts
- One question at a time
- Default to "yes" for required dependencies
- Let users skip optional things without judgment
- If something fails, explain what went wrong and how to retry manually
