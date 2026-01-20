# lf init: Interactive Setup

Replaces `lfops init` and `lfops install` with a single interactive prompt.

## What Changed

**Before:** Two separate commands with imperative logic:
- `lfops init` - create .lf/config.yaml
- `lfops install` - install dependencies via Homebrew/npm

**After:** One command that runs an AI-guided setup:
- `lf init` - interactive prompt walks user through setup

## Components

### Interactive Prompt (`src/loopflow/templates/commands/init.md`)

Five-phase setup flow:
1. Check environment (brew, npm, coding agents, worktrunk)
2. Install missing required deps
3. Configure repository (.lf/config.yaml)
4. Optional extras (additional agents, superpowers, IDE prefs)
5. Summary

Requires at least one coding agent: Claude Code, Codex, or Gemini CLI.

### Dependency Module (`src/loopflow/lf/deps.py`)

Programmatic dependency checking for use by other commands:

```python
from loopflow.lf.deps import require_wt, require_agent

require_wt()                    # Ensure worktrunk
require_agent("claude")         # Ensure Claude Code
require_agent("codex", repo_root=path)  # Install + configure
```

Supports: `wt` (brew), `claude`, `codex`, `gemini` (npm).

### Reduced lfops/init.py

Now only contains `doctor` and `version` commands. Setup logic moved to the interactive prompt.

### Maestro Integration

`SetupService.swift` updated to install deps directly (Node.js, Claude Code, worktrunk) since `lf init` is interactive and can't run headless.

## Design Decisions

- **Interactive over imperative:** Setup involves choices (which agent, optional extras). An AI prompt handles this better than flag parsing.
- **Multiple agent support:** Users can pick Claude Code, Codex, or Gemini CLI based on their API keys and preferences.
- **deps.py for programmatic use:** Commands that need dependencies (like `lf agent`) can auto-install at runtime without requiring `lf init` first.
