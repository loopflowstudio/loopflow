# Multi-agent coding orchestrators and background coding agents (as of 2026-10-24)

Method note: most facts come from fetched homepages, docs, GitHub READMEs and changelogs, fetched 2026-10-24. Some come from search-result summaries of third-party reviews, and those are marked "(secondary)". Homepage quotes are verbatim from the fetch tool's quoted output.

## How does each isolate work, and what persists afterwards?

### Takeaway
Local orchestrators almost all use one git worktree plus a branch per agent. Sculptor is the exception: it uses a container per agent. Cloud agents use a VM or container per task. Across the field, what persists is a branch and a PR, plus sometimes a saved session log. A durable, resumable conversation tied to the unit of work is rare: Sculptor saves sessions, Copilot keeps session logs and Copilot Memory, Devin and Amp have memory or thread features.

### Cited Findings
**Local orchestrators**
- Conductor (Melty Labs, Mac app): "Run parallel Claude Code, Codex, and Cursor agents in isolated workspaces on your Mac." Latest release on the homepage is 0.92.0. — [Conductor](https://www.conductor.build/)
- Conductor creates a separate git worktree and branch for each workspace. Each task gets its own workspace, branch, files, terminal, diff and review path. The flow is: review the diff, open a PR, merge, then archive the workspace. — [Conductor docs: Codex with worktrees](https://www.conductor.build/docs/guides/git-worktrees/run-codex-with-git-worktrees); [Conductor docs: Claude Code with worktrees](https://www.conductor.build/docs/guides/git-worktrees/run-claude-code-with-git-worktrees)
- Sculptor (Imbue): "Sculptor: the missing UI for parallel coding agents" / "Now you can run multiple coding agents in safe containers and instantly see their changes in action." Each agent gets its own container, and Imbue contrasts this with worktrees, which share your local environment. "Every session is saved, including plans, chats, tool calls, and code changes, and can be reopened." Forking from any point in history is listed as upcoming. The page was last updated 2026-04-30. Sculptor runs on macOS (Apple Silicon) and Linux. — [Imbue Sculptor](https://imbue.com/sculptor/)
- Sculptor Pairing Mode brings an agent's container work into your local repo with one click and syncs changes both ways. A third-party review says it uses Mutagen (secondary). Users can supply their own Dockerfile. — [Imbue announce](https://imbue.com/blog/sculptor-announce); [HN Show HN](https://news.ycombinator.com/item?id=45427697)
- Vibe Kanban creates git worktrees automatically and runs setup scripts. It has a planning board, issues and sub-issues, and a team tracker that shows agent and PR status. — [vibekanban.com](https://www.vibekanban.com/)
- Claude Squad gives each session its own tmux terminal and its own git worktree on a separate branch. — [GitHub smtg-ai/claude-squad](https://github.com/smtg-ai/claude-squad)
- Crystal: "Run multiple Codex and Claude Code AI sessions in parallel git worktrees." It is deprecated and replaced by Nimbalyst as of February 2026. — [GitHub stravu/crystal](https://github.com/stravu/crystal)
- Superset: "Each agent runs in its own isolated Git worktree." Repos, worktrees, terminal output and sessions stay local, and cloud sync covers only account and org metadata. "Workspaces keep running when your laptop sleeps": any machine can be added as a host. — [superset.sh](https://superset.sh/)
- uzi: a CLI with automatic git worktree management and a tmux session per agent, e.g. `uzi prompt --agents claude:3,codex:2 "Implement user auth"`. The repo was last updated June 2025, so it is likely dormant (older item). — [GitHub devflowinc/uzi](https://github.com/devflowinc/uzi)
- cmux (Manaflow): a Ghostty/libghostty-based macOS terminal. Its workspaces are vertical tabs showing branch, cwd and ports. The homepage does not list worktrees as a feature. It has a scriptable browser pane, SSH workspaces and attach to remote tmux; cloud VMs are listed as an upcoming Founder's Edition feature. — [cmux.com](https://cmux.com/)
- herdr: a background server hosts the terminals. "closing the lid or losing the network doesn't stop the agents." After a machine restart it restores the layout and resumes sessions. It adds remote machines over SSH, and Herdr Cloud is "coming soon". Worktrees are not mentioned. — [herdr.dev](https://herdr.dev/)
- Terragon Labs was a cloud background orchestrator for Claude Code, Codex and others. It shut down on February 9, 2026, all sandboxes were terminated, and the code was open-sourced as-is. The shutdown notice points users to Claude Code on the web. — [terragonlabs.com](https://www.terragonlabs.com/); [GitHub terragon-oss](https://github.com/terragon-labs/terragon-oss)

**Cloud/background agents**
- Cursor Cloud Agents (formerly Background Agents) run on isolated Ubuntu VMs, configured via `.cursor/environment.json` and build snapshots, and return a branch and PR (secondary). — [continuumcode guide](https://continuumcode.ai/guides/cursor-background-agents/); [learncursor](https://www.learncursor.dev/guides/cursor-cloud-agents)
- Cursor changelog, 2026-08-19: subagents can run on their own VMs, "Each gets an isolated copy of the project with clean context in its own cloud environment." Cloud agents "automatically subscribe to PRs they create and drive them to completion." — [Cursor changelog](https://cursor.com/changelog/08-19-26)
- A June 2026 guide reports that a Cursor agent's VM can become unreachable mid-run and be rebuilt from the snapshot (secondary). — [learncursor](https://www.learncursor.dev/guides/local-agent-vs-cloud-agent)
- Codex cloud: each task runs in a separate cloud environment preloaded with the repo. At DevDay (2026-09-29) OpenAI added reusable cloud dev environments that work across devices. — [Wikipedia](https://en.wikipedia.org/wiki/OpenAI_Codex_(AI_agent)); [TechCrunch](https://techcrunch.com/2026/09/29/openai-gives-codex-reusable-cloud-environments-that-work-across-devices/)
- Codex local worktrees: CLI 0.154.0 (Sept 9) added experimental starting or forking of tasks in separate worktrees, and 0.162.0 (Oct 8) added managed worktree create/list. The Codex app merged into the ChatGPT desktop app on 2026-07-09 (secondary, via search summaries of the OpenAI changelog and Wikipedia). — [ChatGPT & Codex changelog](https://learn.chatgpt.com/docs/changelog); [Wikipedia](https://en.wikipedia.org/wiki/OpenAI_Codex_(AI_agent))
- Devin: "Each session runs in its own VM," including Windows, Mac and Linux VMs. Devin "runs in the cloud or on your machine." — [devin.ai](https://devin.ai/)
- Google Jules clones the repo into a "Cloud VM" and produces a PR you can review, approve and publish. — [jules.google](https://jules.google/)
- GitHub Copilot coding agent (the docs now say "Copilot cloud agent") works in an "ephemeral cloud development environment", configured with `copilot-setup-steps.yml`, with a 59-minute session maximum. It creates branches and commits and can open one PR per task. Session logs show the work and tools used, and teammates can follow sessions. Copilot Memory builds long-term understanding across sessions. — [GitHub Docs](https://docs.github.com/en/copilot/concepts/agents/coding-agent/about-coding-agent)
- Amp: "When you give every agent its own orb *everything* changes." Orbs are remote machines where agents run unsupervised after the laptop closes. `amp sync` mirrors an orb's changes into the local checkout, and `amp threads continue` exists. — [ampcode.com](https://ampcode.com/)
- Ona (formerly Gitpod): each agent gets its own cloud environment ("More than a sandbox"), can run in the customer VPC, and has kernel-level policies. It returns a PR. — [ona.com](https://ona.com/)
- Augment: the page now leads with "Run your software factory" (platform "Cosmos"). It offers managed cloud sandboxes or customer-managed machines. The earlier Remote Agents allowed up to 10 in parallel in VS Code (2025, older item). — [Augment Remote Agents page](https://www.augmentcode.com/product/remote-agents); [Augment blog](https://www.augmentcode.com/blog/production-ready-ai-remote-agents-now-available-for-all-augment-code-users)
- Factory: Droids run across CLI, desktop, web, mobile, Slack, Teams, Jira, Linear and CI. There are "persistent Droid Computers" and a Droid CLI for interactive or headless work. — [factory.com](https://factory.com/)

### Inferences
- "Branch + PR" is the universal persistence contract. Several products persist a transcript (Sculptor sessions, Copilot session logs, Amp threads), but none of the local worktree orchestrators surfaced in this research markets the conversation as a durable object any other agent can pick up.
- The multiplexer layer (cmux, herdr) persists *processes and terminals*, not *work units*. Neither manages worktrees on its homepage.

### Gaps
- Conductor's checkpoints, chat-history persistence and archive semantics weren't confirmed; the docs index fetch redirected and wasn't re-fetched.
- What persists after Jules or Codex cloud tasks finish (whether transcripts can be resumed) wasn't confirmed from primary sources.

## Can one agent continue another's work? Is there a coordinating conversation or a planning/task layer?

### Takeaway
Handoff is mostly *location* handoff (local↔cloud, laptop↔phone), not *agent-to-agent* continuation. Coordination comes in two forms. The orchestrator-agent form is Devin managing Devins, Cursor subagents, and herdr agents prompting each other. The tracker form is Vibe Kanban boards, Linear/Slack triggers in Factory, Warp and Amp, and GitHub issues assigned to Copilot or third-party agents.

### Cited Findings
- Conductor: a review reports no agent-to-agent handoff, "parallel work with a human merge" (secondary). A third-party profile says Conductor v0.63.0 (June 2026) added Cursor support and a "Dispatcher" feature (secondary, unverified). — [collaborativecodingagents review](https://collaborativecodingagents.com/reviews/conductor); [rywalker.com](https://rywalker.com/research/conductor)
- Cursor 3 introduced an Agents Window with local-to-cloud handoff: "move an agent from cloud to local to iterate, then back to keep it running" (secondary). — [digitalapplied](https://www.digitalapplied.com/blog/cursor-3-agents-window-complete-guide)
- Cursor added `/goal` "to give the agent a long-lived objective to work towards until it's fully complete". It pairs with `/loop` and Slack thread steering: "`@cursor check back in an hour and keep going until that feedback is in`". — [Cursor changelog 2026-08-19](https://cursor.com/changelog/08-19-26)
- Devin "can split a large task across parallel cloud agents, manage them, and report back". The homepage links a post titled "Memory and dreaming: how Devin learns from working with you." — [devin.ai](https://devin.ai/)
- herdr: a CLI and socket API expose the same surface agents use. Agents can split panes, "start and prompt each other, and wait until another agent is blocked." — [herdr.dev](https://herdr.dev/)
- cmux: a CLI and socket API let scripts and agents create workspaces and splits, send input, read the screen and drive the browser. Subagents appear as native panes. A cmux PR adopts "Herdr-style agent plugins and journal projections". — [cmux.com](https://cmux.com/); [cmux PR #13299](https://github.com/manaflow-ai/cmux/pull/13299)
- Superset has a CLI (`superset new`, `superset ls`), a TypeScript SDK and an MCP server for scripting workspaces and agents. Automations run agents on schedules or events and open PRs. — [superset.sh](https://superset.sh/)
- Vibe Kanban's tagline: "Accelerate the human planning and review your coding agents are waiting on". It offers kanban cards, issues and sub-issues, and lets you switch agents without changing workflow. — [vibekanban.com](https://www.vibekanban.com/)
- Amp: a prompt can be continued from your phone after closing the laptop. Multiplayer lets teammates share control of an orb. Agents wake on Linear issues, failing CI or self-set schedules. — [ampcode.com](https://ampcode.com/)
- Warp: "Warp Factories coordinates work from triage through review to a mergeable PR." A "Foreman" routes work via @-mentions in Slack and Linear. Runs start from an issue, chat message or schedule. — [warp.dev](https://www.warp.dev/)
- Ona: Automations fire on PRs, schedules or webhooks across a codebase. A customer quote reads "90-95% of migration work is done by Ona Automations". — [ona.com](https://ona.com/)
- GitHub Agent HQ was announced at Universe on 2025-10-28 (older item). It brings agents from Anthropic, OpenAI, Google, Cognition and xAI into GitHub under a paid Copilot subscription. In Feb 2026, Pro+ and Enterprise users could start sessions from GitHub, Mobile and VS Code, and Codex could be assigned to issues and PRs and compared with Claude and Copilot. — [Help Net Security](https://www.helpnetsecurity.com/2026/02/05/github-enables-coding-agents/); [DevOps Digest](https://www.devopsdigest.com/github-introduces-agent-hq)
- Copilot Chat can pull in session logs to answer questions about agent-created PRs. — [GitHub Docs](https://docs.github.com/en/copilot/concepts/agents/coding-agent/about-coding-agent)
- Sculptor: forking from session history is "upcoming"; the page doesn't say it has shipped. — [Imbue Sculptor](https://imbue.com/sculptor/)
- Codex CLI 0.154.0 supports "forking" tasks into separate worktrees (secondary). — [ChatGPT & Codex changelog](https://learn.chatgpt.com/docs/changelog)

### Inferences
- The "coordinating conversation that talks to work running elsewhere" exists mainly in the cloud vendors (Devin parent session, Cursor Slack threads and `/goal`, Warp Foreman). Local tools offer a *dashboard* (status: working/blocked/waiting) rather than a conversation.
- Agent-to-agent continuation of a failed run by a *different* agent or provider is not a headline feature anywhere found. Herdr's socket API makes it possible, but leaves the work model to the user.

### Gaps
- No primary source describes cross-agent "resume this other agent's transcript" in Conductor, Superset or Vibe Kanban.
- Whether Devin's child agents leave independent, resumable sessions wasn't confirmed.

## Multi-provider or locked to one vendor? Own subscription or vendor billing?

### Takeaway
Local orchestrators are multi-agent and bring-your-own-subscription by design. Cloud agents split into two groups. Some are model-vendor-locked (Codex = OpenAI, Jules = Gemini, Claude Code web = Anthropic). Others are platforms that mix models under their own billing (Devin, Factory, Cursor, Warp, Augment, Amp), and Copilot/Agent HQ aggregates third-party agents under a Copilot subscription.

### Cited Findings
- Conductor supports Claude Code, Codex, Cursor and OpenCode. It is free, and you bring your own Claude or Codex subscription (secondary). — [Conductor](https://www.conductor.build/); [codepick guide](https://codepick.dev/en/guides/conductor-build-intro/)
- Superset lists Claude Code, Codex, OpenCode, Gemini, Cursor Agent, Mistral Vibe, Kimi Code, Grok CLI, Hermes, Devin, Antigravity, Kiro and others: "any CLI-based coding agent". It uses your own subscriptions or keys and doesn't proxy model calls. — [superset.sh](https://superset.sh/)
- herdr detects 22 agent CLIs, including Claude Code, Codex, Pi, opencode, Cursor, Grok, Copilot and Hermes, and doesn't wrap them. — [herdr.dev](https://herdr.dev/)
- cmux works with any terminal agent (Claude Code, Codex, OpenCode, Gemini CLI, Aider). — [cmux.com](https://cmux.com/)
- Claude Squad supports Claude Code (default), Codex, Gemini, OpenCode, Amp and Aider. — [GitHub](https://github.com/smtg-ai/claude-squad)
- Sculptor supports Claude Code and works with your existing Claude Code subscription or API key. Codex support was added 2025-12-03 and requires an OpenAI API key. — [Imbue on X](https://x.com/imbue_ai/status/1973513469753434433); [Imbue Sculptor](https://imbue.com/sculptor/)
- Nimbalyst "works with your Claude Code or Codex subscription or API key." — [nimbalyst.com](https://nimbalyst.com/)
- Codex Cloud is available on Plus, Pro, Business, Enterprise, Healthcare and Education, but not Free or Go. — [OpenAI Help](https://help.openai.com/en/articles/11369540-using-codex-with-your-chatgpt-plan)
- Jules plans with Gemini (Gemini 3 Pro named as latest). — [jules.google](https://jules.google/)
- Devin "mixes and matches frontier models", which the page says gives frontier performance "for up to 60% less". — [devin.ai](https://devin.ai/)
- Factory supports multiple model providers. Models bill at usage multipliers, and a "Droid Core" open-weight pool kicks in after limits (secondary). — [factory.com](https://factory.com/); [CostKite](https://costkite.com/tools/factory-droids)
- Warp: "any model or harness". Teams can use Claude Code or Codex as the harness and pick models per pipeline stage. Billing is usage-based per agent run. — [warp.dev](https://www.warp.dev/)
- Amp lets you link a ChatGPT subscription to run the low, medium and high settings of its "Dial". It also has Amp credits and orb-time billing. — [ampcode.com](https://ampcode.com/)
- Augment: teams pick a model per task, with no single provider. — [Augment](https://www.augmentcode.com/product/remote-agents)
- Agent HQ brings Anthropic, OpenAI, Google, Cognition and xAI agents into a paid Copilot subscription. — [Help Net Security](https://www.helpnetsecurity.com/2026/02/05/github-enables-coding-agents/)
- Ona runs "Ona, Codex, and other agents under one governed setup". — [ona.com](https://ona.com/)

### Inferences
- Bring-your-own-subscription orchestration (using Claude Max or ChatGPT Pro logins) is the norm for local tools and close to absent in the cloud. Amp's ChatGPT-link and Warp's harness choice are partial exceptions. Terragon was a BYO-Claude-Code cloud product and shut down. Its X posts promoted tracking "if you are maxing your max plan". — [Sawyer Hood on X](https://x.com/sawyerhood/status/1944908622280188252)

### Gaps
- Billing for Cursor Cloud Agents and Copilot cloud agent (premium requests, Actions minutes) wasn't confirmed from primary pages.

## Audience, pricing, open source, traction, pitch language

### Takeaway
Local tools are free and mostly open source (or source-available), aimed at individual power users. Cloud products sell to teams and enterprises on usage-based pricing, and as of late 2026 the dominant pitch phrase is "software factory".

### Cited Findings
**Verbatim homepage pitches (fetched 2026-10-24)**
- Conductor: "Run parallel Claude Code, Codex, and Cursor agents in isolated workspaces on your Mac." — [conductor.build](https://www.conductor.build/)
- Superset: "Run an Army of Coding Agents" / "One workspace for Claude Code, Codex, and any coding agent." — [superset.sh](https://superset.sh/)
- herdr: "Run them anywhere. Leave them running." / "the agent runtime" — [herdr.dev](https://herdr.dev/)
- cmux: "The terminal built for multitasking, organization, and programmability." — [cmux.com](https://cmux.com/)
- Sculptor: "the missing UI for parallel coding agents" — [imbue.com](https://imbue.com/sculptor/)
- Vibe Kanban: "Your Engineering Bottleneck Has Shifted" / "Accelerate the human planning and review your coding agents are waiting on" — [vibekanban.com](https://www.vibekanban.com/)
- Nimbalyst: "Overwhelmed and disorganized by Codex and Claude Code?" / "Think clearly with free, local, open source tools from Nimbalyst" — [nimbalyst.com](https://nimbalyst.com/)
- Devin: "Meet Devin, your team's autonomous software engineer" / "Devin runs in the cloud or on your machine, tests in its own browser, and won't stop until the PR is ready to merge" — [devin.ai](https://devin.ai/)
- Factory: "Build Your Software Factory" / "Factory is an agent-native software development platform for planning, building, reviewing, testing, …" — [factory.com](https://factory.com/)
- Warp: "Open infrastructure for cloud software factories" / "build on Warp: factories as code, any model or harness, with evals, benchmarks, and self-improvement built in." — [warp.dev](https://www.warp.dev/)
- Ona: "The Enterprise software factory" / "Run background agents in the cloud. Orchestrated, governed, secured at the kernel." — [ona.com](https://ona.com/)
- Augment: "Run your software factory" — [augmentcode.com](https://www.augmentcode.com/product/remote-agents)
- Amp: "Coding agent and dev environment built for the frontier" / "When you give every agent its own orb everything changes." — [ampcode.com](https://ampcode.com/)
- Jules: "Jules does coding tasks you don't want to do." — [jules.google](https://jules.google/)

**Licenses, pricing and traction**
- herdr: Apache 2.0, Herdr, Inc. Claims 44,627 GitHub stars, 1,644,092 installs and 1,789 community plugins. — [herdr.dev](https://herdr.dev/)
- cmux: GPL (sources conflict, GPL-3.0 vs AGPL). Free, with a paid Founder's Edition. About 26–28k stars (secondary). — [cmux.com](https://cmux.com/); [flaviocopes](https://flaviocopes.com/cmux/)
- Superset: ELv2, source-available. Has a free tier, about 15.5k stars, and SOC 2 Type II. — [superset.sh](https://superset.sh/)
- Claude Squad: AGPL-3.0, 8.7k stars. — [GitHub](https://github.com/smtg-ai/claude-squad)
- Crystal: MIT, 3.1k stars, deprecated Feb 2026 for Nimbalyst. — [GitHub](https://github.com/stravu/crystal)
- Vibe Kanban: claims 100,000+ PRs created and 30,000+ active users. Now "sunsetting" into community open source. Bloop announced its shutdown on 2026-04-10, Apache 2.0 (secondary). — [vibekanban.com](https://www.vibekanban.com/); [Nimbalyst blog](https://nimbalyst.com/blog/vibe-kanban-after-bloop-whats-next/)
- Sculptor: free in beta (2025). — [Imbue on X](https://x.com/imbue_ai/status/1973513469753434433)
- Jules: free, Pro and Ultra tiers at 15/100/300 tasks per day and 3/15/60 concurrent tasks. — [jules.google](https://jules.google/)
- Devin: self-serve tiers from 2026-04-14 are Free, Pro $20/mo, Max $200/mo, and Teams from $80/mo usage-based. Windsurf was renamed Devin Desktop on 2026-06-02. Cognition ARR was about $492M in May 2026, with estimates of $900M–$1B by Sept 2026 (secondary, unverified). — [Sacra](https://sacra.com/c/cognition/); [tech-insider](https://tech-insider.org/au/windsurf-devin-desktop-vs-cursor-2026/)
- Factory: Pro $20, Plus $100, Max $200/mo, and Teams $60/team + $40/seat (secondary, CostKite Oct 2026). Raised a $150M Series C led by Khosla at about $1.5B (secondary). — [CostKite](https://costkite.com/tools/factory-droids); [tech-insider](https://tech-insider.org/factory-ai-150-million-series-c-khosla-coding-droids-2026/)
- Warp: 800,000+ developers. A case study says cost per PR fell from $80 to $30. — [warp.dev](https://www.warp.dev/)
- Ona: enterprise, contact sales. It is "joining OpenAI" (post dated 2026-06-11), and the homepage says "Part of OpenAI". — [ona.com](https://ona.com/)
- Cursor: reports say agents make 30% of Cursor's own PRs, or that cloud agents merge 56% of PRs. These conflict and are unverified (secondary). — [tech-insider](https://tech-insider.org/cursor-ai-agents-56-percent-merged-prs-2026/)
- Terragon: shut down 2026-02-09. — [terragonlabs.com](https://www.terragonlabs.com/)

### Inferences
- Consolidation in 2026: Ona joined OpenAI, Windsurf folded into Devin, Bloop/Vibe Kanban shut down, Crystal became Nimbalyst, and Terragon shut down in favor of Claude Code on the web. Independent cloud BYO-subscription orchestration did not survive. Free local tools, the multiplexers above all (herdr's 1.6M installs), show the strongest grassroots traction.
- The "software factory" framing (Factory, Warp, Ona, Augment) targets engineering leaders. Local tools pitch to individual flow: "army", "leave them running", "multitasking".

### Gaps
- Conductor pricing and traction aren't on the homepage. Amp's corporate status (spun out of Sourcegraph?) wasn't confirmed on ampcode.com. Codex homepage returned 403, so no verbatim Codex pitch.

## Which are closest to "a CLI your coding agent uses to launch background runs, each Task with its own worktree, branch, PR and persistent conversation; phases that leave artifacts so any agent can continue; context-aware opening in tmux/cmux/Herdr"?

### Takeaway
No single product found matches the whole description. The nearest neighbors each cover one slice. Superset has an agent-scriptable CLI/SDK/MCP plus a worktree per agent, multi-agent, BYO subscription, and remote hosts. Herdr has an agent-facing socket API, agents launching and prompting agents, and persistence. Conductor and Vibe Kanban have a worktree→PR per task, plus a planning board in Vibe Kanban. uzi was a CLI for worktrees plus tmux plus multi-agent, now dormant. Codex CLI is adding managed worktrees and task forking, but only for OpenAI.

### Cited Findings
- Superset: a CLI (`superset new`, `superset ls`), TypeScript SDK and MCP server to script workspaces and agents, a worktree per agent, any CLI agent, BYO keys, remote hosts, and automations that open PRs. — [superset.sh](https://superset.sh/)
- herdr: agents "start and prompt each other, and wait until another agent is blocked" via the same CLI and socket surface; sessions persist across restart; no worktree management. — [herdr.dev](https://herdr.dev/)
- cmux: scriptable CLI and socket that agents can drive, but no worktree or Task model on the homepage. A community poll on native herdr integration shows the two layers converging. — [cmux.com](https://cmux.com/); [cmux Discussion #10106](https://github.com/manaflow-ai/cmux/discussions/10106)
- uzi: CLI, automatic worktrees, tmux per agent, mixed agents per prompt; last updated June 2025. — [GitHub devflowinc/uzi](https://github.com/devflowinc/uzi)
- Claude Squad: TUI over tmux plus worktrees, a background mode, `--autoyes`. — [GitHub](https://github.com/smtg-ai/claude-squad)
- Vibe Kanban: tasks as kanban issues and sub-issues, each in a worktree, with PR creation and agent switching. — [vibekanban.com](https://www.vibekanban.com/); [Hysen Labs review](https://hysenlabs.com/projects/bloopai-vibe-kanban)
- Sculptor: the only local tool found that persists the full session (plans, chats, tool calls), but it is GUI-first and container-based, with history forking still upcoming. — [Imbue Sculptor](https://imbue.com/sculptor/)
- Codex CLI: experimental worktree start and fork (0.154.0), managed worktrees (0.162.0) (secondary). — [ChatGPT & Codex changelog](https://learn.chatgpt.com/docs/changelog)
- Cursor `/goal` plus PR subscription is the closest cloud analog of a long-lived Task that drives itself to a merged PR. — [Cursor changelog](https://cursor.com/changelog/08-19-26)

### Inferences
- The unoccupied combination: (a) the *agent itself* calls the CLI to spawn background runs, as opposed to a person clicking in a GUI; (b) the Task, not the agent process, owns worktree, branch, PR and conversation; (c) phase artifacts are explicitly designed so a different provider can continue; (d) the run opens in whatever multiplexer you're already in. Superset and herdr each have (a). Conductor and Vibe Kanban have part of (b) without a durable conversation. Nobody advertises (c). (d) exists only as each multiplexer's own surface.
- Agent-to-agent handoff/continuation is the clearest gap across both categories. The market handles "continue" as location handoff (Cursor local↔cloud, Amp phone, Superset laptop sleep).

### Gaps
- Conductor's "Dispatcher" (reported June 2026) might be a coordinating layer, but it is unverified from primary sources.
- I didn't check whether Superset's CLI is used by agents themselves (agent-issued) rather than people.
