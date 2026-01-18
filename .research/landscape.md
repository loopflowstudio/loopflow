# AI Coding Agent Landscape

The bottleneck has shifted from writing code to managing agents. This research maps the current state: workflows, tools, and unsolved problems.

---

## Points of Consensus

1. **Persistent context files** are universal—debate is what to put in them, not whether to have one
2. **Context management is the hard problem**—compaction destroys context, models forget mid-session
3. **Verification loops are required**—"trust but verify" is universal
4. **Hooks beat prompting** for deterministic control
5. **CLI first**—"start with the model and a CLI first" (steipete)
6. **Design for agents, not humans**—"I engineer codebases so agents can work efficiently" (steipete)

## Active Debates

**Plan Mode**

Boris Cherny: "Most sessions start in Plan mode... A good plan is really important!"

steipete: "Plan mode feels like a hack that was necessary for older generations of models." He starts conversations naturally, then writes "build" when ready.

**Git Worktrees**

Orchestration tools treat worktrees as foundational. steipete commits to main: "I find the added cognitive load unnecessary." Uses multiple Macs instead of worktrees.

**Multi-Agent Orchestration**

steipete: "I see many folks experimenting with various systems... so far I don't see much need—usually I'm the bottleneck." Uses simple queueing instead.

---

## User Segments

### Factory Operators
Ship at inference speed. 3-8 projects simultaneously. Don't read most code. Minimal tooling, maximum velocity. Commit to main, queue tasks, short prompts with images. "Most software does not require hard thinking." (steipete)

### Orchestrators
5-10+ parallel agents. Plan mode → auto-accept → verification loop. Elaborate CLAUDE.md, worktrees, iTerm notifications. Pain: rate limits, subscription costs. (Boris Cherny, Anthropic teams)

### Surgeons
IDE (Cursor/Copilot) for daily work, Claude Code for hard problems—complex refactors, architecture, debugging. Single agent at a time.

### Vibers
Want AI to "just work." Default settings, single prompt, frustrated by configuration and limits.

### Enterprisers
Team leads deploying across orgs. Centralized CLAUDE.md governance, security hooks, compliance APIs, spend caps.

---

## Pain Points (ranked by frequency)

| Pain | Detail |
|------|--------|
| **Usage limits** | #1 complaint. Hit limits in 10-15 min on $200/mo plans. No visibility into calculations. |
| **Context loss** | Auto-compacting destroys project goals, earlier instructions, architectural decisions |
| **File clutter** | Scratch files, markdown notes, temp outputs pollute repos |
| **Large codebases** | Performance degrades >1,000 lines. Manual context selection required. |
| **Bad architecture** | Code works but creates downstream problems. ~30% first-try reliability. |
| **MCP friction** | Too many manual steps to connect external services |

---

## Existing Tools

### Multi-Agent Orchestration

| Tool | What it does |
|------|--------------|
| **Claude Squad** | TUI for multiple agents. tmux + worktrees. Go/BubbleTea. |
| **Worktrunk** | Worktree management. `wt switch -c -x claude feat` |
| **Code Conductor** | GitHub-native. Agents claim tasks from Issues. |
| **CCPM** | GitHub Issues as task DB. `/pm:epic-start` slash commands. |
| **Conductor** | macOS native parallel agents with oversight |

### Context Switching

**Notifications:** Terminal bell, Claude Code hooks (most flexible), terminal-notifier, Pushcut (iOS)

**Window management:** Rectangle, Peacock (VS Code colors), Ghostty, tmux

### The Gap

What people want: launch agents → background processing → smart notifications → quick switch → compare results → cleanup.

| Need | Current State | Pain |
|------|---------------|------|
| Notification routing | Broadcasts everywhere | Check each terminal |
| Task identification | Title says "claude" | Can't tell which task |
| Status dashboard | None | Manual checking |
| Quick switch | Keyboard shortcuts | Remember which pane |

---

## Market Dynamics

**Top 3 capture 70%+ of $4B market:** GitHub Copilot, Claude Code, Cursor. All crossed $1B ARR.

**Consolidation:** Anthropic acquired Bun. Anysphere acquired Graphite. OpenAI acquired Windsurf.

### Likely Outcomes

1. **Claude Code absorbs orchestration**—standalone tools get obsoleted
2. **Verification becomes a layer**—sits on top of any agent
3. **Enterprise stays fragmented**—compliance requirements vary too much
4. **Context management unsolved**—hardest problem, biggest advantage

### Opportunity

**Build:** Enterprise compliance, domain-specific verification, training/onboarding, internal tool integration

**Don't build:** Basic orchestration (absorbed), generic TUI wrappers, simple slash command libraries

---

## Takeaways for Loopflow

1. **Solve one problem well** — `lf` is for prompt/context construction. Like worktrunk for worktrees.
2. **Context is the hard problem** — better context management = competitive advantage
3. **Don't compete with terminals** — Warp, Ghostty, iTerm are great. Let people use what they like.
4. **Prompts as artifacts** — the gap is reusable, versioned prompts. Own that.
5. **CLI first, GUI later** — "start with the model and a CLI first" (steipete). Maestro comes after CLI is solid.
6. **Build carefully** — Claude Code changes weekly. Stay portable, don't depend on internals.

---

*January 2026*
