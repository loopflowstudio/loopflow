# lf init: Interactive Setup Prompt

## What to build

An interactive prompt (`lf init`) that guides users through installing loopflow dependencies and configuring their repo, replacing `lfops init` and `lfops install` as the primary setup experience.

## Bootstrap sequence

```bash
# 1. Global install (prerequisite - user does this first)
uv tool install loopflow

# 2. Repo setup (what this prompt does)
cd my-repo
lf init
```

The prompt assumes loopflow is already installed globally via `uv tool install`. It handles everything *after* that: installing Claude Code, worktrunk, configuring the repo, etc.

## User's words

> "an interactive experience for installing the dependencies and optional parts of loopflow"
> "instruct the agent on what to install and what options to offer the user for choosing TUI / install extras, leveraging skill providers etc"

## Data structures

No new Python types. This is a prompt file that instructs the agent to:
1. Run shell commands to check/install dependencies
2. Ask the user questions about preferences
3. Write config files

The prompt lives in:

```
src/loopflow/lf/builtins/init.md
```

## Prompt structure

```markdown
---
interactive: true
---
Guide the user through setting up loopflow in this repository.

## Phase 1: Check environment

Run these checks and report status:

1. `which claude` — Claude Code installed?
2. `which wt` — worktrunk installed?
3. `which npm` — Node.js available?
4. `which brew` — Homebrew available (needed for installs)?
5. Check if `.lf/config.yaml` exists
6. Check if `~/.superpowers` exists

Print a summary like:
```
Checking environment...
✓ Node.js
✓ Claude Code
✗ worktrunk (required)
- superpowers (optional)

Config: not initialized
```

## Phase 2: Install missing required dependencies

If Claude Code or worktrunk missing, offer to install:

"worktrunk is required for worktree management. Install it?"
- Yes → run `brew install max-sixty/worktrunk/wt`
- No → explain they can run `lfops install` later

"Claude Code is required. Install it?"
- Yes → run `npm install -g @anthropic-ai/claude-code`
- No → exit with instructions

## Phase 3: Configure repository

If no `.lf/config.yaml`:

"Initialize loopflow in this repo?"
- Yes → create `.lf/config.yaml` from template
- No → skip

## Phase 4: Optional extras

Present choices:

**Coding agents:**
"Which coding agents do you want to use?"
- Claude Code (default, already installed)
- Codex CLI (`npm install -g @openai/codex`)
- Gemini CLI (`npm install -g @google/gemini-cli`)

**Skill libraries:**
"Install superpowers skill library? (adds `lf sp:` commands)"
- Yes → `git clone https://github.com/obra/superpowers ~/.superpowers`
- No → skip

**IDE integration:**
"Which tools do you use?"
- Warp terminal → configure `ide.warp: true`
- Cursor → configure `ide.cursor: true`
- Other → skip

## Phase 5: Summary

Print what was done:

```
✓ worktrunk installed
✓ Claude Code installed
✓ .lf/config.yaml created
✓ superpowers installed

Ready! Try: lf debug -v
```
```

## Key functions

None new. The prompt instructs the agent to use existing shell commands:

```bash
# Dependency checks
which claude && echo "installed" || echo "missing"
brew install max-sixty/worktrunk/wt
npm install -g @anthropic-ai/claude-code

# Config creation
mkdir -p .lf
cat > .lf/config.yaml << 'EOF'
...
EOF
```

## Changes to existing code

1. **Add builtin prompt:** `src/loopflow/lf/builtins/init.md`

2. **Deprecate or remove:**
   - `lfops init` — replaced by `lf init`
   - `lfops install` — replaced by `lf init`
   - Keep `lfops doctor` for quick non-interactive checks

3. **Update docs:** Point users to `lf init` instead of `lfops install && lfops init`

## Constraints

- **macOS only:** Homebrew-based installs assume macOS. The prompt should check `uname` and bail early on other platforms.
- **Interactive required:** This must run with `-i` flag. Auto mode doesn't make sense for a setup wizard.
- **No state:** Each run re-checks everything. No tracking of "already ran init."

## UI changes

None. This is CLI-only. Maestro could eventually have an onboarding flow but that's separate.

## Done when

```bash
# Fresh machine simulation
lf init
# Agent guides through setup, installs deps, creates config

# Verify
which wt && which claude && test -f .lf/config.yaml && echo "success"
```

## Open questions

1. Should `lf init` auto-detect if everything is already set up and skip to "you're ready"?
2. Should we keep `lfops install` as a non-interactive escape hatch, or fully deprecate?
3. What about users who want to install deps but not initialize a specific repo? (global setup vs repo setup)
