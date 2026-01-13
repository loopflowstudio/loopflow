# Why a Prompt Organizer? The Case for LoopFlow

An analysis of gaps in the current Claude Code / Codex workflow ecosystem that a prompt organizer can fill.

---

## The Core Problem: Slash Commands Are Platform-Locked

Claude Code and Codex both have custom command systems:

```
Claude Code:  .claude/commands/my-command.md
Codex CLI:    ~/.codex/ or AGENTS.md inline
```

**The problem:** These are incompatible. A slash command written for Claude Code doesn't work in Codex. A skill written for one doesn't automatically work in the other.

**Current workarounds:**
- Maintain duplicate prompt files in both formats
- Manually copy-paste prompts between systems
- Give up and pick one platform

**What users actually want:**
- Write a prompt once
- Run it on Claude, Codex, or both simultaneously
- Compare results
- Pick the best implementation

---

## Gap #1: Cross-Platform Prompt Execution

### The Behavior Users Want

Simon Willison (October 2025):
> "My daily drivers are currently Claude Code (on Sonnet 4.5), Codex CLI (on GPT-5-Codex), and Codex Cloud... These are currently a mixture of Claude Code and Codex CLI."

steipete (October 2025):
> "I've completely moved to codex cli as daily driver... I experimented with worktrees, PRs but always revert back to this setup."

Dogukan Uraz Tuna (Medium, June 2025):
> "Give both AIs the same prompt... Choose the better implementation."

### What Exists Today

| Tool | Description | Limitation |
|------|-------------|------------|
| BLACKBOX Multi-Agent API | Run same task across Claude/Codex/Gemini | Cloud-only, no local CLI |
| async-code (GitHub) | Parallel Claude Code tasks with UI | Claude-only, not cross-platform |
| myclaude (GitHub) | Wrapper for Claude+Codex | Complex setup, dev-oriented |
| Duolingo CodingAgent | Internal library wrapping both SDKs | Proprietary, not public |
| Braintrust | A/B test prompts across models | API-level, not CLI agents |

### The Gap

**No tool exists that:**
1. Stores prompts in a platform-agnostic format
2. Dispatches the same prompt to both Claude Code and Codex CLI
3. Shows side-by-side results
4. Lets you pick the winner

**Value proposition for LoopFlow:**
- Single prompt library, multiple execution targets
- "Run on Claude" / "Run on Codex" / "Run on both" toggle
- Diff view for comparing implementations
- One-click to apply the winning result

---

## Gap #2: Git Worktree Orchestration Hell

### The Current Pain

From Feature Request #4963 on Claude Code (highly upvoted):

> "The current implementation is entirely manual and presents several significant problems:
> - **High Cognitive Load:** The developer must manually create worktrees, navigate between directories, and manage multiple terminal windows
> - **Manual Orchestration:** The user is forced to be the orchestrator of parallel work
> - **Clunky and Disjointed Workflow:** Switching between tasks, checking their status, and merging results is cumbersome"

From Takafumi Endo (Medium, June 2025):
> "The primary task of the developer shifts from the cognitive load of coding to the cognitive load of orchestration."

### steipete's Contrarian View

> "I tried the whole worktree setup, just slows me down... I simply commit to main."

He uses multiple Macs instead of worktrees. But most users don't have that luxury.

### What Users Actually Do

1. **Manual setup every time:**
   ```bash
   git worktree add ../project-feature-a feature-a
   cd ../project-feature-a
   claude
   # ...repeat for each task
   ```

2. **Lose track of what's where:**
   - Which worktree has which task?
   - What's the status of each?
   - Did I remember to copy .env?

3. **Merge manually:**
   - Compare outputs by eye
   - Cherry-pick changes
   - Clean up worktrees

### The Gap

**Current tools don't handle:**
- Automatic worktree creation with naming conventions
- Status dashboard across all active worktrees
- Environment sync (.env, dependencies)
- Unified merge/PR workflow
- Cleanup automation

**Value proposition for LoopFlow:**
- `loopflow start "implement auth" --parallel 3` → creates 3 worktrees, starts 3 agents
- Dashboard showing status of each
- "Compare results" view when done
- One-click to merge winner back to main
- Auto-cleanup of worktrees

---

## Gap #3: Commit Protocols and PR Sharing

### The Problem

When you run parallel agents:
- Each produces commits in their worktree/branch
- You need to compare those commits
- You want to share the "winning" implementation as a PR
- The losing branches should be cleaned up

### Current State

**GitButler approach (July 2025):**
> "With Claude Code hooks, you can make Claude tell GitButler when a file is about to be edited... We do a quick commit that stores the prompt used to generate that change."

This is clever but requires:
- GitButler installation
- Hook configuration
- Learning a new tool

**Standard workflow:**
```bash
# After agent finishes in worktree
cd ../project-feature-a
git add -A && git commit -m "feature: auth implementation"
gh pr create --title "Auth feature" --body "..."
# Repeat for each worktree, compare PRs
```

### The Gap

**No tool automates:**
- Consistent commit message format across parallel runs
- PR creation with diff comparison
- "This PR was generated by Claude Code at [timestamp] using prompt: [link to prompt]"
- Audit trail: which prompt, which model, which settings

**Value proposition for LoopFlow:**
- Every prompt execution creates a commit with metadata
- Commit message includes: prompt hash, model used, execution time
- "Create PR" button that includes:
  - The original prompt
  - Model/settings used
  - Link to compare with other implementations
- Shareable prompt URLs for reproducibility

---

## Gap #4: Running Same Exact Task on Both Platforms

### Why This Matters

Different models have different strengths (from our research):

| Model | Strength | Weakness |
|-------|----------|----------|
| Claude (Opus) | Deep reasoning, refactoring | Sometimes over-eager, misses parts |
| Codex (GPT-5.2) | Reads extensively before writing, fewer "fix-the-fix" loops | Slower, more literal |
| Gemini | Fast, multimodal | Edit tools are messy |

**steipete's experience:**
> "Whatever OpenAI did in post-training, codex has been trained to read LOTS of code before starting... I noticed that even tho codex sometimes takes 4x longer than Opus for comparable tasks, I'm often faster because I don't have to go back and fix the fix."

### The Ideal Workflow

1. Define task once: "Implement JWT authentication for the auth service"
2. Click "Run on Claude + Codex"
3. Both agents work in parallel (separate worktrees or environments)
4. See side-by-side comparison when both finish
5. Pick the better implementation
6. Optionally: merge best parts from each

### What Exists

- **Cursor v2.0 concept** (from feature request #10599): "Run multiple agents simultaneously on a single prompt, each in an isolated environment"
- **BLACKBOX API**: Has multi-agent support but cloud-only
- **Manual approach**: Start Claude in one terminal, Codex in another, compare by eye

### The Gap

**No CLI-native tool provides:**
- Unified prompt format that works on both
- Parallel dispatch to both agents
- Synchronized completion detection
- Structured comparison of results

**Value proposition for LoopFlow:**
- Prompt written once in LoopFlow format
- `loopflow run --agents claude,codex "implement feature X"`
- Wait for both to complete
- Diff view showing:
  - Files changed by each
  - Token usage by each
  - Time taken by each
- "Apply Claude's changes" / "Apply Codex's changes" / "Merge manually"

---

## Gap #5: Prompt Versioning and Reproducibility

### The Problem with Slash Commands

Slash commands are just markdown files. They don't have:
- Version history (beyond git)
- A/B testing infrastructure
- Execution logs
- Performance metrics

From paddo.dev (November 2025):
> "You version a foldered capability whose core is SKILL.md plus optional scripts/resources. This encourages documentation and modularity, but artifact-level diffs are only as clear as your repo structure and commit hygiene."

### What Enterprise Users Need

1. **Audit trail:** Which prompt produced which code change?
2. **Rollback:** Previous prompt version worked better, how do I get it back?
3. **A/B testing:** Is prompt v2 actually better than v1?
4. **Team sharing:** How do I share a prompt with the team with confidence it'll work?

### What Exists

- **Braintrust/LangSmith/PromptLayer:** Great for API prompts, not CLI agents
- **Git:** Version control works but no prompt-specific features
- **Skills directory:** Anthropic has one, but no versioning UI

### The Gap

**Value proposition for LoopFlow:**
- Every prompt has a version number and changelog
- Execution history: "This prompt was run 47 times, avg success rate 82%"
- A/B testing: "v2 completed 30% faster with same quality"
- Export to Claude commands / Codex format with one click
- Import from Claude commands / Codex format
- Team sync: shared prompt library with access controls

---

## Summary: The LoopFlow Value Stack

| Layer | Current Pain | LoopFlow Solution |
|-------|-------------|-------------------|
| **Prompt Storage** | Platform-specific formats (.claude/commands, AGENTS.md) | Universal format, export to any platform |
| **Execution** | One agent at a time, manual switching | Parallel dispatch to Claude + Codex |
| **Worktree Management** | Manual create/navigate/cleanup | Automated orchestration with dashboard |
| **Comparison** | Side-by-side terminals, manual diff | Structured diff view, metrics comparison |
| **Commit/PR** | Manual commits, no audit trail | Metadata-rich commits, one-click PR |
| **Versioning** | Git only, no prompt-specific features | Version history, A/B testing, metrics |
| **Sharing** | Copy files, hope they work | Shareable URLs, reproducibility guaranteed |

---

## What the Power Users Say They Want

**Simon Willison:**
> "I'm still settling into patterns that work for me. I imagine I'll be iterating on my processes for a long time to come."

**Feature Request #4963:**
> "True Agentic Parallelism: Empowers developers to delegate multiple complex, long-running tasks without blocking their own interactive workflow."

**Feature Request #10599:**
> "Run multiple agents simultaneously on a single prompt, each in an isolated environment... a unified diff view opens... The developer analyzes both solutions."

**steipete:**
> "I tried so many [wrapper tools]. None stick. IMO they work around current inefficiencies and promote a workflow that just isn't optimal."

The key insight from steipete: **Don't build a thin wrapper. Build something that enables a genuinely better workflow.**

---

## Strategic Positioning

LoopFlow should NOT be:
- Another Claude Code wrapper
- A worktree management tool (branchyard exists)
- A prompt template library (Anthropic has one)

LoopFlow SHOULD be:
- **The prompt layer that sits above Claude/Codex**
- **The orchestrator for cross-platform prompt execution**
- **The comparison engine for picking the best AI output**
- **The audit trail for prompt-to-code traceability**

The moat: **prompt portability + parallel execution + structured comparison**

Nobody else is doing all three.

---

*Research compiled January 2026*
