---
interactive: true
produces: .lf/config.yaml
---
Guide the user through setting up loopflow in this repository.

## Reviewer mode

The launch prompt identifies the reviewer for this exercise.

- **Human reviewer:** ask the setup questions below one at a time.
- **Parent reviewer:** run the same detection, preserve a valid existing
  override, and otherwise leave `agent` unset when Codex is installed. If Codex
  is absent, choose the sole detected agent or the first detected supported
  harness. Send the exact repo-config change to the Task through the review protocol
  and verify its reply; do not edit the Task's repository yourself.
  Never create or modify personal user config without a human—report those
  choices as deferred. If no agent is installed, return the install
  instructions and request changes.

## Phase 1: Environment check

Run these checks:
```bash
git rev-parse --show-toplevel
uname -s
command -v claude
command -v codex
command -v opencode
test -f .lf/config.yaml && echo "config exists"
test -f ~/.lf/config.yaml && echo "user config exists"
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
User config: ~/.lf/config.yaml (exists)
```

## Phase 2: Agent guidance

**Multiple agents found:** Report all. Codex is Loopflow's implicit default, so
leave `agent` unset unless the human reviewer explicitly chooses a repo-wide
override. With a parent reviewer, preserve a valid existing override; otherwise
leave `agent` unset when Codex is installed, or choose the first detected
supported harness when it is not.

**One agent found:** Leave `agent` unset for Codex. Otherwise default to the sole
installed agent. Report the choice.

**None found:** Show install instructions and stop:
```
No coding agent found. Install one:
  Claude Code:  npm install -g @anthropic-ai/claude-code
  OpenCode:     go install github.com/anomalyco/opencode@latest
  Codex CLI:    npm install -g @openai/codex
Then run `lf init` again.
```

Do not run package managers. Init detects — it never installs.

## Phase 3: Create or update repo config

Repo config (`.lf/config.yaml`) is for team conventions. Only write repo
properties — never personal preferences like yolo, ide, chrome, or autoprune.

**No `.lf/config.yaml`:**
- Ask the human reviewer: "Initialize loopflow in this repo? This creates
  .lf/config.yaml"
- With a parent reviewer, direct the Task to create it with the deterministic
  default selected above
- Omit `agent` when using the implicit Codex default; otherwise write the
  detected or explicitly selected override

**Existing `.lf/config.yaml`:**
- Compare `supported_harnesses` against detected agents
- If new agents detected that aren't listed, offer to add them
- If current `agent:` value isn't installed, offer to switch the default
- If everything matches, report "Config is up to date" and move on
- Never overwrite custom `exclude` patterns or other user-tuned fields

Repo config template:
```yaml
# agent: <explicit-repo-override>

supported_harnesses:
  - <detected-agents>

exclude:
  - "*.lock"
  - node_modules
  - .venv
```

Use the agent name without model suffix: `agent: claude` not `agent: claude:opus`.

## Phase 4: User config

Personal preferences (yolo, ide, chrome, autoprune) belong in `~/.lf/config.yaml`.
Repo config wins when both files set the same key, so personal prefs should live
in the user file.

**User config exists:** Mention it and move on. Don't modify it.

**No user config:** With a human reviewer, offer to create `~/.lf/config.yaml`
with a couple of common preferences:
- Skip permission prompts (yolo)? [y/N]
- Preferred IDE setup? [skip]

Keep it brief — two questions, not an interrogation. Generate only what the user
chose. If they skip everything, don't create the file.

With a parent reviewer, do not ask these questions and do not create the user
file. Report the personal choices as deferred.

## Phase 5: Optional extras

**External skills via npx:** mention that any Claude Skills package can be run as
`lf npx/<owner>/<repo>` — fetched live, no install step. `gstack` and core loopflow
workflows are built in; npx covers everything else.

**Prompt authoring:** mention `lf prompt: <intent>` when the user wants to create
or tighten a repo skill, direction, or Wave goal.

## Phase 6: Next steps

Platform-aware guidance:

**macOS:**
```
Setup complete!

✓ Claude Code
✓ Codex (implicit default)
✓ .lf/config.yaml created

Next:
  lf prompt: create a skill # author a repo prompt
  lf design              # interactive design session
  lf debug -c            # paste an error, fix it
  lf --list              # see all steps and flows

  Download Loopflow for visual wave management
```

**Linux:**
```
Setup complete!

✓ OpenCode (repo override)
✓ .lf/config.yaml created

Next:
  lf prompt: create a skill # author a repo prompt
  lf design              # interactive design session
  lf debug -c            # paste an error, fix it
  lf --list              # see all steps and flows
```

## Conversation style

- Short, clear prompts
- One question at a time
- Let users skip optional things without judgment
- If something fails, explain what went wrong and how to retry manually
