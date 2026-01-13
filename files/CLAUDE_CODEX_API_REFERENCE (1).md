# Claude & Codex API Reference: A Technical Deep Dive

A comprehensive comparison of Anthropic's Claude ecosystem (Claude API, Agent SDK, Claude Code) and OpenAI's Codex ecosystem (Responses API, Agents SDK, Codex CLI), focusing on configuration, extensibility, and implementation details.

---

## Table of Contents

1. [API Landscape Overview](#api-landscape-overview)
2. [Context Configuration: CLAUDE.md vs AGENTS.md](#context-configuration-claudemd-vs-agentsmd)
3. [Skills vs Subagents: Mechanical Differences](#skills-vs-subagents-mechanical-differences)
4. [Claude vs Codex Feature Comparison](#claude-vs-codex-feature-comparison)
5. [Anthropic API Stack Explained](#anthropic-api-stack-explained)
6. [OpenAI API Stack Explained](#openai-api-stack-explained)
7. [Agent Skills Open Standard](#agent-skills-open-standard)
8. [Tool Use & Function Calling](#tool-use--function-calling)
9. [MCP Integration](#mcp-integration)
10. [Configuration Deep Dive](#configuration-deep-dive)

---

## API Landscape Overview

### Anthropic's Stack

```
┌─────────────────────────────────────────────────────────────┐
│                     Claude.ai (Consumer)                     │
│         Web/Mobile/Desktop interface for end users          │
└─────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                     Claude Code (Product)                    │
│   CLI + VS Code Extension for agentic coding workflows      │
│   - Terminal UI (TUI)                                       │
│   - Git integration                                         │
│   - File operations                                         │
│   - Hooks, Skills, Subagents                                │
└─────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                Claude Agent SDK (Framework)                  │
│   Python/TypeScript SDK wrapping Claude Code capabilities   │
│   - Same tools as Claude Code                               │
│   - Programmatic access                                     │
│   - Build custom agents                                     │
└─────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                  Claude Messages API (Core)                  │
│   Low-level API for direct model interaction                │
│   - Tool use / function calling                             │
│   - Streaming                                               │
│   - Extended thinking                                       │
│   - Batch processing                                        │
└─────────────────────────────────────────────────────────────┘
```

### OpenAI's Stack

```
┌─────────────────────────────────────────────────────────────┐
│                    ChatGPT (Consumer)                        │
│         Web/Mobile/Desktop interface for end users          │
└─────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                    Codex CLI (Product)                       │
│   Terminal-based coding agent (open source, Rust)           │
│   - Local + Cloud execution                                 │
│   - Skills, slash commands                                  │
│   - Review workflows                                        │
└─────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                 OpenAI Agents SDK (Framework)                │
│   Python/TypeScript SDK for multi-agent workflows           │
│   - Agents, Handoffs, Guardrails                            │
│   - Sessions management                                     │
│   - Tracing                                                 │
└─────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                   Responses API (Core)                       │
│   API designed for agentic workflows                        │
│   - Built-in tools (code interpreter, web search)           │
│   - Background tasks                                        │
│   - State management                                        │
└─────────────────────────────────────────────────────────────┘
```

---

## Context Configuration: CLAUDE.md vs AGENTS.md

### Can You Make a Non-CLAUDE.md File Act Like CLAUDE.md?

**Short answer:** Yes, with limitations.

### Claude Code Configuration

**CLAUDE.md is special but not unique.** Claude Code loads context from a hierarchy:

```
Precedence (highest to lowest):
1. Managed settings (enterprise/org-level)
2. User settings (~/.claude/)
3. Project settings (.claude/)
4. Local settings (.claude/settings.local.json)
```

**Alternative context sources:**

1. **Skills (auto-loaded based on description matching)**
   ```
   ~/.claude/skills/my-skill/SKILL.md
   .claude/skills/my-skill/SKILL.md
   ```
   
2. **Subagents (invoked via Task tool)**
   ```
   ~/.claude/agents/my-agent.md
   .claude/agents/my-agent.md
   ```

3. **Slash commands (user-invoked)**
   ```
   ~/.claude/commands/my-command.md
   .claude/commands/my-command.md
   ```

4. **Append system prompt flag:**
   ```bash
   claude --append-system-prompt "Additional instructions here"
   ```

**Key insight:** CLAUDE.md is injected into the user message with a system reminder:
```
<system-reminder>
IMPORTANT: this context may or may not be relevant to your tasks. 
You should not respond to this context unless it is highly relevant to your task.
</system-reminder>
```

This means Claude can *ignore* CLAUDE.md content if it's not relevant, which is why keeping it focused matters.

### Codex CLI Configuration

**AGENTS.md is the equivalent, with more flexibility:**

```toml
# ~/.codex/config.toml
project_doc_fallback_filenames = ["TEAM_GUIDE.md", ".agents.md", "CONTEXT.md"]
project_doc_max_bytes = 65536
```

**Discovery order per directory:**
1. `AGENTS.override.md` (if exists)
2. `AGENTS.md`
3. Files in `project_doc_fallback_filenames`

**You can absolutely use custom filenames** by adding them to the fallback list. Codex will treat `TEAM_GUIDE.md` identically to `AGENTS.md`.

**Additional Codex context options:**

```toml
# config.toml
experimental_instructions_file = "path/to/instructions.md"  # Experimental replacement for AGENTS.md
developer_instructions = "Additional instructions here"      # Inline instructions
```

### Comparison Table: Context Configuration

| Feature | Claude Code | Codex CLI |
|---------|-------------|-----------|
| Primary file | CLAUDE.md | AGENTS.md |
| Custom filename support | No (only CLAUDE.md) | Yes (via fallback list) |
| Override mechanism | No | AGENTS.override.md |
| Directory hierarchy | Yes (walks up to project root) | Yes (walks down from root) |
| Global config | ~/.claude/CLAUDE.md | ~/.codex/AGENTS.md |
| Max size control | Not configurable | `project_doc_max_bytes` |
| Append at runtime | `--append-system-prompt` | `developer_instructions` |

---

## Skills vs Subagents: Mechanical Differences

### How Skills Work (Implementation)

Skills use **progressive disclosure** - a three-level loading system:

**Level 1: Metadata (always loaded, ~30-100 tokens each)**
```yaml
# Only name and description go into system prompt at startup
---
name: pdf-processing
description: Extract text and tables from PDF files...
---
```

**Level 2: Full SKILL.md (loaded on match)**
When Claude determines a skill is relevant, it reads the full file via bash:
```
Claude invokes: bash: read pdf-skill/SKILL.md → Instructions loaded into context
```

**Level 3: Supporting files (loaded on demand)**
Referenced files are read only when needed:
```
pdf-processing/
├── SKILL.md         # Level 2
├── FORMS.md         # Level 3 (loaded if form-filling needed)
├── REFERENCE.md     # Level 3 (loaded if API details needed)
└── scripts/
    └── fill_form.py # Executed without reading into context
```

**API injection point:** Skills metadata is injected into the **system prompt** at session start.

### How Subagents Work (Implementation)

Subagents spawn **completely separate conversations**:

```
Main conversation context
    │
    ├── Claude calls Task tool with:
    │   - subagent_type: "code-reviewer" (or custom agent name)
    │   - prompt: "Review this code for security issues"
    │
    └── New conversation starts with:
        - Own system prompt (from agent .md file)
        - Own context window
        - Own tool permissions
        - Returns results to main conversation
```

**Each subagent invocation = 2+ API calls** (one to spawn, one+ for the subagent's work).

**API injection point:** Subagent definitions are stored as markdown files, loaded when the Task tool is invoked.

### Skills vs Subagents: When to Use

| Aspect | Skills | Subagents |
|--------|--------|-----------|
| **Context** | Shares main conversation | Isolated context window |
| **Invocation** | Auto-triggered by description match | Explicit via Task tool |
| **Token cost** | Adds to main context | Separate API calls |
| **Tool access** | Same as main agent | Can have restricted tools |
| **Use case** | Reusable knowledge/patterns | Complex workflows, parallel work |
| **Persistence** | Knowledge stays in context | Results returned, rest discarded |

**Best practice from Daniel Miessler:**
```
Skills contain commands (in workflows/ subdirectories)
Commands don't contain anything—they're leaf nodes
Agents can EXECUTE skills and commands as parallel workers
```

### Codex Skills (Comparison)

Codex adopted the same SKILL.md format:

```
~/.codex/skills/
├── .system/          # Built-in skills (plan, skill-creator)
└── my-skill/
    └── SKILL.md
```

**Key difference:** Codex skills are invoked with `$skill-name` prefix:
```
$skill-installer install create-plan from .experimental
```

**Codex skill discovery:**
- Only loads YAML frontmatter into system prompt
- Body stays on disk unless explicitly invoked
- Same progressive disclosure principle

---

## Claude vs Codex Feature Comparison

### What They Agree On

Both Claude Code and Codex CLI have converged on:

1. **Context files (CLAUDE.md / AGENTS.md)**
   - Hierarchical loading
   - Project-specific + global
   - Markdown format

2. **Skills system**
   - SKILL.md format (now an open standard)
   - Progressive disclosure
   - Bundled scripts/resources

3. **MCP support**
   - Both support Model Context Protocol
   - stdio and HTTP transports
   - OAuth flows

4. **Permission/approval modes**
   - Multiple tiers (auto-accept, ask, never)
   - Sandbox options

5. **Slash commands**
   - User-invoked shortcuts
   - Custom command directories

6. **Session management**
   - Resume previous sessions
   - Context persistence

### Where They Differ

| Feature | Claude Code | Codex CLI |
|---------|-------------|-----------|
| **Language** | TypeScript (closed source) | Rust (open source) |
| **Subagents** | Full system (Task tool, fork context) | Not native (use Agents SDK) |
| **Hooks** | 8 event types (PreToolUse, PostToolUse, Stop, etc.) | Limited (notify on turn complete) |
| **Cloud execution** | No (local only) | Yes (Codex Cloud) |
| **Review workflow** | Via subagents | Native `/review` command |
| **Override files** | No | AGENTS.override.md |
| **Custom context files** | No | `project_doc_fallback_filenames` |
| **IDE integration** | VS Code extension | VS Code extension |
| **Web interface** | Claude Code for Web | Codex Cloud |

### Hooks Comparison

**Claude Code hooks:**
```json
{
  "hooks": {
    "PreToolUse": [...],
    "PostToolUse": [...],
    "Stop": [...],
    "SubagentStart": [...],
    "SubagentStop": [...],
    "SessionStart": [...],
    "Notification": [...]
  }
}
```

**Codex hooks:**
```toml
# Limited to notifications
[notify]
command = "path/to/notification-script"
```

---

## Anthropic API Stack Explained

### Claude Messages API

The foundational API for all Claude interactions.

```python
import anthropic

client = anthropic.Anthropic()
response = client.messages.create(
    model="claude-sonnet-4-5",
    max_tokens=1024,
    messages=[{"role": "user", "content": "Hello"}],
    tools=[...],  # Optional function calling
    system="..."  # Optional system prompt
)
```

**Features:**
- Tool use / function calling
- Streaming
- Extended thinking (beta)
- Batch processing
- Vision (images, PDFs)
- Structured outputs

### Claude Agent SDK (formerly Claude Code SDK)

**What it is:** A Python/TypeScript library that wraps Claude Code's capabilities for programmatic use.

```python
from claude_agent_sdk import query, ClaudeAgentOptions

options = ClaudeAgentOptions(
    system_prompt="You are a helpful assistant",
    allowed_tools=["Read", "Write", "Bash"],
    permission_mode='acceptEdits',
    cwd="/path/to/project"
)

async for message in query(prompt="Create hello.py", options=options):
    print(message)
```

**Key insight:** The Agent SDK **bundles Claude Code CLI** - no separate installation required.

**What it provides:**
- Same tools as Claude Code (file operations, bash, code execution)
- Context management (automatic compaction)
- Rich tool ecosystem
- Hooks, Skills, Subagents support
- MCP extensibility

**Authentication:**
- Anthropic API key (ANTHROPIC_API_KEY)
- Amazon Bedrock (CLAUDE_CODE_USE_BEDROCK=1)
- Google Vertex AI (CLAUDE_CODE_USE_VERTEX=1)
- Microsoft Foundry (CLAUDE_CODE_USE_FOUNDRY=1)

### When to Use Each

| Use Case | API |
|----------|-----|
| Chat application | Messages API |
| Code generation (no execution) | Messages API with tools |
| Agentic coding workflows | Claude Code CLI |
| Custom autonomous agents | Agent SDK |
| Browser automation | Agent SDK + Computer Use |
| CI/CD integration | Agent SDK |

---

## OpenAI API Stack Explained

### Responses API

OpenAI's API designed specifically for agents:

```python
from openai import OpenAI

client = OpenAI()
response = client.responses.create(
    model="gpt-5",
    input="Analyze this codebase",
    tools=[
        {"type": "code_interpreter"},
        {"type": "web_search"}
    ]
)
```

**Built-in tools:**
- Code interpreter
- Web search
- File search
- Computer use (beta)

### OpenAI Agents SDK

**What it is:** A lightweight framework for multi-agent workflows.

```python
from agents import Agent, Runner

spanish_agent = Agent(
    name="Spanish agent",
    instructions="You only speak Spanish.",
)

english_agent = Agent(
    name="English agent",
    instructions="You only speak English",
)

triage_agent = Agent(
    name="Triage agent",
    instructions="Handoff to appropriate agent based on language.",
    handoffs=[spanish_agent, english_agent],
)

result = await Runner.run(triage_agent, input="Hola, ¿cómo estás?")
```

**Core primitives:**
- **Agents:** LLMs with instructions, tools, and handoffs
- **Handoffs:** Transfer control between agents
- **Guardrails:** Input/output validation
- **Sessions:** Automatic conversation history
- **Tracing:** Built-in debugging and monitoring

### Codex SDK

**For using Codex as an MCP server:**

```bash
codex mcp-server  # Run Codex as MCP server
```

Tools exposed:
- `codex`: Run a Codex session
- `codex-reply`: Continue a session

---

## Agent Skills Open Standard

### The Specification

Released December 18, 2025, Agent Skills is now an **open standard** at agentskills.io.

**File structure:**
```
my-skill/
├── SKILL.md          # Required: main instructions
├── reference.md      # Optional: detailed docs
├── examples.md       # Optional: usage examples
└── scripts/
    └── helper.py     # Optional: executable code
```

**SKILL.md format:**
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

### Adoption

Already adopted by:
- **Claude Code** (Anthropic)
- **Codex CLI** (OpenAI)
- **ChatGPT** (OpenAI - `/home/oai/skills`)
- **VS Code** (Microsoft)
- **Cursor**
- **GitHub Copilot**
- **Amp, Letta, Goose**

### Why It Matters

Skills solve the **vendor lock-in problem:**
- Write once, use across platforms
- Portable expertise
- Community-shareable

**Anthropic's strategy:** Build standards (MCP, Agent Skills) rather than proprietary moats.

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

# Response includes tool_use block
# You execute the tool and return tool_result
```

**Tool types:**
1. **Client tools:** You implement and execute
2. **Server tools:** Anthropic executes (web_search, web_fetch)
3. **Anthropic-defined tools:** Computer use, text editor, bash

### Programmatic Tool Calling

New feature allowing Claude to orchestrate tools via code:

```python
# Claude writes Python that calls multiple tools
# Script runs in sandbox, pauses for tool results
# Only final output enters context
```

**Benefits:**
- Reduced context consumption
- Parallel tool execution
- Complex orchestration logic

### OpenAI Tool Use

Similar structure:
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

## Configuration Deep Dive

### Claude Code Full Configuration

**settings.json locations:**
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

### Codex Full Configuration

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

### Environment Variables

**Claude Code:**
```bash
ANTHROPIC_API_KEY=sk-...
ANTHROPIC_BASE_URL=...          # Custom endpoint
ANTHROPIC_MODEL=claude-opus-4-5  # Default model
CLAUDE_CODE_USE_BEDROCK=1       # Use AWS Bedrock
CLAUDE_CODE_USE_VERTEX=1        # Use Google Vertex
```

**Codex:**
```bash
OPENAI_API_KEY=sk-...
CODEX_HOME=~/.codex             # Config directory
CODEX_SANDBOX_NETWORK_DISABLED=1  # Network in sandbox
```

---

## Summary: Key Takeaways

### Convergence

1. **Skills are becoming a standard** - Both platforms adopted the same format
2. **MCP is universal** - Both support Model Context Protocol
3. **Progressive disclosure** - Both use lazy loading for context efficiency
4. **Hierarchical config** - Both support global + project settings

### Divergence

1. **Subagents:** Claude Code has rich subagent system; Codex relies on Agents SDK
2. **Hooks:** Claude Code has comprehensive hooks; Codex is limited
3. **Openness:** Codex is open source; Claude Code is closed
4. **Cloud execution:** Codex has Codex Cloud; Claude Code is local-only

### Practical Recommendations

**For custom context files:**
- Claude Code: Use skills for auto-triggered context
- Codex: Use `project_doc_fallback_filenames` for custom filenames

**For parallel agents:**
- Claude Code: Subagents (context: fork) or external orchestration
- Codex: OpenAI Agents SDK with handoffs

**For tool extension:**
- Both: MCP servers for external connectivity
- Both: Skills for procedural knowledge

**For portability:**
- Write skills in Agent Skills format - works everywhere
- Use MCP for tool integrations - also standardized

---

*Compiled January 2026. Based on official documentation and community research.*
