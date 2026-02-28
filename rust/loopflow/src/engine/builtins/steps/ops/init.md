---
interactive: true
produces: .lf/config.yaml
---
Guide the user through setting up loopflow in this repository.

## Phase 1: Environment check

Run these checks:
```bash
git rev-parse --show-toplevel
uname -s
command -v claude
command -v codex
command -v opencode
test -f .lf/config.yaml && echo "config exists"
```

Use `command -v` (POSIX builtin) — not `which`.

Print a summary:
```
Checking environment...
✓ git repo: /path/to/repo
✓ Claude Code
✓ OpenCode
- Codex (not installed)

Platform: Linux
Config: not initialized
```

## Phase 2: Agent guidance

**Multiple agents found:** Report all, ask which to default to.

**One agent found:** Default to it, report the choice.

**None found:** Show install instructions and stop:
```
No coding agent found. Install one:
  Claude Code:  npm install -g @anthropic-ai/claude-code
  OpenCode:     go install github.com/anomalyco/opencode@latest
  Codex CLI:    npm install -g @openai/codex
Then run `lf init` again.
```

Do not run package managers. Init detects — it never installs.

## Phase 3: Create or update config

**No `.lf/config.yaml`:**
- Ask: "Initialize loopflow in this repo? This creates .lf/config.yaml"
- Create the config with detected agent as default

**Existing `.lf/config.yaml`:**
- Compare `supported_harnesses` against detected agents
- If new agents detected that aren't listed, offer to add them
- If current `agent:` value isn't installed, offer to switch the default
- If everything matches, report "Config is up to date" and move on
- Never overwrite custom `exclude` patterns or other user-tuned fields

Config template:
```yaml
# Loopflow configuration
agent: <detected-default>

supported_harnesses:
  - <detected-agents>

context: "."

exclude:
  - "*.lock"
  - node_modules
  - .venv

yolo: false
push: false

skill_registry:
  enabled: false
```

Use the agent name without model suffix: `agent: claude` not `agent: claude:opus`.

Init creates `.lf/config.yaml` and nothing else — no flows README, no IDE config.

## Phase 4: Optional extras

**superpowers:** If `~/.superpowers` doesn't exist, offer:
"Install superpowers skill library? Adds community prompts via `lf sp:` commands"
- Yes: `git clone https://github.com/obra/superpowers ~/.superpowers`
- No: skip

**SkillRegistry:** Offer to enable in config:
"Enable SkillRegistry? Adds remote skills via `lf sr:` commands"
- Yes: set `skill_registry.enabled: true` in `.lf/config.yaml`
- No: skip

## Phase 5: Next steps

Platform-aware guidance:

**macOS:**
```
Setup complete!

✓ Claude Code (default agent)
✓ .lf/config.yaml created

Next:
  lf design              # interactive design session
  lf debug -c            # paste an error, fix it
  lf --list              # see all steps and flows

  Download Concerto for visual wave management
  Run `lfd install` to set up the daemon for autonomous waves
```

**Linux:**
```
Setup complete!

✓ OpenCode (default agent)
✓ .lf/config.yaml created

Next:
  lf design              # interactive design session
  lf debug -c            # paste an error, fix it
  lf --list              # see all steps and flows

  Set up tmux integration: lf ops shell install
  Run `lfd install` to set up the daemon for autonomous waves
```

## Conversation style

- Short, clear prompts
- One question at a time
- Let users skip optional things without judgment
- If something fails, explain what went wrong and how to retry manually
