# Technical Reference: Claude Code & Codex

Deep technical details on APIs, configuration, skills, hooks, and implementation patterns for AI coding agents.

---

## API Stack Overview

### Anthropic's Stack

```
Claude.ai (Consumer)
    │
Claude Code (Product)
    │   CLI + VS Code Extension for agentic coding
    │   Terminal UI, Git integration, File operations
    │   Hooks, Skills, Subagents
    │
Claude Agent SDK (Framework)
    │   Python/TypeScript SDK wrapping Claude Code
    │   Same tools as Claude Code, programmatic access
    │
Claude Messages API (Core)
        Tool use, streaming, extended thinking, batch
```

### OpenAI's Stack

```
ChatGPT (Consumer)
    │
Codex CLI (Product)
    │   Terminal-based coding agent (open source, Rust)
    │   Local + Cloud execution, Skills, slash commands
    │
OpenAI Agents SDK (Framework)
    │   Python/TypeScript for multi-agent workflows
    │   Agents, Handoffs, Guardrails, Sessions, Tracing
    │
Responses API (Core)
        Built-in tools (code interpreter, web search)
        Background tasks, state management
```

---

## Context Configuration

### CLAUDE.md vs AGENTS.md

| Feature | Claude Code | Codex CLI |
|---------|-------------|-----------|
| Primary file | CLAUDE.md | AGENTS.md |
| Custom filename support | No (only CLAUDE.md) | Yes (via fallback list) |
| Override mechanism | No | AGENTS.override.md |
| Directory hierarchy | Walks up to project root | Walks down from root |
| Global config | ~/.claude/CLAUDE.md | ~/.codex/AGENTS.md |
| Max size control | Not configurable | `project_doc_max_bytes` |
| Append at runtime | `--append-system-prompt` | `developer_instructions` |

### Claude Code Configuration Hierarchy

```
Precedence (highest to lowest):
1. Managed settings (enterprise/org-level)
2. User settings (~/.claude/)
3. Project settings (.claude/)
4. Local settings (.claude/settings.local.json)
```

### Codex Configuration

```toml
# ~/.codex/config.toml
project_doc_fallback_filenames = ["TEAM_GUIDE.md", ".agents.md", "CONTEXT.md"]
project_doc_max_bytes = 65536
experimental_instructions_file = "path/to/instructions.md"
developer_instructions = "Additional instructions here"
```

Discovery order per directory:
1. `AGENTS.override.md` (if exists)
2. `AGENTS.md`
3. Files in `project_doc_fallback_filenames`

---

## Skills System

### How Skills Work

Skills use **progressive disclosure** - a three-level loading system:

**Level 1: Metadata (always loaded, ~30-100 tokens)**
```yaml
---
name: pdf-processing
description: Extract text and tables from PDF files...
---
```

**Level 2: Full SKILL.md (loaded on match)**

When Claude determines a skill is relevant, it reads the full file.

**Level 3: Supporting files (loaded on demand)**
```
pdf-processing/
├── SKILL.md         # Level 2
├── FORMS.md         # Level 3 (loaded if form-filling needed)
├── REFERENCE.md     # Level 3 (loaded if API details needed)
└── scripts/
    └── fill_form.py # Executed without reading into context
```

### SKILL.md Format (Open Standard)

Released December 2025 at agentskills.io:

```yaml
---
name: skill-name                    # Required, max 100 chars
description: What it does           # Required, max 500 chars
metadata:
  short-description: Brief          # Optional
  allowed-tools: Read, Bash         # Optional
  context: fork                     # Optional (fork = isolated subagent)
  mode: true                        # Optional (appears in Mode Commands)
---

# Skill Instructions

Markdown content with instructions for the agent...
```

### Skills vs Subagents

| Aspect | Skills | Subagents |
|--------|--------|-----------|
| **Context** | Shares main conversation | Isolated context window |
| **Invocation** | Auto-triggered by description match | Explicit via Task tool |
| **Token cost** | Adds to main context | Separate API calls |
| **Tool access** | Same as main agent | Can have restricted tools |
| **Use case** | Reusable knowledge/patterns | Complex workflows, parallel work |
| **Persistence** | Knowledge stays in context | Results returned, rest discarded |

### Subagents Implementation

Subagents spawn completely separate conversations:

```
Main conversation context
    │
    ├── Claude calls Task tool with:
    │   - subagent_type: "code-reviewer"
    │   - prompt: "Review this code for security issues"
    │
    └── New conversation starts with:
        - Own system prompt (from agent .md file)
        - Own context window
        - Own tool permissions
        - Returns results to main conversation
```

Each subagent invocation = 2+ API calls.

---

## Hook Systems

### Claude Code Hook Events

| Event | When | Can Block? | Use Case |
|-------|------|------------|----------|
| PreToolUse | Before tool execution | Yes (exit 2) | Validate, block dangerous commands |
| PostToolUse | After tool completion | No | Log, validate results, cleanup |
| Stop | Agent considers stopping | Yes | Verify task completion |
| SubagentStop | Subagent finishes | Yes | Ensure subtasks complete |
| UserPromptSubmit | Before Claude processes | Yes | Validate prompts, inject context |
| SessionStart | Session begins | No | Load context, init environment |
| SessionEnd | Session ends | No | Cleanup, logging |
| PreCompact | Before compaction | No | Backup transcripts |
| Notification | User interactions | No | Custom notifications |

### Hook Configuration

```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Bash",
      "hooks": [{
        "type": "command",
        "command": "/path/to/validate.sh"
      }]
    }],
    "PostToolUse": [{
      "matcher": "Write|Edit",
      "hooks": [{
        "type": "command",
        "command": "/path/to/lint.sh ${file_path}"
      }]
    }],
    "Stop": [{
      "matcher": "*",
      "hooks": [{
        "type": "prompt",
        "prompt": "Verify: tests run, build succeeded, questions answered."
      }]
    }]
  }
}
```

### Common Hook Patterns

**File Guard (PreToolUse):** Block access to sensitive files (.env, credentials)

**Lint on Save (PostToolUse):** Run linter after every Write/Edit

**Test on Change (PostToolUse):** Run tests for changed files

**Completion Verification (Stop):** Ensure tests pass before allowing stop

**Context Injection (UserPromptSubmit):** Load codebase map at session start

### Codex Hooks (Limited)

```toml
# Limited to notifications
[notify]
command = "path/to/notification-script"
```

---

## Claude Code vs Codex Feature Comparison

### What They Agree On

1. **Context files (CLAUDE.md / AGENTS.md)** - Hierarchical loading, project-specific + global, markdown format

2. **Skills system** - SKILL.md format (now an open standard), progressive disclosure, bundled scripts/resources

3. **MCP support** - Both support Model Context Protocol, stdio and HTTP transports, OAuth flows

4. **Permission/approval modes** - Multiple tiers (auto-accept, ask, never), sandbox options

5. **Slash commands** - User-invoked shortcuts, custom command directories

6. **Session management** - Resume previous sessions, context persistence

### Where They Differ

| Feature | Claude Code | Codex CLI |
|---------|-------------|-----------|
| **Language** | TypeScript (closed source) | Rust (open source) |
| **Subagents** | Full system (Task tool, fork context) | Not native (use Agents SDK) |
| **Hooks** | 8 event types | Limited (notify on turn complete) |
| **Cloud execution** | No (local only) | Yes (Codex Cloud) |
| **Review workflow** | Via subagents | Native `/review` command |
| **Override files** | No | AGENTS.override.md |
| **Custom context files** | No | `project_doc_fallback_filenames` |

---

## Full Configuration Examples

### Claude Code settings.json

Locations:
1. `~/.claude/settings.json` (user)
2. `.claude/settings.json` (project, shared)
3. `.claude/settings.local.json` (project, personal)

```json
{
  "model": "claude-sonnet-4-5-20250929",
  "permissions": {
    "allowedTools": ["Read", "Write", "Bash(git *)"],
    "deny": ["Read(./.env)", "Read(./.env.*)"]
  },
  "hooks": {
    "PostToolUse": [{
      "matcher": "Write(*.py)",
      "hooks": [{
        "type": "command",
        "command": "python -m black $file"
      }]
    }]
  },
  "enabledPlugins": {
    "formatter@team-tools": true
  }
}
```

### Codex config.toml

```toml
# ~/.codex/config.toml

# Model selection
model = "gpt-5.2-codex"
model_reasoning_effort = "high"

# Context management
model_auto_compact_token_limit = 233000
tool_output_token_limit = 25000

# Approval and sandbox
approval_policy = "on-failure"  # "always", "never", "on-failure"
sandbox_mode = "workspace-write"  # "danger-full-access", "workspace-write"

# Features
[features]
web_search_request = true
skills = true
shell_snapshot = true
unified_exec = true
apply_patch_freeform = true
ghost_commit = false

# Project trust
[projects."/Users/me/Projects"]
trust_level = "trusted"

# MCP servers
[mcp_servers.my-server]
command = "node"
args = ["server.js"]
env = {API_KEY = "..."}

# TUI settings
[tui]
notifications = true
animations = true
```

Key insight: Raising `tool_output_token_limit` lets the model read more in one go. Defaults are small and fail silently.

---

## MCP Integration

### Model Context Protocol

Both platforms support MCP for external tool integration.

**Claude Code MCP config:**
```json
// .mcp.json (project root)
{
  "servers": {
    "github": {
      "command": "npx",
      "args": ["-y", "@anthropic/mcp-server-github"],
      "env": {"GITHUB_TOKEN": "..."}
    }
  }
}
```

**Codex MCP config:**
```toml
# ~/.codex/config.toml
[mcp_servers.github]
command = "npx"
args = ["-y", "@anthropic/mcp-server-github"]
env = {GITHUB_TOKEN = "..."}
```

**MCP vs Skills:**
- **MCP:** Provides tools (connectivity to external systems)
- **Skills:** Provides knowledge (how to use those tools effectively)

They're complementary - an MCP server connects to GitHub, a skill teaches best practices for using it.

---

## Environment Variables

### Claude Code

```bash
ANTHROPIC_API_KEY=sk-...
ANTHROPIC_BASE_URL=...          # Custom endpoint
ANTHROPIC_MODEL=claude-opus-4-5  # Default model
CLAUDE_CODE_USE_BEDROCK=1       # Use AWS Bedrock
CLAUDE_CODE_USE_VERTEX=1        # Use Google Vertex
```

### Codex

```bash
OPENAI_API_KEY=sk-...
CODEX_HOME=~/.codex             # Config directory
CODEX_SANDBOX_NETWORK_DISABLED=1  # Network in sandbox
```

---

## Tool Use & Function Calling

### Claude Tool Use

```python
response = client.messages.create(
    model="claude-sonnet-4-5",
    max_tokens=1024,
    tools=[{
        "name": "get_weather",
        "description": "Get current weather",
        "input_schema": {
            "type": "object",
            "properties": {
                "location": {"type": "string"}
            },
            "required": ["location"]
        }
    }],
    messages=[{"role": "user", "content": "Weather in SF?"}]
)
```

**Tool types:**
1. **Client tools:** You implement and execute
2. **Server tools:** Anthropic executes (web_search, web_fetch)
3. **Anthropic-defined tools:** Computer use, text editor, bash

### OpenAI Tool Use

```python
response = client.chat.completions.create(
    model="gpt-5",
    tools=[{
        "type": "function",
        "function": {
            "name": "get_weather",
            "description": "Get current weather",
            "parameters": {...}
        }
    }],
    messages=[...]
)
```

---

## Notification Systems

### Where Notifications Get Registered

**1. Terminal Bell (Built-in)**
```bash
claude config set --global preferredNotifChannel terminal_bell
echo -e "\a"  # Test
```

Supported: iTerm2, Ghostty, most Unix terminals
Not Supported: VS Code terminal (inconsistent)

**2. OSC Escape Sequences**
```bash
# OSC 777 format (VSCode, rxvt-unicode)
printf '\033]777;notify;Title;Message\007'

# OSC 9 format (iTerm2, Windows Terminal)
printf '\033]9;Message\007'
```

**3. Hooks System (Most Flexible)**
```json
{
  "hooks": {
    "Notification": [{
      "matcher": "",
      "hooks": [{
        "type": "command",
        "command": "notify-send 'Claude Code' 'Awaiting your input'"
      }]
    }],
    "Stop": [{
      "matcher": "",
      "hooks": [{
        "type": "command",
        "command": "osascript -e 'display notification \"Task Done\" with title \"Claude Code\"'"
      }]
    }]
  }
}
```

**4. Platform-Specific**

macOS:
```bash
terminal-notifier -title "Claude Code" -message "Task complete" -sound Glass
osascript -e 'display notification "Task completed" with title "Claude Code" sound name "Glass"'
```

Linux:
```bash
notify-send "Claude Code" "Awaiting your input"
```

---

## Workflow Patterns

### steipete's "Inference Speed" Workflow

**Core Principles:**

1. **Don't read code:** "These days I don't read much code anymore. I watch the stream and sometimes look at key parts."

2. **No plan mode:** Start conversations naturally, explore, build plan collaboratively, then write "build" when ready.

3. **Commit to main:** "I simply commit to main... I find the added cognitive load of having to think of different states in my projects unnecessary."

4. **Never revert:** "If something isn't how I like it, I ask the model to change it."

5. **Queue, don't orchestrate:** Use Codex's queueing feature - add ideas to the pipeline.

**Cross-Project Operations:**
```bash
"look at ../vibetunnel and do the same for Sparkle changelogs"
"find all my recent go projects and implement this change there too + update changelog"
```

**The Oracle Pattern (Escalation):**

When agents get stuck, escalate to a stronger model:
> "oracle - it's a CLI that allows the agent to run GPT 5 Pro and upload files + a prompt... The model sometimes by itself triggered oracle when it got stuck."

### Boris Cherny's Workflow

**Stats:** 259 PRs in 30 days, 497 commits - all via Claude Code.

**Parallel Execution:**
- 5-10 Claude instances simultaneously
- iTerm2 notifications for managing streams
- "Teleport" command to hand off between web and terminal

**CLAUDE.md Practice:**
> "Anytime we see Claude do something incorrectly we add it to CLAUDE.md, so Claude knows not to do it next time."

**Verification Loop:**
> "Claude tests every single change using the Claude Chrome extension. Opens browser, tests UI, iterates until code works and UX feels good."

### Plan -> Execute Pattern

1. **Plan Mode (Shift+Tab twice):** Claude analyzes but cannot modify files
2. **Write plan to plan.md:** Creates persistent artifact
3. **Review and adjust plan**
4. **Execute with plan as checklist**

### Benchy Pattern (Parallel Same-Prompt)

Run N agents with identical prompt, pick best result.

```
project/
├── specs/
│   └── feature.md          # Shared spec
└── trees/
    ├── feature-1/          # Agent 1 worktree
    ├── feature-2/          # Agent 2 worktree
    └── feature-3/          # Agent 3 worktree
```

Compare implementations, cherry-pick best approach.

---

## Git Worktree Management

### Why Worktrees for Agents

Git worktrees solve the fundamental parallel agent problem: **file conflicts**. Each agent needs:
- Its own working directory
- Its own branch
- Isolated from other agents' changes

### The Contrarian View

steipete: "I simply commit to main. I find the added cognitive load of having to think of different states in my projects unnecessary and prefer to evolve it linearly."

His alternative - multiple machines: "I usually work on two Macs... Sometimes I edit different parts of the same project on each machine and sync via git."

### When Worktrees Are Valuable

- Team environments with shared repos
- Need for reviewable, isolated PRs
- CI/CD pipelines expect branches
- Multiple features that might conflict
- Want checkpoint/rollback capability

### Manual Worktree Workflow

```bash
# Create worktree
git worktree add ../feature-auth -b feature/auth
cd ../feature-auth
claude

# When done
git worktree remove ../feature-auth
git branch -d feature/auth
```

---

## Summary: Key Convergence Points

1. **Skills are becoming a standard** - Both platforms adopted the same format
2. **MCP is universal** - Both support Model Context Protocol
3. **Progressive disclosure** - Both use lazy loading for context efficiency
4. **Hierarchical config** - Both support global + project settings

## Key Divergence Points

1. **Subagents:** Claude Code has rich subagent system; Codex relies on Agents SDK
2. **Hooks:** Claude Code has comprehensive hooks; Codex is limited
3. **Openness:** Codex is open source; Claude Code is closed
4. **Cloud execution:** Codex has Codex Cloud; Claude Code is local-only

---

*Compiled January 2026*
