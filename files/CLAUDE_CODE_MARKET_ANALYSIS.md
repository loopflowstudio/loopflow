# Claude Code Ecosystem: Market Analysis

A strategic analysis of the market forming around Claude Code tooling, examining consensus, disagreements, user segments, pain points, and build-vs-buy decisions.

---

## Table of Contents

1. [Points of Consensus](#points-of-consensus)
2. [Points of Disagreement](#points-of-disagreement)
3. [User Segments & Behavioral Clusters](#user-segments--behavioral-clusters)
4. [Primary Pain Points](#primary-pain-points)
5. [Winner-Take-All Feature Set](#winner-take-all-feature-set)
6. [Build vs Buy Analysis](#build-vs-buy-analysis)
7. [Market Dynamics](#market-dynamics)

---

## Points of Consensus

### Everyone Agrees On These

**1. A persistent context file is essential**

The pattern of CLAUDE.md (or AGENTS.md for Codex) that the agent reads at session start is universal. Nobody argues against it. The debate is only about what to put in it, not whether to have one.

**2. Context management is the hard problem**

Everyone agrees that:
- Auto-compacting destroys important context
- Context "rot" (degraded performance as context fills) is real
- The model forgets things mid-session
- Better models help but don't solve this

**3. Verification loops are necessary for quality**

Whether it's running tests, using a Chrome extension to visually verify UI, or having a review subagent check work—the consensus is that agents without verification produce unreliable output. "Trust but verify" is universal.

**4. Hooks provide deterministic control that prompting cannot**

When you need something to always happen (format code, block dangerous commands, run linters), hooks are preferred over hoping the model remembers instructions.

**5. Start with CLI, add UI later**

steipete: "Whatever you build, start with the model and a CLI first." Boris Cherny and others echo this. CLIs let agents verify their own output, closing the feedback loop.

**6. Language/ecosystem choice matters more than before**

steipete: "The important decisions these days are language/ecosystem and dependencies." With agents writing most code, the choice of what they write in becomes the key architectural decision.

**7. Design codebases for agents, not humans**

steipete: "I don't design codebases to be easy to navigate for me, I engineer them so agents can work in it efficiently." This represents a philosophical shift in how to think about code organization.

---

## Points of Disagreement

### Active Debates in the Community

**1. Plan Mode: Essential or Obsolete Hack?**

Boris Cherny: "Most sessions start in Plan mode... A good plan is really important!"

steipete (strong dissent): "Plan mode feels like a hack that was necessary for older generations of models that were not great at adhering to prompts, so we had to take away their edit tools." He instead starts conversations naturally, explores code, builds a plan collaboratively, then writes "build" when ready.

This isn't a minor disagreement—it's a fundamental split on whether explicit plan mode is a feature or a crutch.

**2. Git Worktrees: Essential Infrastructure or Unnecessary Complexity?**

Orchestration tools: Git worktrees are the foundation of parallel agent work.

steipete: "I simply commit to main. Sometimes codex decides that it's too messy and automatically creates a worktree and then merges changes back, but it's rare... I find the added cognitive load of having to think of different states in my projects unnecessary and prefer to evolve it linearly."

He uses multiple Macs instead: "I usually work on two Macs... Sometimes I edit different parts of the same project on each machine and sync via git. Simpler than worktrees because drifts on main are easy to reconcile."

**3. Claude Code vs Codex: Which Agent Runtime?**

Claude Code camp: Terminal-native, great at complex refactoring, 200K context, mature skills/hooks system.

steipete (switched to Codex): "Whatever OpenAI did in post-training, codex has been trained to read LOTS of code before starting. Sometimes it just silently reads files for 10, 15 minutes before starting to write any code... I noticed that even tho codex sometimes takes 4x longer than Opus for comparable tasks, I'm often faster because I don't have to go back and fix the fix, sth that felt quite normal when I was still using Claude Code."

He reports getting "5x more done on one codex session than with claude" due to better context management.

**4. Opus vs Sonnet vs GPT 5.2 (Cost vs Quality vs Behavior)**

Boris Cherny: "I use Opus 4.5 with thinking for everything."

steipete: "GPT 5.2 one-shots almost anything I throw at it... Opus on the other hand is much more eager - great for smaller edits - not so good for larger features or refactors, it often doesn't read the whole file or misses parts."

He still uses Opus for non-coding: "I love Opus as general purpose model. My AI agent wouldn't be half as fun running on GPT 5."

**5. Reading Code: Necessary or Optional?**

Traditional view: Developers should review AI-generated code carefully.

steipete: "These days I don't read much code anymore. I watch the stream and sometimes look at key parts, but I gotta be honest - most code I don't read. I do know where which components are and how things are structured and how the overall system is designed, and that's usually all that's needed."

This is controversial but represents where power users are heading.

**6. Issue Trackers & Task Management: Structured or Ad-Hoc?**

Orchestration tools (code-conductor, ccpm): GitHub Issues as task queue, structured task management.

steipete: "I tried linear or other issue trackers, but nothing did stick. Important ideas I try right away, and everything else I'll either remember or it wasn't important... when I find a bug, I'll immediately prompt it - much faster than writing it down."

**7. Multi-Agent Orchestration: Sophisticated Systems or Simple Queuing?**

Community tools: Build elaborate orchestration for parallel agents.

steipete: "I see many folks experimenting with various systems of multi-agent orchestration, emails or automatic task management - so far I don't see much need for this - usually I'm the bottleneck." He uses Codex's simple queueing feature instead: "as I get a new idea, I add it to the pipeline."

**8. Session Management: Fresh Sessions or Ride the Context?**

Claude Code practice: Start fresh sessions for new tasks to avoid context pollution.

steipete: "I used to be really diligent to restart a session for new tasks. With GPT 5.2 this is no longer needed. Performance is extremely good even when the context is fuller, and often it helps with speed since the model already has loaded plenty files."

**9. Reverting & Checkpointing: Safety Net or Crutch?**

Cursor: Checkpointing is a key feature for complex work.

steipete: "I basically never revert or use checkpointing. If something isn't how I like it, I ask the model to change it... Building software is like walking up a mountain. You don't go straight up, you circle around it and take turns."

**10. Prompt Length: Elaborate or Minimal?**

Earlier practice: Long, detailed prompts with voice dictation.

steipete: "With codex, my prompts gotten much shorter, I often type again, and many times I add images... If you show the model what's wrong, just a few words are enough."

---

## User Segments & Behavioral Clusters

### Cluster 1: The Factory Operators
**Profile:** Ship at "inference speed." Run 3-8 projects simultaneously. Don't read most code. Design systems for agents, not humans. Minimal tooling, maximum velocity.

**Representative:** Peter Steinberger (steipete)

**Workflow:**
- Multiple projects cooking simultaneously across multiple machines
- Commit directly to main, never revert
- Queue tasks instead of parallel orchestration
- Short prompts, often with images
- Don't use plan mode—have natural conversations, then say "build"
- Maintain docs folders instead of session history
- Cross-reference between projects liberally

**Key insight:** "The amount of software I can create is now mostly limited by inference time and hard thinking. And let's be honest - most software does not require hard thinking."

**Tools:** Codex (switched from Claude Code), simple queuing, multiple Macs

**Pain points:** Inference speed, model knowledge cutoff dates, dependency selection

**Philosophy:** "I don't design codebases to be easy to navigate for me, I engineer them so agents can work in it efficiently."

### Cluster 2: The Orchestrators
**Profile:** Run 5-10+ agents in parallel. Heavy slash command users. Maintain elaborate CLAUDE.md. Use subagents for verification.

**Representative:** Boris Cherny, teams at Anthropic

**Workflow:**
- Plan mode → auto-accept mode → verification loop
- iTerm notifications to track multiple agents
- Shared CLAUDE.md across team, constantly updated
- Tools wired into Slack, BigQuery, Sentry
- Git worktrees for isolation

**Pain points:** Rate limits, subscription costs, managing multiple sessions

**Spend:** Max subscription + extra usage

### Cluster 3: The Surgeons
**Profile:** Use Claude Code for specific hard problems. Most work in IDE (Cursor/Copilot). Pull out Claude Code for complex refactors, architecture decisions, debugging.

**Workflow:**
- Daily work in Cursor
- Switch to Claude Code for "senior engineer" problems
- Often use Claude for planning, Codex for review
- Single agent at a time

**Pain points:** Context switching between tools, inconsistent behavior

**Spend:** Pro subscription, rarely hit limits

### Cluster 4: The Vibers
**Profile:** Newer to agentic coding. Want AI to "just work." Less configuration, more magic.

**Workflow:**
- Default settings
- Single prompt → hope for good output
- Frustrated by permissions, configuration, limits

**Pain points:** Hitting limits unexpectedly, context loss, generic outputs

**Spend:** Pro subscription, constantly evaluating alternatives

### Cluster 5: The Enterprisers
**Profile:** Team leads or architects deploying Claude Code across organizations. Care about compliance, audit trails, cost control.

**Workflow:**
- Centralized CLAUDE.md governance
- Hooks for security scanning
- Compliance API integration
- Seat management, spend caps

**Pain points:** Lack of visibility into what agents are doing, unpredictable costs, audit requirements

**Spend:** Enterprise plans ($100-200+/seat/month)

### Cluster 6: The Non-Coders
**Profile:** Marketers, lawyers, designers using Claude Code to build things beyond their traditional skill set.

**Representative:** Anthropic's Growth Marketing team (Figma plugin, ad variation generation)

**Workflow:**
- Natural language descriptions → working tools
- Don't read/understand generated code
- Care about output, not process

**Pain points:** When things break, they can't debug. Black box.

**Spend:** Whatever their org provides

---

## Primary Pain Points

### Ranked by Frequency of Complaint

**1. Usage Limits & Unpredictable Throttling**

The #1 complaint. Users report:
- Hitting limits in 10-15 minutes on $200/month Max plans
- ~60% reduction in effective limits over time
- No visibility into actual limit calculations
- Silent tightening without notice

> "I get anxious as I get close to the limit, because I know the odds of the thing going completely off the rails go up tremendously."

**2. Context Loss & Compaction**

Auto-compacting "obliterates important context." Users lose:
- Project goals mid-session
- Earlier instructions
- Established architectural decisions

> "Context drift where Claude loses track of project goals mid-session"

**3. Intermediate File Mess**

Claude produces scratch files, markdown notes, temporary outputs that clutter repositories.

> "It tends to produce all these interim markdown files and scratch pad output... makes it impossible to inspect directories or repositories manually."

**4. Large Codebase Struggles**

Performance degrades on modules >1,000 lines. Complex projects require extensive manual context selection.

> "Claude Code can't handle complex codebases" - common complaint

**5. Poor Architectural Decisions**

Code works but creates downstream problems. Users report only ~30% reliability on first try due to architectural issues, not syntax errors.

> "It's biased towards its training data, so it might suggest older tech stacks instead of modern best practices."

**6. MCP Server Setup Friction**

Connecting to external services (transcripts, APIs, tools) requires too many manual steps.

> "Connecting to remote MCP servers is still a headache... way too many setup steps and creates a lot of user friction."

**7. Skill/Hook Controllability (Improving)**

Skills auto-invoke when you don't want them, or don't invoke when you do. No explicit invocation (until v2.1). No disable flag.

**8. Permission System Friction**

Users either:
- Constantly click "allow" (annoying)
- Use `--dangerously-skip-permissions` (risky)

No good middle ground until `/permissions` presets.

---

## Winner-Take-All Feature Set

If a single dominant platform emerges, here's what it would need to consolidate all the current fragmented tools:

### Core Infrastructure (Must Have)

| Feature | Current Best | Why Essential |
|---------|--------------|---------------|
| Git worktree management | Worktrunk | Foundation of all parallel work |
| Multi-agent TUI | Claude Squad | Visibility into concurrent agents |
| Session persistence | Claude Squad | Don't lose work when terminal closes |
| CLAUDE.md management | Native | Needs versioning, inheritance, scoping |
| Slash command system | Native | Workflow codification |
| Hook system | Native | Deterministic control |

### Orchestration Layer (Differentiator)

| Feature | Current State | Winner Feature |
|---------|---------------|----------------|
| Task decomposition | Manual | Automatic analysis of work parallelizability |
| Agent routing | Manual | Smart assignment based on task type |
| Cross-agent communication | None | Agents share discoveries without human mediation |
| Conflict detection | Manual merge | Real-time detection of overlapping work |
| Cost optimization | None | Route simple tasks to cheaper models |

### Context Management (Biggest Gap)

| Feature | Current State | Winner Feature |
|---------|---------------|----------------|
| Context visualization | None | See what's in context, what was dropped |
| Selective compaction | Auto-compact only | User controls what to preserve |
| Context transfer | Manual | Move relevant context between sessions |
| Knowledge base | CLAUDE.md | Searchable, versioned project knowledge |
| Progressive disclosure | Skills | Better auto-selection of relevant context |

### Verification & Quality (Trust)

| Feature | Current State | Winner Feature |
|---------|---------------|----------------|
| Automated testing | Manual hooks | Built-in test-after-change |
| Visual verification | Chrome extension | Native browser automation for UI testing |
| Code review | Separate tool (Codex, Graphite) | Integrated review before commit |
| Architectural validation | None | Check for known antipatterns |
| Regression detection | CI | Immediate feedback, not async |

### Enterprise & Team (Scale)

| Feature | Current State | Winner Feature |
|---------|---------------|----------------|
| Shared context | Commit CLAUDE.md | Real-time team knowledge sync |
| Usage analytics | Third-party tools | Native dashboards per user/project |
| Audit trails | Compliance API | Every agent action logged, searchable |
| Cost allocation | Per-seat | Per-project, per-task attribution |
| Policy enforcement | Manual hooks | Centralized security rules |

### UX Polish (Retention)

| Feature | Current State | Winner Feature |
|---------|---------------|----------------|
| Session naming | Manual | Auto-suggest based on work |
| History search | /resume picker | Full-text search of past sessions |
| Notifications | iTerm integration | Cross-platform, cross-device |
| Mobile control | Claude iOS app | Start/monitor tasks from phone |
| IDE integration | Separate tools | Deep integration without leaving terminal |

---

## Build vs Buy Analysis

### What Every Company Will Rebuild (Undifferentiated)

These are table stakes that don't provide competitive advantage but must exist:

| Component | Why Rebuild | Complexity |
|-----------|-------------|------------|
| Git worktree wrapper | Everyone needs isolation | Low |
| Basic slash commands | Specific to company workflow | Low |
| CLAUDE.md templates | Company-specific conventions | Low |
| Permission presets | Security requirements vary | Low |
| Simple hooks (linting, formatting) | Stack-specific | Medium |

**Cost to rebuild:** Days to weeks per company
**Should be outsourced?** Probably not—too company-specific

### What Companies Will Buy/Outsource (High Value, Undifferentiated)

| Component | Why Buy | Current Provider |
|-----------|---------|------------------|
| Multi-agent TUI | Hard to build well, not core competency | Claude Squad |
| Worktree lifecycle management | Annoying edge cases | Worktrunk |
| Usage monitoring/cost tracking | Necessary but boring | ccusage, third-party |
| MCP servers for common services | No reason to rewrite GitHub/Slack integration | Community |

**These will likely consolidate into the platform itself** once Claude Code matures.

### What Requires Building In-House (Differentiated)

| Component | Why Build | Complexity |
|-----------|-----------|------------|
| Task decomposition logic | Company understands own codebase | High |
| Verification procedures | Domain-specific correctness criteria | High |
| Integration with internal tools | Proprietary systems | Medium |
| Security/compliance hooks | Regulatory requirements | Medium |
| Custom subagents | Encode company-specific expertise | Medium |

**These are where competitive advantage lives.**

### Platform Opportunities (Winner Take All)

| Feature | Current Gap | Market Size |
|---------|-------------|-------------|
| Context visualization & control | Everyone wants this, no one has it | All Claude Code users |
| Cross-session knowledge persistence | Memory helps but isn't enough | All users |
| Intelligent task routing | Manual is tedious | Power users (Orchestrators) |
| Built-in verification framework | Everyone implements their own | Everyone |
| Team collaboration features | Currently awkward | Enterprise |

---

## Case Study: steipete's "Inference Speed" Workflow

Peter Steinberger's workflow deserves dedicated analysis because it represents a mature, high-velocity approach that challenges many community assumptions. He ships prolifically (dozens of open source tools) and has documented his evolution from Claude Code to Codex.

### Core Philosophy

> "The amount of software I can create is now mostly limited by inference time and hard thinking. And let's be honest - most software does not require hard thinking. Most apps shove data from one form to another, maybe store it somewhere, and then show it to the user in some way or another."

This reframes agentic coding not as "AI assistance" but as a production line where the human is a supervisor, not a co-author.

### Why He Switched from Claude Code to Codex

**The reading problem:** "Whatever OpenAI did in post-training, codex has been trained to read LOTS of code before starting. Sometimes it just silently reads files for 10, 15 minutes before starting to write any code."

**The fix-the-fix problem:** "I noticed that even tho codex sometimes takes 4x longer than Opus for comparable tasks, I'm often faster because I don't have to go back and fix the fix, sth that felt quite normal when I was still using Claude Code."

**Context efficiency:** "Codex is just FAR better at context management, I feel I get 5x more done on one codex session than with claude. This is more than just the objectively larger context size, there's other things at work. My guess is that codex internally thinks really condensed to save tokens, whereas Opus is very wordy."

### The Oracle Pattern

When agents get stuck, steipete built a tool to escalate:

> "I built oracle 🧿 - it's a CLI that allows the agent to run GPT 5 Pro and upload files + a prompt and manages sessions so answers can be retrieved later. I did this because many times when agents were stuck, I asked it to write everything into a markdown file and then did the query myself... The instructions are in my global AGENTS.MD file and the model sometimes by itself triggered oracle when it got stuck."

This is **automated escalation to a more capable model**—a pattern that could be generalized.

### Specific Workflow Patterns

**1. Multi-project parallelism via machines, not worktrees:**
> "I usually work on two Macs. My MacBook Pro on the big screen, and a Jump Desktop session to my Mac Studio on another screen. Some projects are cooking there, some here. Sometimes I edit different parts of the same project on each machine and sync via git. Simpler than worktrees because drifts on main are easy to reconcile."

**2. Queuing over orchestration:**
> "I extensively use the queueing feature of codex - as I get a new idea, I add it to the pipeline."

**3. Cross-project learning:**
> "I cross-reference projects all the time, esp if I know that I already solved sth somewhere else, I ask codex to look in ../project-folder... I can just write 'look at ../vibetunnel and do the same for Sparkle changelogs'."

**4. Docs over sessions:**
> "I've seen plenty of systems for folks wanting to refer to past sessions. Another thing I never need or use. I maintain docs for subsystems and features in a docs folder in each project."

**5. Broadcast updates:**
> "Often I let an agent simply run in my project folder and when I figure out a new pattern, I ask it to 'find all my recent go projects and implement this change there too + update changelog'."

### Anti-Patterns He Avoids

- **Plan mode:** "Plan mode feels like a hack that was necessary for older generations of models"
- **Reverting:** "I basically never revert or use checkpointing"
- **Worktrees:** "I find the added cognitive load of having to think of different states in my projects unnecessary"
- **Issue trackers:** "I tried linear or other issue trackers, but nothing did stick"
- **Slash commands:** "I used to play with slash commands, but just never found them too useful"
- **Fresh sessions:** "I used to be really diligent to restart a session for new tasks. With GPT 5.2 this is no longer needed"

### His Codex Config

```toml
model = "gpt-5.2-codex"
model_reasoning_effort = "high"
tool_output_token_limit = 25000
model_auto_compact_token_limit = 233000

[features]
ghost_commit = false
unified_exec = true
apply_patch_freeform = true
web_search_request = true
skills = true
shell_snapshot = true
```

Key insight: He raises `tool_output_token_limit` to let the model read more in one go—defaults are too small and fail silently.

### What This Means for Tool Builders

steipete's workflow suggests:

1. **Model capability reduces tooling needs:** Many tools exist to compensate for model limitations. As models improve, these tools become unnecessary overhead.

2. **Simple beats sophisticated:** Queueing beats orchestration. Multiple machines beats worktrees. Docs folders beat session history.

3. **The bottleneck is human, not system:** "Usually I'm the bottleneck" for multi-agent orchestration. Tools that assume the system is the bottleneck may be solving the wrong problem.

4. **Escalation is valuable:** The oracle pattern (auto-escalate to stronger model when stuck) is underexplored.

5. **Cross-project operations matter:** "Find all my projects with X and update them" is a common pattern that most tools don't support.

---

## Market Dynamics

### Current Market Structure

**Top 3 capture 70%+ of $4B market:**
- GitHub Copilot (Microsoft)
- Claude Code (Anthropic)
- Cursor (Anysphere)

All three have crossed $1B ARR.

**Consolidation signals:**
- Anthropic acquired Bun (Claude Code dependency)
- Anysphere acquired Graphite (code review)
- OpenAI acquired Windsurf (Codeium)

### What This Means for Builders

**Build on shifting sand:** Claude Code's feature set changes weekly. Oikon tracked 176 changelog updates in 2025. Tools that wrap Claude Code risk being obsoleted by native features.

**But platform gaps are real:** Usage limits, context management, parallelism—these aren't being solved fast enough. Opportunity exists.

**Enterprise is underserved:** Team coordination, audit trails, cost allocation—Claude Code's enterprise features are new and shallow.

### Likely Outcomes

**1. Claude Code absorbs most orchestration**

Worktrunk, claude-squad, and similar tools solve problems Claude Code should solve natively. Anthropic will likely build these features, obsoleting standalone tools.

**2. Verification/review becomes a layer**

Graphite (acquired by Anysphere) and similar tools suggest code review is a distinct layer that sits on top of any agent. This may consolidate rather than fragment.

**3. Enterprise tooling remains fragmented**

Compliance, audit, and governance requirements vary enough that no single tool wins. Expect a long tail of enterprise-specific integrations.

**4. Context management remains unsolved**

This is the hardest technical problem. Whoever solves it (better compaction, true long-term memory, knowledge graphs) wins a significant advantage.

### Where to Focus

**High probability of value:**
- Enterprise compliance/audit tooling
- Domain-specific verification (healthcare, finance, etc.)
- Training/onboarding for agentic workflows
- Integration with company-specific internal tools

**High risk of obsolescence:**
- Basic orchestration (Claude Code will absorb this)
- Generic TUI wrappers
- Simple slash command libraries
- MCP servers for common services (GitHub, Slack, etc.)

**Uncertain but high potential:**
- Intelligent task decomposition
- Cross-agent memory/communication
- Hybrid cloud/local agent orchestration
- Cost optimization across models/providers

---

## Summary: The Strategic Picture

**What's actually settled:**
- A persistent context file (CLAUDE.md/AGENTS.md) is essential
- Verification loops are necessary
- Hooks provide deterministic control
- Context management is the hard problem
- Start with CLI, add UI later
- Design for agents, not humans

**What's more contested than it appears:**
- Plan mode (Boris: essential / steipete: "a hack for older models")
- Git worktrees (orchestration tools: required / steipete: unnecessary complexity)
- Issue trackers (some tools: GitHub Issues as task queue / steipete: "nothing did stick")
- Reading code (traditional: review carefully / steipete: "most code I don't read")
- Multi-agent orchestration (community: build sophisticated systems / steipete: "I don't see much need")
- Session management (common: fresh sessions / steipete: ride the context)

**The model capability question:**
steipete's workflow suggests many tools exist to compensate for model limitations. As models improve (his shift from Opus to GPT 5.2), tooling needs decrease. This is existential risk for tool builders: your tool may be obsoleted by the next model release, not the next competitor.

**What's painful:**
- Usage limits (universal complaint)
- Context loss (technical problem)
- Large codebase handling (model limitation)
- Inference speed (steipete: "limited by inference time")

**What a winner needs:**
- Solve context management (hardest)
- Built-in verification (expected)
- Team features (enterprise)
- Intelligent routing (power users)
- Model escalation (oracle pattern)
- Cross-project operations

**What to build vs buy:**
- Buy: TUI, monitoring, common integrations
- Build: Task decomposition, verification, internal integrations
- **Careful:** Many "build" items may be obsoleted by model improvements

**User segment insights:**
- Factory Operators (steipete): Minimal tooling, maximum velocity, trust the model
- Orchestrators (Boris): Elaborate tooling, parallel agents, verify everything
- These are not stages of evolution—they're different philosophies based on different work types

**The model-vs-tooling tension:**
steipete: "I love Opus as general purpose model... it has something special. I use it for most of my computer automation tasks."
But for coding: "Codex is just FAR better... I feel I get 5x more done on one codex session than with claude."

This suggests the market may bifurcate: Claude/Opus for general agent work, Codex for pure coding velocity. Tool builders need to decide which population they serve.

**Strategic questions for builders:**
1. Are you compensating for model limitations that will disappear?
2. Does your tool add value for Factory Operators or only for Orchestrators?
3. Can your tool survive users who "don't read much code anymore"?
4. Does your tool assume the system is the bottleneck when "usually I'm the bottleneck"?

The market is consolidating fast. Infrastructure-layer tools will be absorbed. The opportunity is in differentiated layers (enterprise compliance, domain verification) and in solving the hard problems no one has cracked (context management, intelligent decomposition). But beware: model improvements may solve problems faster than you can ship tools.

---

*Analysis compiled January 2026*
