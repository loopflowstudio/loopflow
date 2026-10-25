# Shared Context and Memory Layers for AI Agents (as of 2026-10-24)

Scope: Mem0, Letta / Letta Code, Zep / Graphiti, Supermemory, Cognee, ByteRover (formerly Cipher), Basic Memory, plus coding-agent-native memory (Claude Code auto memory, Codex memories, GitHub Copilot Memory), file conventions (AGENTS.md, CLAUDE.md, Cline Memory Bank), and notable 2026 entrants (claude-mem, Beads). Homepage claims were fetched 2026-10-24 unless noted. Funding and star figures come from secondary press unless marked otherwise.

## What is shared between agents or sessions, where does it live, and can a person read it?

### Takeaway
The field splits three ways. **Extracted-memory services** (Mem0, Zep, Supermemory, Cognee) turn conversations and data into facts, vectors and graphs held in a hosted or self-hosted database that people query rather than read. **File-based memory** (Basic Memory, Letta Context Repositories, ByteRover's context tree, Claude Code auto memory, Cline Memory Bank, AGENTS.md) stores Markdown that people can read and edit, and some of it is git-versioned. **Work-item memory** (Beads) stores issues as JSONL in the repo. In 2026 the movement is toward files and git. Letta rebuilt its memory on git in Feb 2026, ByteRover moved to "Git-Semantic" `brv vc` commands in v3, and Mem0 now ships coding-agent skills and plugins. Even so, every one of these stores memory *about* the work. None makes the work artifact itself (branch, commits, PR) the thing that carries context forward.

### Cited Findings

**Mem0**
- Headline: "AI memory that persists across sessions and agents"; Mem0 "extracts and updates memories" and "Automatically condenses chat history into compact memories that cut tokens and latency" — [mem0.ai](https://mem0.ai)
- Governance pitch: "Every read and write logged. Know what, who, and when." / "Your data stays yours." — [mem0.ai](https://mem0.ai)
- April 2026 algorithm: "single-pass, add-only extraction" (no updates or deletes). It stores agent-confirmed facts, links entities, and retrieves by semantic, keyword and entity match with temporal reasoning. Reported scores: LoCoMo 71.4→92.5 and LongMemEval 67.8→94.4. The README says the hosted platform includes optimizations that the OSS SDK lacks — [GitHub mem0ai/mem0](https://github.com/mem0ai/mem0)
- OpenMemory MCP is described in Mem0's own listing as "a local-first memory server and UI for sharing context among compatible MCP clients" (via search summary) — [search summary of mem0 materials](https://yespress.io/mem0.md) (aggregator, treat with caution)

**Letta / Letta Code**
- Context Repositories (Letta blog, **Feb 12, 2026**): "a rebuild of how memory works in Letta Code." Memory is a file hierarchy with YAML frontmatter, and a `system/` directory is always loaded into the prompt. "Context Repositories are git-backed, so every change to memory is automatically versioned with informative commit messages." — [Letta blog](https://www.letta.com/blog/context-repositories)
- Multi-agent mechanics: "each subagent gets an isolated git worktree" and memory edits merge back through git. `/init` fans subagents out over Claude Code and Codex histories. Background "reflection" runs in a separate worktree and merges back. A "defragmentation" skill targets 15–25 files. `/memfs` migrates the older memory blocks into the git-backed filesystem — [Letta blog](https://www.letta.com/blog/context-repositories)
- Letta Code "clones the memory repository locally," while API agents run server-side — [Letta blog](https://www.letta.com/blog/context-repositories)
- Letta Code is "stateful, so one agent can carry across many coding sessions"; `/remember` steers memory, and skills can be learned "from its current trajectory" — [Letta Code docs](https://docs.letta.com/letta-code)

**Zep / Graphiti**
- Zep: "Zep unifies business data, documents, and conversations into shared, governed context so agents can complete tasks correctly." It builds a "Context Graph" per user, customer, account or domain, with dated facts that update. The Zep v4 Context API was posted Oct 14–16, 2026 — [getzep.com](https://www.getzep.com)
- Graphiti: "a framework for building and querying temporal context graphs for AI agents." It stores entities (nodes), facts (edges with validity windows), and episodes, the raw data every fact traces to. Backends: Neo4j, FalkorDB, Amazon Neptune (Kuzu deprecated). "Graphiti is the graph construction framework inside Zep." The reference MCP server ships without authentication — [GitHub getzep/graphiti](https://github.com/getzep/graphiti)

**Supermemory**
- Inputs are files, chats, URLs and raw data. Its "learner-1" model "extracts, merges, infers, and forgets" into a vector-graph database that understands time, then injects the results into context. Deployment options: hosted, VPC, laptop, air-gapped — [supermemory.ai](https://supermemory.ai)
- Claude Code and Cursor integrations inject memory through hooks; MCP is also offered — [supermemory.ai](https://supermemory.ai)

**Cognee**
- "graph links facts across sessions"; an MCP server lets agents read and write memory; deployment via OSS, Cognee Cloud, or BYOC/on-prem — [cognee.ai](https://www.cognee.ai)

**ByteRover (formerly Cipher)**
- Cipher was "an opensource memory layer specifically designed for coding agents" with a dual memory: System 1 holds concepts and System 2 holds reasoning steps. It also had a team-shared workspace memory — [ByteRover docs: Cipher overview](https://docs.byterover.dev/cipher/overview)
- ByteRover CLI lets developers "curate project knowledge into a context tree, sync it to the cloud, and share it across tools and teammates." It stores a `.brv/` dir in the project — [GitHub campfirein/byterover-cli](https://github.com/campfirein/byterover-cli)
- v3.0.0 deprecates `brv push`/`brv pull` "Use Git-Semantic version control commands (brv vc push, brv vc pull ...)" — [ByteRover CLI reference](https://docs.byterover.dev/reference/cli-reference)
- Homepage: memory is "local first on your own machine," and a cloud option shares chosen files with "other agents and people." Files can be private or shared — [byterover.dev](https://www.byterover.dev)
- The GitHub page says the repository "was archived by the owner on Sep 27, 2026" and is read-only. The homepage carries a publish date of Sep 27, 2026. I found no successor repo — [GitHub campfirein/byterover-cli](https://github.com/campfirein/byterover-cli); [byterover.dev](https://www.byterover.dev)

**Basic Memory**
- Notes are plain Markdown ("Plain text on your disk. Forever."), indexed in local SQLite. AI writes through MCP tools (`bm_write`, `bm_read`, `bm_edit`). "AI and humans write to the same files; sync keeps them in step." — [GitHub basicmachines-co/basic-memory](https://github.com/basicmachines-co/basic-memory)
- "Read it like a doc. Query it like a database." Full-text, semantic and knowledge-graph indexes sit over the Markdown — [basicmemory.com](https://basicmemory.com)

**Claude Code (native)**
- Two mechanisms. CLAUDE.md is "instructions you write," and Claude can read AGENTS.md in its place. Auto memory is "notes Claude writes itself based on your corrections and preferences" — [Claude Code docs: memory](https://code.claude.com/docs/en/memory)
- Project CLAUDE.md files are "shared with your team through version control." Auto memory lives at `~/.claude/projects/<project>/memory/` with a `MEMORY.md` index. "Auto memory is machine-local... Files are not shared across machines or cloud environments." All worktrees of one repo share one directory. The first 200 lines or 25KB load each session. It is "on by default in local sessions." One of its note types is `project`: "ongoing work, deadlines, and decisions that Claude can't derive from the code or git history" — [Claude Code docs: memory](https://code.claude.com/docs/en/memory)
- Subagents do not load the main conversation's auto memory (forks do), and subagents can keep their own separate memory dir — [Claude Code docs: memory](https://code.claude.com/docs/en/memory)
- A secondary source says auto memory shipped in v2.1.59, Feb 2026 — [morphllm](https://www.morphllm.com/claude-code-memory-files) (secondary)
- One practitioner found it captured little after months of use, and titled the piece "automatic memory is not learning" — [Medium, B. Peterson](https://medium.com/@brentwpeterson/automatic-memory-is-not-learning-4191f548df4c) (opinion)

**OpenAI Codex (native)**
- Secondary sources describe AGENTS.md as the static layer, discovered from the git root down with a 32 KiB default cap. "Memories" are background summaries of prior sessions written to `~/.codex/memories/`, machine-local, and opt-in or region-limited depending on the source. I could not reach official OpenAI docs — [Mem0 blog: how memory works in Codex CLI](https://mem0.ai/blog/how-memory-works-in-codex-cli); [memoryplugin blog](https://blog.memoryplugin.com/does-codex-remember-past-conversations/) (both secondary, both from memory vendors)

**GitHub Copilot Memory**
- Public preview since **Jan 15, 2026** for all paid Copilot plans. It is off by default and repository-scoped. It is shared across Copilot cloud agent, code review and CLI, so a pattern code review finds can later guide the cloud agent. Only contributors with write access create memories, and memories are used only for users with read access. Agents validate memories against current code, and memories are deleted after 28 days. Owners review them in Repository Settings > Copilot > Memory — [GitHub changelog](https://github.blog/changelog/2026-01-15-agentic-memory-for-github-copilot-is-in-public-preview/); [GitHub blog](https://github.blog/ai-and-ml/github-copilot/building-an-agentic-memory-system-for-github-copilot/)
- In VS Code it falls back to local file storage when the feature is off — [VS Code docs: memory](https://code.visualstudio.com/docs/copilot/agents/memory)

**File conventions**
- Cline Memory Bank keeps six Markdown files in `memory-bank/`: projectbrief, productContext, systemPatterns, techContext, activeContext ("current focus and recent changes," updated most often), and progress ("what works, what's left, known issues"). Rules in `.clinerules/` are "stored in the repo, which makes it easy to share with collaborators" — [Cline docs: memory bank](https://docs.cline.bot/features/memory-bank.md)
- AGENTS.md was contributed by OpenAI as a founding project of the Linux Foundation's Agentic AI Foundation (Dec 9, 2025), alongside MCP and goose. The adoption claim is "more than 60,000" projects, which is OpenAI-sourced (earlier coverage said 40,000) — [OpenAI](https://openai.com/blog/agentic-ai-foundation); [SiliconANGLE](https://siliconangle.com/2025/12/09/linux-foundation-announces-agentic-ai-foundation-joined-anthropic-openai-block/)

**Notable 2026 entrants**
- claude-mem (Alex Newman) is a Claude Code plugin. It hooks session lifecycle events to observe tool use, file reads and edits, compresses them in a background worker into SQLite (plus ChromaDB vectors), and injects relevant context at session start. Reported at ~46k stars within ~48 hours of an early-April 2026 launch and 74.8k by May 11, 2026 — [Augment Code: claude-mem 74k](https://www.augmentcode.com/learn/claude-mem-74k-stars-agent-memory); [Crypto Briefing](https://cryptobriefing.com/claude-mem-permanent-memory-46k-stars/) (secondary; star counts unverified against GitHub)
- Beads (Steve Yegge) is "a distributed git-backed graph issue tracker" for agents. It stores JSONL in `.beads/`, which is "versioned and merged like code," with hash IDs to avoid collisions across branches and agents. Agents query it at session start for unblocked work. Reported ~17.9k stars as of Mar 2026, and one source claims it inspired Claude Code Tasks — [LinuxLinks](https://www.linuxlinks.com/beads-distributed-git-backed-graph-issue-tracker-ai-agents/); [Morph](https://www.morphllm.com/beads-agent-memory) (secondary)

### Inferences
- Transparency ranking. Readable and git-native: Letta Context Repositories, Beads, AGENTS.md/CLAUDE.md, Cline Memory Bank. Readable but local or proprietary sync: Basic Memory, ByteRover, Claude Code auto memory, Codex memories. Database-first and opaque to a casual reader: Mem0, Zep/Graphiti, Supermemory, Cognee, Copilot Memory (managed in GitHub settings, not in the repo), claude-mem (SQLite/Chroma).
- Claude Code's own docs define `project` memory as what "Claude can't derive from the code or git history." Native memory is thus positioned as a complement to git artifacts. That is a useful foil: Loopflow's claim is that the work artifacts should carry this information.
- Letta's Context Repositories are the closest structural neighbor to Loopflow. They use git, worktrees per subagent, and merges. But the git repo holds the *agent's memory*, separate from the code repo and PR, while Loopflow puts the context in the project's own branch, commits and PR.
- Beads is the second-closest neighbor. It keeps state in the code repo and merges with branches, but it is a task graph, not a design doc or PR narrative.
- Copilot Memory's 28-day expiry and "validate against current code" step admit that extracted memory goes stale. Artifacts tied to commits do not drift the same way, though that is an inference, not a claim any source makes.

### Gaps
- Official OpenAI Codex memories documentation was not retrieved; Codex details are from vendor blogs.
- OpenMemory's current (2026) status is not confirmed from a primary source.
- I could not determine whether ByteRover's archived repo means the CLI was discontinued, relocated or made closed-source.
- I did not confirm whether Supermemory or Cognee expose stored memory as human-readable files.

## Who is each for, and what are the license, pricing, funding and traction?

### Takeaway
Mem0, Zep, Supermemory and Cognee are venture-backed infrastructure for app builders and enterprises, with coding-agent plugins added as a distribution channel. Letta is a research lab whose Letta Code is its direct coding-agent product. ByteRover and Basic Memory are smaller and aimed at individual users and teams. The largest coding-agent traction belongs to free things: native features (Claude Code, Copilot), conventions (AGENTS.md), and OSS plugins (claude-mem, Beads).

### Cited Findings
- **Mem0**: Apache-2.0, 67.8k GitHub stars on the README page, while the homepage displays 62,590 (counts differ by snapshot) — [GitHub](https://github.com/mem0ai/mem0); [mem0.ai](https://mem0.ai). ~$24M total announced Oct 28, 2025: ~$3.9M seed (Kindred) plus a $20M Series A (Basis Set), with Peak XV, GitHub Fund and YC participating — [VCA Online](https://www.vcaonline.com/news/2025102807/mem0-raises-24m-series-a-to-build-memory-layer-for-ai-agents/); [Entrackr](https://entrackr.com/snippets/mem0-raises-24-mn-in-series-a-round-10602602). Audience: agent and app developers, enterprise governance, healthcare, education, support and CRM verticals — [mem0.ai](https://mem0.ai)
- **Letta**: Letta Code is Apache-2.0, released Dec 16, 2025 per a directory listing — [Terminal Trove](https://terminaltrove.com/ai-coding-agents/letta-code/). $10M seed led by Felicis, Sept 2024 (older), with Sunflower and Essence VC; angels include Jeff Dean and Clem Delangue — [VCA Online](https://www.vcaonline.com/news/2024092404/berkeley-ai-research-lab-spinout-letta-raises-10m-seed-financing-led-by-felicis-to-build-ai-with-memory/). A claim that Letta Code ranks #1 on Terminal-Bench among model-agnostic OSS harnesses is unverified — [Feedbagel](https://feedbagel.com/post/letta-code-a-memory-first-coding-agent-for-persistent-learning-across-sessions)
- **Zep / Graphiti**: Graphiti is Apache-2.0 with 32.2k stars at capture — [GitHub](https://github.com/getzep/graphiti). Zep is YC W24, seed-stage (amount not found). Its homepage offers a free tier plus enterprise sales and names Samsung, Zscaler and HoneyBook as customers — [YC](https://ycombinator.com/companies/zep-ai); [getzep.com](https://www.getzep.com)
- **Supermemory**: seed announced Oct 2025, reported as $2.6M (TechCrunch, per aggregator) or $3M (founder's post). Led by Susa Ventures, Browder Capital and SF1.vc, with angels Jeff Dean, Dane Knecht and Logan Kilpatrick — [Lets Data Science](https://letsdatascience.com/news/dhravya-shah-raises-3m-for-supermemory-9b29cd09); [Dealroom](https://nz.dealroom.co/news/feed/supermemory-secures-2-6m-for-ai-memory). A "$29 million" headline surfaced only on an unreliable aggregator and is unverified — [oreateai](https://discover.oreateai.com/discover/supermemory-secures-29-million-to-build-the-persistent-memory-layer-for-ai-agents). Tiers are Free, Pro, Max, Scale and Enterprise (no prices shown). It claims "100k+ organizations," "tens of millions" of end users and ">1T tokens per month" — [supermemory.ai](https://supermemory.ai)
- **Cognee**: Berlin-based, $7.5M seed led by Pebblebed, ~Feb 19–20, 2026, with 42CAP and Vermilion. Funds go to the cloud platform, a Rust engine for edge use, and OSS — [Pulse 2.0](https://pulse2.com/cognee-7-5-million-seed-funding-raised-for-building-enterprise-grade-memory-layer-for-ai-agents/). ~32.7k stars and "5M+ SDK runs/month" — [cognee.ai](https://www.cognee.ai)
- **ByteRover**: Elastic License 2.0, 4.9k stars, archived Sep 27, 2026 — [GitHub](https://github.com/campfirein/byterover-cli). Pricing: Free (100 context files, team sharing), Pro at 15/mo (currency not shown) with unlimited files, and Enterprise with SOC 2, SSO, RBAC and on-prem — [byterover.dev](https://www.byterover.dev). Cipher launched on Product Hunt Aug 3, 2025 (older) — [hunted.space](https://www.hunted.space/product/byterover/launches/cipher-by-byterover)
- **Basic Memory**: AGPL-3.0, ~4.3k stars — [GitHub](https://github.com/basicmachines-co/basic-memory). Pricing: Local free, Cloud from $15/seat/mo, Agents custom and credit-based. Audiences include individuals, teams, "agent fleets" and MSPs — [basicmemory.com](https://basicmemory.com)
- **GitHub Copilot Memory**: included in paid Copilot plans (preview) — [GitHub changelog](https://github.blog/changelog/2026-01-15-agentic-memory-for-github-copilot-is-in-public-preview/)
- **claude-mem**: free and OSS, reported at 74.8k stars by May 2026 — [Augment Code](https://www.augmentcode.com/learn/claude-mem-74k-stars-agent-memory) (secondary)

### Inferences
- Star counts for Mem0 (~68k) and claude-mem (~75k, reported) are comparable. A single-harness plugin built on lifecycle hooks matched years of SDK traction within weeks, a sign of strong demand from individual coding-agent users.
- Memory startups have raised modest sums (Supermemory ~$3M, Cognee $7.5M, Letta $10M, Mem0 ~$24M) while platform owners (Anthropic, GitHub, OpenAI) ship memory free. The standalone memory layer for coding agents is under pressure from bundling.

### Gaps
- No 2026 funding found for Zep, Letta or Supermemory. Mem0, Zep and Cognee pricing pages were not fetched.
- I found no Hacker News threads with point counts. Traction signals come from stars and press only.

## Pitch language (verbatim headlines and taglines)

### Takeaway
Two vocabularies dominate: "memory that persists" (Mem0, Basic Memory, claude-mem coverage) and "learning" or "context" (Letta, Supermemory, Zep). In 2026 the leaders moved from "memory" toward "continual learning" (Letta, Supermemory) and "governed context" (Zep). ByteRover's "share the right context across your agents and team" is the closest to a multi-agent/team sharing pitch.

### Cited Findings
- Mem0: "AI memory that persists across sessions and agents" / "Drop-in memory infrastructure for AI agents and apps. Context that persists. Built for production." / "Mem0 gives agents persistent memory without pipeline changes." — [mem0.ai](https://mem0.ai). README: "The Memory Layer for AI Agents" — [GitHub](https://github.com/mem0ai/mem0)
- Letta: "Machines that learn" / "experiential agents that remember everything, learn continuously, and improve themselves over time." / "Context is selfhood." / "Learning in token space is the key to building AI agents that truly improve over time." (Dec 2025) — [letta.com](https://www.letta.com)
- Zep: "Reliable agents need the right context" / "Zep unifies business data, documents, and conversations into shared, governed context so agents can complete tasks correctly." — [getzep.com](https://www.getzep.com). Graphiti: "Build Real-Time Knowledge Graphs for AI Agents" — [GitHub](https://github.com/getzep/graphiti)
- Supermemory: "supermemory is building the default engine for memory and continual learning for agents." / "Memory that keeps learning." / "Any model, any harness." / "The hard infrastructure, done." — [supermemory.ai](https://supermemory.ai). Founder, Oct 2025: "the best memory for LLMs and agents" — [via search summary of X post](https://americanbazaaronline.com/2025/10/09/at-20-dhravya-shah-raises-3-million-for-his-al-startup-468551/)
- Cognee: "Memory for Agents, Teams, and People" / "Agents get lost in your complex systems." / "Give your agents memory in 60 seconds." — [cognee.ai](https://www.cognee.ai)
- ByteRover: "Share the right context across your agents and team. You control what crosses." / "You own your data" — [byterover.dev](https://www.byterover.dev). CLI: "Interactive REPL CLI for AI-powered context memory." — [GitHub](https://github.com/campfirein/byterover-cli)
- Basic Memory: "Knowledge Management. AI Native." / "Real memory for your AI." / "A knowledge base you own." / "Read it like a doc. Query it like a database." — [basicmemory.com](https://basicmemory.com). README: "Your AI never forgets again." — [GitHub](https://github.com/basicmachines-co/basic-memory)
- Claude Code docs: "Each Claude Code session begins with a fresh context window." / auto memory "lets Claude learn from your corrections without manual effort." — [Claude Code docs](https://code.claude.com/docs/en/memory)
- Cline: "Memory Bank: How to Make Cline an AI Agent That Never Forgets" (blog title) — [cline.bot](https://cline.bot/blog/memory-bank-how-to-make-cline-an-ai-agent-that-never-forgets)

### Inferences
- The shared framing across the category is anthropomorphic: agents "remember," "learn," "never forget." The thing remembered is the agent's knowledge, not the state of a piece of work. None of these headlines talks about handing off a task in progress. The "artifacts, not memory" story Loopflow tells has no direct competitor slogan.
- "Never forgets" is the cliché of the category (Basic Memory, Cline, claude-mem coverage), so it is worth avoiding.

### Gaps
- Mem0, Zep and Cognee pricing-page copy was not captured.

## Which target coding agents and multi-agent handoff specifically?

### Takeaway
Nearly all now advertise coding-agent integrations, usually Claude Code and Cursor through MCP, hooks or plugins. Only a few were built for coding agents first: Letta Code, ByteRover/Cipher, claude-mem, Beads, Cline Memory Bank, and the native Claude Code, Codex and Copilot features. Explicit cross-agent handoff is rare. GitHub Copilot Memory ("one agent's findings can inform the others") and Letta (subagent worktrees merging into shared memory) are the clearest. Beads hands off *work items*, not context. Most others share *knowledge* across agents, not *in-progress work*.

### Cited Findings
- Mem0 ships agent skills for Claude Code, Cursor, Codex and Windsurf (`npx skills add`, `.claude-plugin`, `.cursor-plugin`) — [GitHub](https://github.com/mem0ai/mem0). It also publishes coding-agent content marketing (e.g. "Codex + Mem0 MCP: build a coding agent that remembers your codebase") — [Mem0 blog](https://mem0.ai/blog/codex-mem0-mcp-build-a-coding-agent-that-remembers-your-codebase)
- Letta Code is a "memory-first coding agent." Its co-founder argues that most CLI agents start fresh each time while Letta Code keeps history — [HackerNoon](https://sia.hackernoon.com/letta-unveils-memory-first-coding-agent-as-context-engineering-takes-hold); [Letta docs](https://docs.letta.com/letta-code). Memory init ingests Claude Code and Codex histories, and subagents share memory through git worktrees and merges — [Letta blog](https://www.letta.com/blog/context-repositories)
- Supermemory lists Claude Code, Cursor, Claude Desktop, ChatGPT and Hermes integrations, and cites a SWE-ContextBench result (Feb 2026) of 30.30% resolution — [supermemory.ai](https://supermemory.ai)
- Cognee lists Claude Code, Codex, Cursor, OpenClaw, Hermes, LangGraph and CrewAI, with Claude Code and Cursor "keeping project context across sessions" — [cognee.ai](https://www.cognee.ai)
- ByteRover "Works with 22+ AI coding agents (Cursor, Claude Code, Windsurf, Cline, and more)" — [GitHub](https://github.com/campfirein/byterover-cli). Its homepage describes multi-agent setups (builder/reviewer/planner) where agents "share context but respect role boundaries." The homepage FAQ "Will a new agent or session remember my past context?" was blank — [byterover.dev](https://www.byterover.dev)
- Basic Memory lists Claude, Codex, Cursor, VS Code and Obsidian via MCP — [basicmemory.com](https://basicmemory.com)
- Zep's homepage mentions no coding-agent or MCP features. Graphiti has a reference MCP server — [getzep.com](https://www.getzep.com); [GitHub graphiti](https://github.com/getzep/graphiti)
- GitHub Copilot Memory is explicitly cross-agent: a pattern found in code review can later guide the cloud agent, and memory is shared across cloud agent, code review and CLI — [GitHub blog](https://github.blog/ai-and-ml/github-copilot/building-an-agentic-memory-system-for-github-copilot/)
- Claude Code auto memory is shared across worktrees of one repo but is machine-local and not loaded into subagents (only into forks) — [Claude Code docs](https://code.claude.com/docs/en/memory)
- Beads: agents ask for the highest-priority unblocked work at session start. Users tell the agent, in AGENTS.md or CLAUDE.md, to use `bd` instead of markdown plans — [Morph](https://www.morphllm.com/beads-agent-memory); [pkg.go.dev](https://pkg.go.dev/github.com/steveyegge/beads@v0.12.0)
- Cline Memory Bank's `activeContext.md`/`progress.md` are a manual handoff note, refreshed by the agent on "update memory bank" — [Cline docs](https://docs.cline.bot/features/memory-bank.md)

### Inferences
- "One agent picks up where another left off" is served today by:
  - hand-maintained progress files (Cline's activeContext/progress, ad-hoc memory-bank conventions);
  - auto-extracted recall that restores context at session start (claude-mem, Mem0/Supermemory/Cognee plugins, Claude/Codex native memory);
  - Copilot Memory's repository-scoped learnings;
  - Beads's task graph.

  None of these treats the PR or branch as the handoff unit. That is Loopflow's distinct ground.
- The closest contrasts for positioning:
  - Letta: git-versioned *agent memory* separate from the code, versus Loopflow's git *work artifacts* inside the code repo.
  - Copilot Memory: repo-scoped but stored in GitHub settings, expires in 28 days, invisible in diffs.
  - Beads: git-native, but holds tasks, not designs or PRs.
- A recurring vendor admission is that memory goes stale: Copilot validates against current code and expires memories, Claude Code says to treat notes as hints, and Mem0 moved to add-only extraction. This gives Loopflow a concrete angle, "context that is reviewed and merged, not recalled."

### Gaps
- I found no explicit task-handoff feature (pausing one agent mid-task and resuming in another harness with state) documented by Mem0, Zep, Supermemory or Cognee.
- Cursor's own native memory status in 2026 was not researched.
- I did not survey other memory MCP servers (e.g. the official MCP "memory" reference server, OpenMemory) for 2026 traction.
