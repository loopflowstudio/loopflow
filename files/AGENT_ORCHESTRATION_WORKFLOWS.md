# Agent Orchestration & Workflow Patterns

A practical compilation of tools, patterns, and workflow designs for AI coding agents. Focused on implementation details, architectural decisions, and what to build vs. buy.

---

## Table of Contents

1. [Parallel Agent Orchestration Tools](#parallel-agent-orchestration-tools)
2. [Git Worktree Management](#git-worktree-management)
3. [Skills & Context Engineering](#skills--context-engineering)
4. [Hook Systems](#hook-systems)
5. [Claude Code vs Codex CLI](#claude-code-vs-codex-cli)
6. [Workflow Patterns](#workflow-patterns)
7. [Architecture Decisions](#architecture-decisions)

---

## Parallel Agent Orchestration Tools

### Claude Squad (smtg-ai/claude-squad)
**Purpose:** TUI for managing multiple AI terminal agents in separate workspaces

**Architecture:**
- Written in Go
- Uses tmux for terminal session isolation
- Uses git worktrees for code isolation
- BubbleTea framework for TUI, lipgloss for styling

**Key Design:**
```
CLI Entry (main.go)
    └── app.Run()
        ├── session.Instance (per agent)
        │   ├── tmux.TmuxSession (terminal isolation)
        │   └── git.GitWorktree (code isolation)
        ├── ui.List, ui.PreviewPane
        └── session.Storage (persistence)
```

**Commands:**
```bash
cs                           # Launch TUI
cs -y, --autoyes            # Auto-accept prompts (experimental)
cs -p "aider --model X"     # Specify program to run
```

**Workflow:**
- Each agent runs in own session.Instance
- Instances transition: created → started → paused → resumed → terminated
- Background daemon for automatic prompt acceptance
- Sessions persist when detached

**What it solves:** Unified view of multiple agents, keyboard navigation, status monitoring

**Limitations:** Still requires manual task assignment, no intelligent routing

---

### Worktrunk (max-sixty/worktrunk)
**Purpose:** Git worktree management CLI designed for parallel AI agent workflows

**Philosophy:** Wraps git worktrees in clean interface. "Scaling agents becomes as simple as scaling git branches."

**Key Commands:**
```bash
wt switch feat              # Switch worktrees
wt switch -c -x claude feat # Create worktree + start Claude
wt remove                   # Clean up (worktree + branch)
wt list                     # List with status
wt merge                    # Squash, rebase, merge, clean up in one
```

**Extension Points:**
- Lifecycle hooks (on create, pre-merge, post-merge)
- LLM commit messages via `llm` CLI
- Claude Code integration
- CI status & PR links in list view

**What it solves:** The git worktree UX is clunky (typing branch name 3x). Worktrunk abstracts this.

**Install:** `brew install max-sixty/worktrunk/wt`

---

### Code Conductor (ryanmac/code-conductor)
**Purpose:** GitHub-native orchestration for multiple Claude Code agents

**Design:**
- Agents claim tasks from GitHub Issues (labeled `conductor:task`)
- Each agent works in isolated git worktree
- Auto-detects stack (React, Python, Go, etc.)

**Workflow:**
```bash
./conductor start frontend   # Launch frontend agent
./conductor start backend    # Launch backend agent in parallel
# Agents:
#   ✓ Claim task #42
#   ✓ Create isolated worktree
#   ✓ Implement feature
#   ✓ Open pull request
#   ✓ Move to next task
```

**What it solves:** Self-managing agents that work through backlog autonomously

---

### CCPM (automazeio/ccpm)
**Purpose:** Project management for Claude Code using GitHub Issues + Git worktrees

**Key Insight:** Uses GitHub Issues as database for multi-agent coordination. Progress visible to humans, not trapped in chat logs.

**Commands:**
```bash
/pm:prd-new memory-system      # Create PRD via brainstorming
/pm:prd-parse memory-system    # Transform to technical epic
/pm:epic-start memory-system   # Start parallel execution
/pm:issue-analyze 1234         # Analyze parallelization potential
/pm:epic-merge memory-system   # Merge when done
```

**Architecture:**
```
.claude/
├── context/          # Project-wide context
├── epics/           # Local workspace
│   └── [epic-name]/
│       ├── epic.md
│       ├── [#].md   # Individual tasks
│       └── updates/ # WIP
├── prds/            # PRD files
└── commands/pm/     # PM commands
```

**Parallelism:** Tasks marked `parallel: true` enable conflict-free concurrent development. Claims 12 agents across 3 issues in demos.

---

### CCSwarm (nwiizo/ccswarm)
**Purpose:** Multi-agent orchestration with specialized agents

**Architecture:**
```
ProactiveMaster (orchestrator)
├── Channel-Based Orchestration (zero shared state)
├── Task Analysis & Delegation (pattern matching)
└── Goal-Driven Planning
    
Claude ACP Integration
├── WebSocket: ws://localhost:9100
├── JSON-RPC 2.0
└── Auto-reconnect with exponential backoff
```

**Agent Types:** Frontend Specialist, Backend Specialist, etc.

**Commands:**
```bash
ccswarm init --name "Project" --agents frontend,backend
ccswarm start
ccswarm tui
ccswarm delegate analyze "Add authentication" --verbose
ccswarm delegate task "Add auth" --agent backend --priority high
```

**Features:**
- Predefined benchmark suites for agent evaluation
- Leaderboard system for comparing agent performance
- Template system for task patterns
- Can run without Claude Code (provider-agnostic)

---

### Parallel-CC (frankbria/parallel-cc)
**Purpose:** Automatic worktree creation for parallel Claude sessions

**Design:** Wrapper that detects parallel sessions and auto-creates worktrees.

```bash
# Terminal 1
cd ~/projects/myrepo && claude  # Gets main repo

# Terminal 2  
cd ~/projects/myrepo && claude  # Auto-creates worktree!
# Output: 📂 Parallel session detected - working in worktree
# Path: /home/user/projects/myrepo-worktrees/parallel-m4x2k9...
```

**Flow:**
1. Run `claude-parallel` (or aliased `claude`)
2. Wrapper checks for existing sessions
3. If parallel → creates worktree via gtr
4. cd into worktree, launch claude
5. On exit → session released, worktree cleaned

**What it solves:** Zero-config parallelism. Just open another terminal.

---

### Crystal (stravu/crystal)
**Purpose:** Desktop app for parallel Claude Code + Codex sessions

**Workflow:**
1. Create project (empty folder or existing git repo)
2. Create 1+ sessions per feature
3. Each session: isolated worktree, separate agent
4. Iterate with agent, each iteration commits
5. Review diffs, make manual edits
6. Squash + merge to main

**What it adds:** GUI layer, cross-agent comparison, visual diff review

---

### Claude-Code-by-Agents (baryhuang/claude-code-by-agents)
**Purpose:** Desktop app for multi-agent coordination via @mentions

**Flow:**
```
User → General Request → Orchestrator Analysis → Execution Plan
    ↓
Agent 1 ← Step 1 ← File Dependencies ← Coordination Logic
Agent 2 ← Step 2 ← Read Step 1 Output
Agent N ← Step N ← Read Previous Results
```

**Stack:** Deno backend (WebSocket), Frontend on localhost:3000

---

## Git Worktree Management

### Why Worktrees for Agents

Git worktrees solve the fundamental parallel agent problem: **file conflicts**. Each agent needs:
- Its own working directory
- Its own branch
- Isolated from other agents' changes

Without worktrees, parallel agents overwrite each other's edits and corrupt context.

### The Contrarian View (steipete)

Not everyone agrees worktrees are necessary:

> "I simply commit to main. Sometimes codex decides that it's too messy and automatically creates a worktree and then merges changes back, but it's rare and I only prompt that in exceptional cases. I find the added cognitive load of having to think of different states in my projects unnecessary and prefer to evolve it linearly."

**His alternative - multiple machines:**
> "I usually work on two Macs. My MacBook Pro on the big screen, and a Jump Desktop session to my Mac Studio on another screen. Some projects are cooking there, some here. Sometimes I edit different parts of the same project on each machine and sync via git. Simpler than worktrees because drifts on main are easy to reconcile."

**Caveat:** "I usually work alone, if you work in a bigger team that workflow obv won't fly."

### When Worktrees Are Still Valuable

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

### Automated Parallel Deployment Script

```bash
#!/bin/bash
function deploy_parallel_agents() {
    local feature_name="$1"
    local num_agents="${2:-3}"
    local prompt="$3"
    
    for i in $(seq 1 $num_agents); do
        local branch_name="${feature_name}-agent-${i}"
        local worktree_path="../${branch_name}"
        
        git worktree add "$worktree_path" -b "$branch_name"
        
        (
            cd "$worktree_path"
            if [ -n "$prompt" ]; then
                claude -p "$prompt" &
            else
                claude &
            fi
        ) &
    done
}

# Usage:
# deploy_parallel_agents "authentication" 3 "Implement OAuth2"
```

### Practical Considerations

**Token consumption:** Running 5-10 parallel agents burns through subscriptions fast. One user reported exceeding Pro subscription limits.

**Context switching overhead:** Managing multiple agents is "like moderating two separate meetings in neighboring conference rooms." Mental load is real.

**When it works:** Long-running tasks where one agent doesn't need your input. Bad for tasks requiring frequent human steering.

**Merge conflicts:** Even with worktrees, agents touching same files create conflicts. Some tools track which worktree was rebased onto which branch.

---

## Skills & Context Engineering

### Skills vs Commands vs Subagents vs CLAUDE.md

| Feature | Invocation | Purpose | Context Impact |
|---------|------------|---------|----------------|
| CLAUDE.md | Auto-loaded | Project memory, always-on instructions | Persistent, every session |
| Slash Commands | Explicit `/command` | Repeatable prompts, macros | On-demand |
| Skills | Auto-invoked by Claude | Domain expertise packages | Loaded when matched |
| Subagents | Delegated tasks | Isolated work, clean context | Separate context window |

### CLAUDE.md Best Practices

**Structure:**
```markdown
# Project: [Name]

## Tech Stack
- Framework: X
- Database: Y
- Testing: Z

## Directory Structure
- /src/api - REST endpoints
- /src/models - Database models
- /lib - Shared utilities

## Commands
- `npm run dev` - Start dev server
- `npm test` - Run tests
- `npm run lint` - Lint

## Conventions
- Branch naming: feature/TICKET-description
- Commit format: type(scope): message
- PR requires: tests, lint pass, 1 approval

## Forbidden
- Never modify /config/prod.json
- Never commit .env files
```

**Key Insight from HumanLayer:**
> "Frontier thinking LLMs can follow ~150-200 instructions with reasonable consistency."

Claude Code's system prompt already contains ~50 instructions. Keep CLAUDE.md focused.

**What to include:**
- WHY: Purpose of project, what components do
- WHAT: Tech stack, directory map, key files
- HOW: Build commands, test commands, verification steps

**What to exclude:**
- Database schema instructions (not relevant when doing UI work)
- Exhaustive style guides (use linters instead)
- Instructions that apply to <20% of tasks

### Skills Architecture

Skills are prompt injection via file loading. When Claude detects a matching description, it loads the SKILL.md.

**File Structure:**
```
~/.claude/skills/
└── code-review/
    ├── SKILL.md          # Required, core instructions
    ├── checklist.md      # Referenced file
    └── scripts/
        └── lint.sh       # Executable
```

**SKILL.md Format:**
```markdown
---
name: code-review
description: Review code for bugs and style. Use when asked to review code.
allowed-tools: Read, Grep, Glob    # Optional: restrict tools
---

# Code Review Skill

## Process
1. Read the file
2. Check against checklist.md
3. Report issues

## Output Format
- Category: [bug|style|perf]
- Severity: [low|medium|high]
- Location: file:line
- Issue: description
```

**Skills + Subagents:**
Subagents don't inherit skills. Must explicitly list:
```markdown
# .claude/agents/code-reviewer.md
---
name: code-reviewer
skills: pr-review, security-check
---
```

### elvis (@omarsar0) Workflow

**On Skills vs Subagents:**
> "Subagents are for handing off subtasks (separation of concerns). Skills are about loading context efficiently with a tiered system."

**Compounding effect:**
> "When stuck, use Claude Code + deep research subagent to read new tools, papers, ideas. Then brainstorm sessions and implement a skill with emerging ideas. Even if incomplete, pick them up later. The compounding effect of skills."

**On brainstorming:**
> "Interact with your coding agent for multiple turns before building. You'll know when it's 'just right' as the agent gets eager to implement."

---

## Hook Systems

### Hook Event Types

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
// .claude/settings.json
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

### Prompt-Type Hooks

Instead of shell scripts, hooks can use Claude itself:
```json
{
  "PreToolUse": [{
    "matcher": "Write|Edit",
    "hooks": [{
      "type": "prompt",
      "prompt": "Validate file write safety. Check: system paths, credentials, path traversal. Return 'approve' or 'deny'."
    }]
  }]
}
```

### Common Hook Patterns

**File Guard (PreToolUse):**
Block access to sensitive files (.env, credentials).

**Lint on Save (PostToolUse):**
Run linter after every Write/Edit.

**Test on Change (PostToolUse):**
Run tests for changed files.

**Completion Verification (Stop):**
Ensure tests pass before allowing stop.

**Context Injection (UserPromptSubmit):**
Load codebase map at session start.

**GitButler Integration (Pre/Post/Stop):**
Track changes per session into separate branches.

### TypeScript Hooks (johnlindquist/claude-hooks)

```typescript
async function preToolUse(payload: PreToolUsePayload): Promise<HookResponse> {
  if (payload.tool_name === 'Write' && payload.tool_input) {
    const { file_path } = payload.tool_input as WriteToolInput
    console.log(`Writing to: ${file_path}`)
  }
  return { action: 'continue' }
}
```

---

## Claude Code vs Codex CLI

### Fundamental Difference

**Claude Code:** Developer-in-the-loop, local workflow, terminal-native
**Codex CLI:** Both local and cloud-based, async task delegation, extensive pre-reading

### Feature Comparison

| Feature | Claude Code | Codex CLI |
|---------|-------------|-----------|
| Execution | Local-first | Local + Cloud sandboxes |
| Context | CLAUDE.md, Skills, Subagents | AGENTS.md |
| MCP Support | Native | stdio-based (recent) |
| Hooks | 8 event types | Limited |
| Permission System | 4 modes (default, AcceptEdits, etc.) | 3 tiers |
| Model | Claude 4.x (Sonnet/Opus) | GPT-5, o3 |
| Open Source | No | Yes |
| Pre-execution reading | Moderate | Extensive (10-15 min silent reads) |

### steipete's Perspective (Switched from Claude Code to Codex)

After extensive use of both, steipete documented why he switched:

**The reading behavior difference:**
> "Whatever OpenAI did in post-training, codex has been trained to read LOTS of code before starting. Sometimes it just silently reads files for 10, 15 minutes before starting to write any code. On the one hand that's annoying, on the other hand that's amazing because it greatly increases the chance that it fixes the right thing."

**The fix-the-fix problem:**
> "Opus on the other hand is much more eager - great for smaller edits - not so good for larger features or refactors, it often doesn't read the whole file or misses parts and then delivers inefficient outcomes or misses sth. I noticed that even tho codex sometimes takes 4x longer than Opus for comparable tasks, I'm often faster because I don't have to go back and fix the fix, sth that felt quite normal when I was still using Claude Code."

**Context efficiency:**
> "Codex is just FAR better at context management, I feel I get 5x more done on one codex session than with claude. This is more than just the objectively larger context size, there's other things at work. My guess is that codex internally thinks really condensed to save tokens, whereas Opus is very wordy."

**Session longevity:**
> "I used to be really diligent to restart a session for new tasks. With GPT 5.2 this is no longer needed. Performance is extremely good even when the context is fuller, and often it helps with speed since the model already has loaded plenty files."

**Knowledge cutoff advantage:**
> "Another massive win is the knowledge cutoff date. GPT 5.2 goes till end of August whereas Opus is stuck in mid-March - that's about 5 months."

### Model Selection Within Each Tool

**Claude Code users debate:**
- Opus 4.5: "Best coding model" (Boris Cherny), slower but less steering
- Sonnet 4: Faster, cheaper, good enough for most tasks

**Codex users (steipete):**
> "My go-to model is gpt-5.2-codex high. There's very little benefit to xhigh other than it being far slower, and I don't wanna spend time thinking about different modes."

### Practical Differences

**Determinism:**
> "Claude Code delivered consistent refactor plans across multiple runs. Codex required more oversight—re-running often produced alternative implementations."

**Speed vs Depth:**
> "Codex tends to reason longer, visible tokens-per-second feels faster. Claude reasons less, outputs slower, but more consistent."

**Token Usage (real test):**
- Claude Code (Figma clone): 6,232,242 tokens
- Codex (same task): 1,499,455 tokens
- Codex was 4x more token-efficient but less accurate

**C++ Optimization (Noam Brown poker solver):**
> "Codex's C++ version was 6x faster than Claude Code's (after multiple optimization prompts)."

### When to Use Which

**Claude Code:**
- Complex multi-file refactors
- Codebase requires deep context
- Need consistent, reviewable changes
- Local/private workflows
- Smaller edits where eagerness is good

**Codex CLI:**
- Large features or refactors
- Quick prototypes
- Cloud-based async tasks
- Already in OpenAI ecosystem
- Want open-source extensibility
- Tasks where extensive pre-reading pays off

**Hybrid (steipete's approach):**
> "I love Opus as general purpose model. My AI agent wouldn't be half as fun running on GPT 5. Opus has something special that makes it a delight to work with. I use it for most of my computer automation tasks."

Uses Codex for coding velocity, Claude/Opus for general agent work and personality.

### Interoperability

Some tools (claude-code-skill-factory) enable Claude Code ↔ Codex CLI interop via bridges.

### Codex Configuration (steipete's config)

```toml
model = "gpt-5.2-codex"
model_reasoning_effort = "high"
tool_output_token_limit = 25000
# Raise this - defaults are too small and fail silently
model_auto_compact_token_limit = 233000

[features]
ghost_commit = false
unified_exec = true
apply_patch_freeform = true
web_search_request = true
skills = true
shell_snapshot = true

[projects."/Users/steipete/Projects"]
trust_level = "trusted"
```

Key insight: Raising `tool_output_token_limit` lets the model read more in one go. Defaults are small and fail silently.

---

## Workflow Patterns

### steipete's "Inference Speed" Workflow

**Philosophy:** Ship at the speed of inference. Most software doesn't require hard thinking. Design for agents, not humans.

> "The amount of software I can create is now mostly limited by inference time and hard thinking. And let's be honest - most software does not require hard thinking."

**Stats:** Dozens of shipped open source tools, 3-8 parallel projects simultaneously.

**Core Principles:**

1. **Don't read code:**
> "These days I don't read much code anymore. I watch the stream and sometimes look at key parts, but I gotta be honest - most code I don't read. I do know where which components are and how things are structured and how the overall system is designed, and that's usually all that's needed."

2. **No plan mode:**
> "Plan mode feels like a hack that was necessary for older generations of models that were not great at adhering to prompts, so we had to take away their edit tools."

Instead: Start conversations naturally, explore, build plan collaboratively, then write "build" when ready.

3. **Commit to main:**
> "I simply commit to main. Sometimes codex decides that it's too messy and automatically creates a worktree and then merges changes back, but it's rare... I find the added cognitive load of having to think of different states in my projects unnecessary."

4. **Never revert:**
> "I basically never revert or use checkpointing. If something isn't how I like it, I ask the model to change it... Building software is like walking up a mountain. You don't go straight up, you circle around it."

5. **Queue, don't orchestrate:**
> "I extensively use the queueing feature of codex - as I get a new idea, I add it to the pipeline."

**Cross-Project Operations:**
```bash
# Reference other projects
"look at ../vibetunnel and do the same for Sparkle changelogs"

# Broadcast updates
"find all my recent go projects and implement this change there too + update changelog"
```

**Docs Over Sessions:**
> "I maintain docs for subsystems and features in a docs folder in each project, and use a script + some instructions in my global AGENTS file to force the model to read docs on certain topics."

**Multi-Machine Parallelism:**
Uses multiple Macs instead of worktrees:
> "I usually work on two Macs. My MacBook Pro on the big screen, and a Jump Desktop session to my Mac Studio on another screen. Some projects are cooking there, some here. Sometimes I edit different parts of the same project on each machine and sync via git. Simpler than worktrees because drifts on main are easy to reconcile."

**The Oracle Pattern (Escalation):**
Built a tool to escalate to stronger models when stuck:
> "oracle 🧿 - it's a CLI that allows the agent to run GPT 5 Pro and upload files + a prompt and manages sessions so answers can be retrieved later... The instructions are in my global AGENTS.MD file and the model sometimes by itself triggered oracle when it got stuck."

**Short Prompts + Images:**
> "With codex, my prompts gotten much shorter, I often type again, and many times I add images... If you show the model what's wrong, just a few words are enough to make it do what you want."

**Start with CLI:**
> "Whatever you build, start with the model and a CLI first. Agents can call it directly and verify output - closing the loop."

---

### Boris Cherny's Workflow (Claude Code Creator)

**Stats:** 259 PRs in 30 days, 497 commits, 40k lines added/38k removed—all via Claude Code.

**Parallel Execution:**
- 5-10 Claude instances simultaneously
- iTerm2 notifications for managing streams
- "Teleport" command to hand off between web and terminal

**Model:** Opus 4.5 with thinking for everything.

**CLAUDE.md Practice:**
> "Anytime we see Claude do something incorrectly we add it to CLAUDE.md, so Claude knows not to do it next time."

**Subagents:**
- code-simplifier: Clean up architecture after main work
- verify-app: Run e2e tests before shipping

**Verification Loop:**
> "Claude tests every single change using the Claude Chrome extension. Opens browser, tests UI, iterates until code works and UX feels good."

Claims 2-3x quality improvement from self-verification.

### Plan → Execute Pattern

1. **Plan Mode (Shift+Tab twice):** Claude analyzes but cannot modify files
2. **Write plan to plan.md:** Creates persistent artifact
3. **Review and adjust plan**
4. **Execute with plan as checklist**

> "Writing a plan to an external source and using it as a checklist is surprisingly powerful. When I come back days later, I'm not starting from scratch."

### Explore → Plan → Code → Commit

Anthropic's recommended flow:
1. Ask Claude to read files/URLs, explicitly say "don't code yet"
2. Use subagents for complex analysis
3. Confirm plan before implementation
4. Execute in focused chunks
5. Verify (lint, test) before commit

### Benchy Pattern (Parallel Same-Prompt)

Run N agents with identical prompt, pick best result.

**Rationale:** LLMs are non-deterministic. Same prompt produces different valid solutions.

**Implementation:**
```
project/
├── specs/
│   └── feature.md          # Shared spec
└── trees/
    ├── feature-1/          # Agent 1 worktree
    ├── feature-2/          # Agent 2 worktree
    └── feature-3/          # Agent 3 worktree
```

**Custom commands:**
- `/init-parallel`: Create worktrees from spec
- `/exe-parallel`: Launch agents with spec

**Result:** Compare implementations, cherry-pick best approach.

### GitHub Actions Background Agent

Run Claude Code in CI for:
- Monthly docs sync (read commits, update docs)
- Weekly code quality (review random dirs, auto-fix)
- Biweekly dependency audit

```bash
# Query agent logs to improve CLAUDE.md
query-claude-gha-logs --since 5d | claude -p "see what other claudes got stuck on, fix it, put up PR"
```

Creates feedback flywheel: Bugs → Improved CLAUDE.md → Better Agent

---

## Architecture Decisions

### What to Build vs Use

**Use existing tools for:**
- Git worktree management (worktrunk, claude-squad) - *if you need worktrees*
- TUI for multiple agents (claude-squad)
- Basic parallelism (parallel-cc)

**Build custom for:**
- Domain-specific task routing
- Integration with your issue tracker
- Custom verification loops
- Specialized subagents for your stack

**Consider not building (steipete's view):**
> "I see many folks experimenting with various systems of multi-agent orchestration, emails or automatic task management - so far I don't see much need for this - usually I'm the bottleneck."

Many tools compensate for model limitations. As models improve, tooling needs decrease.

### Two Philosophies

**The Orchestrator Approach (Boris Cherny):**
- Elaborate tooling, parallel agents
- Plan mode → auto-accept → verification
- Git worktrees for isolation
- CLAUDE.md constantly updated
- Verify everything

**The Factory Operator Approach (steipete):**
- Minimal tooling, maximum velocity
- No plan mode—natural conversation then "build"
- Commit to main, never revert
- Queue tasks, don't orchestrate
- Trust the model, don't read most code

These aren't evolution stages—they're different philosophies for different work types.

### Context Flow Design

**Single Agent:**
```
User → CLAUDE.md + Skills → Agent → Tools → Output
```

**Parallel Independent:**
```
User → Task Spec
         ├→ Worktree 1 → Agent 1 → Output 1
         ├→ Worktree 2 → Agent 2 → Output 2
         └→ Worktree 3 → Agent 3 → Output 3
              ↓
         Compare & Select
```

**Orchestrated Multi-Agent:**
```
User → Orchestrator (task decomposition)
         ├→ Planning Agent → Plan
         ├→ Impl Agent 1 → Code (uses Plan)
         ├→ Impl Agent 2 → Code (uses Plan)
         └→ Review Agent → Validation
              ↓
         Merge Coordinator
```

**Hierarchical with Subagents:**
```
Main Agent (8-15K context)
    ├→ Subagent: Explore (read-only, returns summary)
    ├→ Subagent: Test (isolated, returns pass/fail)
    └→ Subagent: Review (isolated, returns issues)
         ↓
    Main continues with clean context
```

**steipete's Escalation Pattern:**
```
Agent working on task
    ↓ (gets stuck)
Agent calls oracle CLI
    ↓
oracle uploads context to GPT 5 Pro
    ↓
Pro thinks hard (10-60 min)
    ↓
Answer returned to agent
    ↓
Agent continues with solution
```

### Prompt Organization

**By Location:**
- `CLAUDE.md` / `AGENTS.md` (root): Always loaded, project-wide
- `.claude/commands/`: Explicit invocation
- `~/.claude/skills/`: Personal, cross-project
- `.claude/skills/`: Project-specific
- `docs/` folder: Persistent knowledge (steipete's approach)

**By Trigger:**
- Auto-loaded: CLAUDE.md
- Description-matched: Skills
- Explicit: Commands (/command)
- Delegated: Subagents
- Script-forced: docs:list pattern (steipete)

### Parallelism Considerations

**How many agents?**
- Boris Cherny: 5-10 simultaneously
- Most tools: 3-5 practical limit
- steipete: 3-8 projects, but sequential tasks via queue
- Token cost is real constraint

**Isolation strategy:**
- Git worktrees: Standard, well-understood
- Containers/devcontainers: Full environment isolation
- Process-only: Same filesystem, tmux sessions (risky)
- Multiple machines: steipete's approach (separate Macs)

**Coordination:**
- GitHub Issues: Visible, persistent, team-accessible
- Local files: Fast, but siloed
- Database (ccswarm): Structured, queryable
- Simple queue: Codex's native feature (steipete prefers)

### Key Architectural Patterns from Tools

**Claude Squad:** Session state machine + tmux + worktrees. Simple, battle-tested.

**Worktrunk:** Git worktree UX wrapper. Lifecycle hooks for customization.

**CCPM:** GitHub as database. Human-visible progress. Slash commands for orchestration.

**CCSwarm:** Provider-agnostic. WebSocket communication. Template system for tasks.

**steipete's stack:** Codex queue + multiple machines + docs folders + oracle escalation. Minimal tooling.

---

## Open Questions / Areas for Exploration

1. **Intelligent task routing:** None of these tools do smart decomposition. All require human to define tasks.

2. **Cross-agent communication:** Agents don't talk to each other. Each works in isolation.

3. **Incremental merge strategies:** When agents touch overlapping code, merging is manual.

4. **Cost optimization:** Running 10 agents at Opus-level is expensive. No tools optimize for this.

5. **Verification standardization:** Every setup has custom verification. No standard "agent did the right thing" protocol.

6. **Context sharing:** Subagents don't inherit skills by default. Context boundaries are rigid.

7. **Long-running session management:** Stop hooks help, but no good "checkpoint and resume" pattern.

8. **Model escalation (oracle pattern):** steipete's oracle tool auto-escalates to GPT 5 Pro when stuck. This pattern is underexplored. Could be generalized: detect when agent is stuck → escalate to more capable model → return answer.

9. **Cross-project operations:** steipete frequently does "find all my projects with X and update them." Most tools don't support this. Multi-repo awareness is rare.

10. **Model capability vs tooling needs:** As models improve, some tools become unnecessary overhead. How do you build tools that remain valuable as models get better? steipete's switch from Claude Code to Codex reduced his tooling needs.

11. **Docs as persistent context vs session history:** steipete maintains docs folders and uses scripts to force doc reading. Others track session history. Which scales better? Do you need both?

12. **Reading code: necessary or optional?** steipete: "most code I don't read." If this becomes common, what tools support code-blind supervision?

---

## Tool Index

| Tool | Type | Language | Stars | Key Feature |
|------|------|----------|-------|-------------|
| claude-squad | TUI | Go | 1k+ | Multi-agent management |
| worktrunk | CLI | Rust | 80 | Worktree UX |
| code-conductor | CLI | Bash | - | GitHub-native orchestration |
| ccpm | Commands | MD | - | GitHub Issues as DB |
| ccswarm | CLI | Rust | - | Provider-agnostic orchestration |
| parallel-cc | CLI | Shell | - | Auto worktree on parallel |
| crystal | Desktop | - | - | GUI for parallel agents |
| claudekit | CLI | TS | - | Hooks + agents toolkit |
| claude-hooks | CLI | TS | - | TypeScript hook framework |
| oracle | CLI | TS | - | Model escalation (steipete) |

---

## Workflow Comparison: Boris vs steipete

| Aspect | Boris Cherny | steipete |
|--------|--------------|----------|
| Tool | Claude Code | Codex |
| Model | Opus 4.5 | GPT 5.2 codex high |
| Plan mode | Essential | "A hack for older models" |
| Parallel agents | 5-10 via iTerm | 3-8 projects via queue |
| Worktrees | Yes, for isolation | No, commit to main |
| Read code | Yes, review PRs | "Most code I don't read" |
| Revert/checkpoint | Sometimes | Never |
| Issue tracker | GitHub Issues | None ("nothing did stick") |
| Session management | Fresh sessions | Ride the context |
| Slash commands | Heavy use | Rarely use |
| Context strategy | CLAUDE.md + Skills | docs/ folders + scripts |
| Verification | Chrome extension, subagents | CLI-first, agent self-verify |

Both are highly productive. Different philosophies, different workflows.

---

---

## Related Documentation

For detailed API comparison and configuration reference, see:
- **CLAUDE_CODEX_API_REFERENCE.md** - Comprehensive API comparison covering CLAUDE.md vs AGENTS.md, Skills vs Subagents mechanics, SDK differences, and the Agent Skills open standard

---

*Compiled January 2026. Focus: practical patterns over hype.*
