# Expansion Opportunities

## Current state

Loopflow solves prompt and context construction for coding agents. The core value: assemble context (docs, diffs, files, clipboard), pair it with reusable prompts, pass to Claude/Codex/Gemini. Prompts are versioned artifacts. Flows chain prompts. Agents run flows in the background.

Target users are "softwarists"—engineers who want craft *and* throughput, co-creation *and* delegation. They use AI constantly but hit limits when code goes off the rails or lacks judgment.

What loopflow does well:
- Context assembly with token management
- Prompts as files, not chat logs
- Multi-model portability
- Background agents with triggers (loop, watch, cron)
- Native macOS app for visual control

---

## Direction 1: Execution Memory

**What**: Learn from every step execution. When steps succeed, fail, or get edited by humans—feed that back into prompt selection and context assembly.

**Why**: Currently prompts are static. Run `lf implement` twice with the same context, get the same prompt. But the system already logs every execution (step_runs table), tracks outcomes, and knows when humans intervene. This data is sitting unused.

With execution memory:
- Prompts that produced failed tests could trigger different follow-up prompts
- Voice selection could adapt based on what worked for similar tasks
- Context assembly could learn which files matter for which step types
- Error patterns could surface as warnings before you hit them again

**Builds on**:
- `step_runs` table with status, started_at, ended_at
- `flow_runs` tracking full execution chains
- Existing summary/cache infrastructure in `~/.lf/lfd.db`
- Token counting already tracks what went in

**Implementation path**:
1. Add outcome tagging to step_runs (success, failure, human_edit, timeout)
2. Build a retrieval layer: "find similar past executions"
3. Surface insights: "last 3 times implement ran on auth code, review caught X"
4. Eventually: auto-adjust voice/context based on learned patterns

**Open questions**:
- Privacy: should execution history sync across machines/repos?
- Granularity: per-step learning vs per-flow vs per-repo?
- Cold start: how much data before patterns emerge?

---

## Direction 2: Verification Layer

**What**: Structured verification that step output matches intent. Not just "did it run" but "did it do what the design doc said."

**Why**: The gap between autonomous agents and trusted agents is verification. Current flows chain steps, but nothing checks whether the output is correct—you're trusting the agent or reviewing everything manually. This is why the "craft over vibes" principle exists.

With a verification layer:
- Flows could include `verify` steps that check output against specs
- Design docs (`scratch/<branch>.md`) become testable contracts
- Autonomous loops gain confidence: the agent self-checks before creating PRs
- Regression detection: "this change affects X, but the spec said Y"

**Builds on**:
- Flow DAG already supports conditional branching (`choose`)
- Design docs in `scratch/` already capture intent
- Review step already assesses code quality
- Polish step already runs tests

**Implementation path**:
1. Define a verification protocol: what does a "spec" look like? (structured design docs)
2. Add `verify` as a flow construct: `{"verify": "spec_path", "on_fail": "iterate"}`
3. Build spec comparison: diff design intent vs actual changes
4. Connect to test runner: verify includes "tests pass" as baseline

**Open questions**:
- Spec format: structured YAML vs natural language in markdown?
- Failure modes: retry, escalate to human, or abort?
- How to verify non-functional requirements (style, architecture)?

---

## Direction 3: Context Intelligence

**What**: Make context assembly task-aware. Instead of "include these files," understand "what's relevant for this task."

**Why**: Context is the hard problem. Too little and the agent hallucinates. Too much and you hit token limits. Current assembly is rule-based: include diff_files, include summaries, exclude patterns. But relevance depends on the task.

Asking "add caching to the API" needs different context than "fix the auth bug." The system knows both the task (step + args) and the codebase (summaries, file structure). It should connect them.

With context intelligence:
- Token budget goes further: less irrelevant code in context
- Agents see what matters: relevant imports, related tests, similar patterns
- Summaries become searchable: "find code that handles caching"
- New codebases become navigable faster

**Builds on**:
- Summary system already generates codebase overviews
- Token trimming already drops components by size
- Skills discovery already searches for relevant prompts
- Context gathering already walks the file tree

**Implementation path**:
1. Index summaries for semantic search (embeddings or keyword)
2. Extract task keywords from step + args
3. Score file relevance: "this file mentions caching, include it"
4. Replace greedy trimming with relevance-weighted trimming

**Open questions**:
- Embedding model: local (fast, private) vs API (better quality)?
- Update frequency: rebuild index on every commit? Lazy?
- Explainability: why was this file included?

---

## Not yet

**Multi-agent collaboration**: Multiple specialized agents working together—one designs, one implements, one reviews, with handoffs. The architecture hints at this (subagents in Claude Code, flow DAGs), but it requires rethinking what an "agent" is. Current agents are solo: one flow, one area, one worktree. True collaboration needs shared state, conflict resolution, and coordination protocols. Worth exploring after the single-agent experience is solid.

**Prompt marketplace**: Share prompts beyond the repo. The skills system already supports external libraries (superpowers, SkillRegistry), but a true marketplace needs trust, versioning, and discovery. The infrastructure isn't wrong—it's just early. Let patterns emerge from local usage before building distribution.

**Cross-project learning**: Apply lessons from one repo to another. "This pattern worked in project A, try it in project B." Compelling but requires solving: data privacy (not all repos should share), semantic translation (different codebases have different idioms), and trust (why should I trust what worked elsewhere?). Execution memory within a repo comes first.

**Rust lfd rewrite**: The daemon works. Python asyncio has overhead, but it's not the bottleneck. Rewriting in Rust adds maintenance burden and build complexity for benefits that aren't proven. If the daemon becomes a stability problem, revisit. Until then, invest in reliability features (the roadmap already covers this) rather than rewrites.
