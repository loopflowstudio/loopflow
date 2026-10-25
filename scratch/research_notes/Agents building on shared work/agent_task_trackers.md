# Agent task trackers and planning tools (landscape as of 2026-10-24)

Scope: tools where coding agents coordinate through a shared work list, plan, or spec.
Star counts were read from GitHub pages on 2026-10-24 unless marked otherwise. [2025] marks older
or unverified-in-2026 details. "Secondary" marks claims from blogs or aggregators, not the vendor.

## Summary table

| Tool | Shared substrate | Claim / handoff | Phases + artifacts | License / price | Traction (Oct 2026) |
|---|---|---|---|---|---|
| Beads (`bd`) | Dolt DB in `.beads/`, synced to git remote `refs/dolt/data`; JSONL is export only | `bd ready` -> `bd update <id> --claim` (assignee + in_progress); `bd prime` re-injects context | No phases; dependency graph, epics, molecules/formulas (workflow templates) | MIT, free | 28.2k stars; v1.3.2 released 2026-10-24 |
| Gas Town (`gt`) | Beads ledger + git worktrees ("hooks") + branches | Mayor `gt sling <bead> <rig>` puts work on a polecat's hook; `gt done` pushes branch + merge-request bead; Refinery merge queue | Molecules = multi-step workflow templates | MIT, free | 18.5k stars; 1.0 on 2026-04-03; successor Gas City (alpha) |
| Backlog.md | Markdown task files in `backlog/` in the repo | Status changes; branch/worktree-aware IDs; MCP | spec -> plan -> code review checkpoints; DoD lists; docs/decisions | MIT, free | 7.1k stars; v1.55.3 (2026-10-23) |
| Task Master | `tasks.json` + generated task files in `.taskmaster/`; tags as workstreams | `task-master next`; status field | PRD -> parse-prd -> tasks/subtasks -> expand; research | MIT + Commons Clause | 28.3k stars; company pivoted to Hamster (SaaS) |
| Hamster | SaaS Briefs/Plans/Tasks synced into the repo by CLI/MCP | Agent follows the Plan; team reviews | Brief -> Plan -> Tasks with acceptance criteria | Free (10 Briefs/mo); Team $40/creator/mo | n/a |
| Claude Code Tasks | `~/.claude/tasks/<list-id>/` | `CLAUDE_CODE_TASK_LIST_ID=name` shares a list across sessions; dependencies (A blocks B) | None | Bundled in Claude Code | Built-in |
| Linear for Agents / Linear Agent | SaaS issues | Assign -> agent becomes `delegate`, human stays `assignee`; AgentSession with activities | Coding sessions produce diffs/PRs; Loops (recurring) | Agents not billable seats; coding sessions need AI credits | Many partner agents |
| GitHub Copilot cloud agent + Agent HQ | GitHub Issues/PRs | Assign issue (or @mention) to Copilot, Claude, Codex; one branch + one PR per task | Plan -> implement -> PR; session logs | Premium requests; Pro/Pro+/Business/Enterprise | GA (Copilot), Claude/Codex preview |
| Jira agents | Jira work items | Assignee dropdown -> Rovo or third-party agents (Copilot, Cursor, Claude, Codex) | Agents on workflow transitions/columns | Jira Cloud Standard+ | GA May 2026 |
| GitHub Spec Kit | `specs/NNN-feature/` in repo + `memory/constitution.md`; git branch per feature | Next agent reads the spec folder | constitution -> specify -> plan -> tasks -> implement (-> converge) | MIT, free | 142.7k stars |
| OpenSpec | `openspec/specs/` + `openspec/changes/<id>/` | Change folder is the handoff | propose -> apply -> archive | MIT, free | 72.9k stars |
| Kiro specs | requirements/bugfix.md, design.md, tasks.md per spec | Tasks run singly or in dependency "waves" | requirements -> design -> tasks | Paid IDE tiers | AWS-backed, GA Nov 2025 |
| BMAD Method | Planning docs + story files in repo | Fresh chats pick up story files [2025] | Clarify -> Plan -> Build and verify -> Learn | MIT, free | 54.5k stars; v6.12, v7 alpha |
| MCP Agent Mail | Markdown messages in a per-project git repo + SQLite | Inboxes, threads, advisory file leases; pairs with Beads | None | Open source | 2.2k stars; Python version deprecated for Rust |
| Vibe Kanban | Kanban + workspace (branch, terminal, dev server) per task | UI-driven | None | Apache-2.0 | 28.4k stars; company shut down 2026-04-10 |
| Traycer | Desktop workspace: shared filesystem, artifacts, history per task | Agents message each other, hand off across chats | Plan / Execute / Review skills | Open source; Sync $16-20/user/mo | 11k+ devs |

## Beads (`bd`) — Steve Yegge, now gastownhall/beads

- Taglines (README, 2026): "Beads - A memory upgrade for your coding agent"; "Distributed graph issue tracker for AI agents, powered by Dolt."
  The earlier tagline was "git-backed" (secondary).
- Launch: "Introducing Beads: A coding agent memory system", Medium, 2025-10-13 [2025]. Yegge's framing
  (via secondary sources): markdown plan files fail because agents only know "what's on disk" and with
  conflicting docs they "get dementia"; Beads is "orchestration for what you're working on today and this week."
- 1.0 post (2026-04-03, "Gas Town: from Clown Show to v1.0"): Beads is "a drop-in, generic, unopinionated
  memory system and knowledge graph for coding agents"; Beads is "the Why" next to Git's what/where/who/how;
  heading toward "a universal ledger for all knowledge work." "Beads is the secret sauce that makes Gas Town and
  Gas City both possible and best-of-class." The SQLite + JSONL architecture was removed and replaced by Dolt.
- Storage: Dolt, either embedded (`.beads/embeddeddolt/`) or server mode. `.beads/issues.jsonl` "is an export for
  viewers and interchange, not the source of truth or a backup." Sync: `bd dolt push/pull` to `refs/dolt/data`
  on the git remote. An experimental shared `bd serve` HTTP backend also exists.
- Work model: hash IDs (`bd-a1b2`) chosen to "prevent merge collisions in multi-agent/multi-branch workflows".
  Hierarchical IDs for epic/task/subtask (`bd-a3f8.1.1`). Links: blocks, related, parent-child, duplicates,
  supersedes, replies-to. Closing an issue unblocks its dependents.
- Claim/resume: `bd ready` lists unblocked work. `bd update <id> --claim` atomically "sets assignee + in_progress".
  `bd prime` injects workflow context and memories, and `bd remember` stores them. A "message" issue type has threads.
  "memory decay" summarizes old closed issues to save context.
- Phases: none built in. Molecules/protomolecules/formulas (`.beads/formulas`) are workflow-graph templates with
  gates (secondary). The tool records the work but not the design doc or PR.
- Traction: 28.2k stars (Oct 2026). It had >20k at 5 months (Apr 2026, per Yegge) and ~17.9k in Mar 2026 (secondary).
  Release v1.3.2 shipped 2026-10-24 (v1.3.1 on 2026-09-30). It has a Rust port (beads_rust), community UIs, and Jira sync.
  Per Yegge, the codebase is "100% vibe coded" at ~130k lines of Go.
- Influence: widely described as the inspiration for Claude Code Tasks (third-party claim, not Anthropic's).

## Gas Town / Gas City (Yegge)

- Tagline: "Multi-agent orchestration system for Claude Code, GitHub Copilot, and other AI agents with persistent
  work tracking". MIT, 18.5k stars. Version 1.0.0 shipped 2026-04-03 and is now in low-churn upkeep. Successor Gas City
  ("SDK", announced ~2026-04-24; MEOW = "Molecular Expression of Work") is in alpha.
- Nouns: Mayor ("Your primary AI coordinator"), Rigs (one per git repo), Polecats (workers with "persistent identity
  but ephemeral sessions"), Convoys (bundles of beads), Hooks ("Git worktree-based persistent storage for agent work.
  Survives crashes and restarts."), Molecules (workflow templates), Refinery (per-rig bisecting merge queue,
  "polecats never push directly to main"), Witness (supervisor), `gt seance` (find predecessor sessions via `.events.jsonl`).
- Flow: `gt sling <bead-id> <rig>` puts the work on a polecat's hook, and the polecat works in a worktree. `gt done`
  pushes its branch and opens a merge-request bead, and the Refinery gates and merges it. Cross-town work goes
  through "Wasteland" (`gt wl claim`).
- Relevance: of the open tools, this is the closest to "one worker + worktree + branch per unit of work, durable
  across sessions." It optimizes for swarm throughput and a merge queue, not for a reviewed PR per task, and it has
  no design-doc phase.

## Backlog.md (MrLesk)

- Tagline: "Markdown‑native Task Manager & Kanban visualizer for any Git repository". MIT, 7.1k stars, v1.55.3 (2026-10-23).
- Storage: one markdown file per task in `backlog/` (configurable). Fields: title, description, acceptance criteria,
  milestones, dependencies, status, Definition of Done. "tasks are plain files in your repo". Git is optional.
- Agent loop: `backlog instructions overview`. The recommended workflow has spec, plan, and code review checkpoints,
  summed up as "one task = one context window = one PR." That is the clearest one-task-one-PR statement in this category.
- Multi-branch: IDs are released only "once no other branch or worktree still holds the task". `checkActiveBranches`
  and `activeBranchDays` give cross-branch accuracy. MCP server (`backlog mcp start`), local web Kanban
  (`backlog browser`), docs/decisions/drafts. Companion tool Groma.md shows which agent is busy on which component.
- No formal claim primitive. Status/assignee edits in files are the handoff.

## Task Master AI (eyaltoledano/claude-task-master) and Hamster

- Tagline: "An AI-powered task-management system you can drop into Cursor, Lovable, Windsurf, Roo, and others."
  28.3k stars. License "MIT License with Commons Clause" (no selling or hosting it).
- Storage: `tasks.json` (fields id, status, dependencies, subtasks, details, testStrategy) plus generated task files,
  under `.taskmaster/`. PRDs live in `.taskmaster/docs/prd.txt`. Tags act as workstreams (`--from-tag/--to-tag`).
- Flow: `task-master parse-prd` -> `next` -> expand into subtasks -> `research` (Perplexity optional). The docs
  mention "Team Collaboration" and a "Loop Command" but give no details.
- Single-agent PRD decomposition. Nothing claims a task, and dependency order decides the next task.
- Hamster (tryhamster.com, the same team's commercial product): "Hamster helps your team turn goals into delivered
  work." / "Hamster proposes strategies, turns them into initiatives, and carries the work through to delivery. Your
  team steers every step." Brief -> Plan -> Tasks with acceptance criteria. The CLI syncs Briefs/Plans/Methods into the
  repo for Claude Code/Cursor/Codex, with MCP access. Free (10 Briefs/mo, unlimited viewers); Team $40/creator/mo with cloud delivery.

## Claude Code Tasks (Anthropic)

- Built-in task list (v2.1.16 introduced dependencies, secondary). Docs: "Tasks persist across context compactions";
  "To share a task list across sessions, set `CLAUDE_CODE_TASK_LIST_ID` to use a named directory in
  `~/.claude/tasks/`". Dependencies mean task A can block task B.
- The list is keyed by ID with no repo awareness, so the same ID in two repos crosses wires (secondary). It is a
  session-level checklist and keeps no artifacts.

## Linear (Linear for Agents -> Linear Agent + coding sessions)

- Agents page: "Artificial colleagues. Natural collaboration." / "Build and deploy AI agents that work alongside you
  as teammates." / "Agents are full members of your Linear workspace."
- Linear for Agents launched 2025-05-20 [2025]. Agent interaction guidelines and SDK followed 2025-07-30 [2025].
- Delegation model (developer docs): assigning an issue to an agent sets it as the "`delegate`, not the `assignee`",
  so humans keep ownership. "Sessions are created automatically when an agent is mentioned or delegated an issue."
  AgentSession webhooks; activities (thought, etc.) drive the visible session state; the first thought is due within 10s.
  "Agents are not counted as billable seats in Linear."
- Partner agents: Cursor, OpenAI Codex, Devin, Sentry, ChatPRD, Oz by Warp, Factory, Charlie, Ranger, Tembo. Since
  2026-07-30, "GitHub Copilot users can assign issues directly to Copilot's cloud agent from Linear."
- 2026: Linear moved from tracker to agent itself.
  - Linear Agent: beta Q1 2026, GA on Free 2026-06-22 (pricingsaas tracker).
  - Coding Sessions: beta 2026-06-15 (requires AI credits), GA 2026-07-13.
  - Changelog quotes: "Coding sessions are the shortest path from issue to diff." (2026-09-24, with adaptive model
    routing and open-weight models like GLM); "Linear Agent can now set up, run, and test your code before returning
    its work." (2026-08-20, with browser use and before/after screenshots); "Diffs now update in realtime throughout
    a Linear coding session." (2026-10-08).
  - Loops ("recurring agent workflows for teams", 2026-07-20 onward; 2026-10-22: triggered from Slack/GitHub events).
  - Project composer: "develop ideas with Linear Agent before creating a live project" (2026-09-03).
- The issue is the shared object, and the PR is produced by the agent. Linear holds the plan and the git artifacts
  live elsewhere. One delegate per issue session.

## GitHub: Copilot cloud agent, Agent HQ, Issues

- Copilot coding agent, renamed "Copilot cloud agent" in the docs, GA ~March 2026 (secondary). It works "in an
  ephemeral cloud development environment" and handles the branch, commits, and push. "Copilot can only work on one
  branch at a time and can open one pull request per task." Sessions are capped at 59 minutes. "Session logs show the
  work and tools used." It can start from issues, PRs, the agents panel, or automations triggered by events such as
  "an issue being opened." The `copilot/` branch + draft PR + iterate-by-PR-comment flow dates from 2025 [2025].
- Agent HQ: Claude and Codex were assignable to issues alongside Copilot from 2026-02-04 (Pro+/Enterprise preview),
  and from 2026-02-26 for Business/Pro. "Each coding agent session consumes one premium request during public
  preview." The Agent Control Plane is GA. You can assign one issue to several agents to compare results. GA of
  third-party agents is unconfirmed as of Oct 2026.
- Shared state: the Issue (intent), branch + PR (output), session log (process). There is no design-doc phase,
  though there is a planning step.

## Jira (Atlassian) — agents in Jira

- GA May 2026 for Jira Cloud Standard/Premium/Enterprise after a March beta. Agents are assigned from the assignee
  dropdown: Rovo agents, the native "Jira Coding Agent", or third-party agents (GitHub Copilot, Cursor, Claude, Codex).
  @mention also works. Admins can attach agents to workflow transitions or board columns, so a board column becomes
  a phase. Agent output stays private until the user shares it.

## Spec-driven tools

### GitHub Spec Kit
- Tagline (2026 README): "Build with a spec, fix a bug, or assess an idea — with your coding agent." MIT, 142.7k stars
  (>115k by June 2026, secondary).
- Commands: `/speckit-constitution`, `-specify`, `-plan`, `-tasks`, `-implement`, `-converge`, plus clarify, checklists,
  and analyze. "Create a constitution once per project and repeat the remaining steps for each feature." "Run these
  skills in your agent's chat ... one at a time, reviewing each result before continuing."
- Artifacts: `memory/constitution.md`; per feature `specs/NNN-feature/` with spec.md, plan.md, research.md,
  data-model.md, contracts/, quickstart.md, tasks.md. `/speckit.specify` "Generates a semantic branch name from your
  description and creates it automatically" (e.g. `003-chat-system`). Bug and assessment work goes in `.specify/bugs/`
  and `.specify/assessments/`.
- Doctrine: "Specifications don't serve code—code serves specifications." "The specification becomes the primary artifact."
- This is the closest of the spec tools to phases that leave git artifacts with a branch per feature. Nothing claims
  or owns a feature, and Spec Kit does not open the PR.

### OpenSpec (Fission-AI)
- Tagline: "The most loved spec framework." MIT, 72.9k stars, works with "30+ AI assistants via slash commands."
- "AI coding assistants are powerful but unpredictable when requirements live only in chat history." "OpenSpec adds
  a lightweight spec layer so you agree on what to build before any code is written." "built for brownfield not just
  greenfield". "fluid not rigid," "iterative not waterfall."
- Artifacts: living specs in `openspec/specs/`. Each change gets `openspec/changes/<id>/` with proposal.md, design.md,
  tasks.md, and spec deltas ("## ADDED Requirements" with WHEN/THEN scenarios). Archive moves the change to
  `changes/archive/<date>-<id>/` and merges its deltas into the specs.
- Commands: `/opsx:explore`, `:propose`, `:apply`, `:archive`; expanded profile adds `:new`, `:continue`, `:ff`, `:verify`.
- The change folder is a durable per-unit-of-work artifact that any agent can `continue`. It is not tied to a branch or PR.

### Kiro specs (AWS)
- Each spec produces requirements.md (or bugfix.md), design.md, and tasks.md. Workflow: requirements -> design -> tasks.
  Feature specs support Requirements-First or Design-First, Bugfix specs fix "surgically while preventing regressions",
  and Quick Spec skips approval gates.
- 2026: it builds a dependency graph from tasks.md and runs independent tasks concurrently in "waves." Markdown Review
  lets you comment on passages.
- Files live in `.kiro/specs/<name>/`, requirements use EARS notation, and steering/hooks exist [2025].
  The IDE has been GA since Nov 2025, with paid tiers up to ~$200/mo (secondary).

### BMAD Method
- Tagline: "Agile Ai Driven Development — turn an idea or change request into working software without giving up the
  thinking." MIT, 54.5k stars, stable v6.12, v7 alpha (ETA 2026-10-16). "Decisions stay explicit, context carries
  forward, and the process sizes itself to the work." "Expect breaking changes between updates." "free and open
  source, with no paywalled workflows".
- v7 loop: Clarify -> Plan -> Build and verify -> Learn and adjust. "Every path runs the same loop." Skills: `bmad`,
  `bmad-build`, `bmad-project-context`, and the PO persona Sofia (`bmad-agent-po`). The docs cover "Break Work into
  Stories and Track It", "Autonomous Development Loops", and "Set Up the Ticket Tree".
- Classic v4–v6 model [2025]: personas Analyst -> PM (PRD) -> Architect (architecture) -> Scrum Master (sharded epic ->
  story files with embedded context) -> Dev -> QA. Each story file is the handoff to a fresh dev chat. This is a strong
  example of phases leaving durable markdown artifacts, though branch and PR ownership are not part of the method.

### Others in the spec layer
- Agent OS (buildermethods): "Agents that build the way you would". MIT, 5.5k stars. Standards discovery/injection
  plus spec shaping.
- Tessl: spec-as-source platform plus registry, still early/closed beta (secondary).
- Traycer: "The Nerve Center for Agentic Coding"; "Open-source agent development environment for Claude Code, Codex,
  Cursor and more." Each task gets a shared filesystem, artifacts, and history. Agents message each other and "hand
  off work across separate chats". "11,000+" developers, "40,000+" tasks. Free; Sync $16–20/user/mo. Formerly a VS
  Code Plan -> Execute -> Verify layer (100K+ users claimed, secondary).

## Coordination layers and worktree runners

- MCP Agent Mail (Jeffrey Emanuel, Dicklesworthstone/mcp_agent_mail): "Asynchronous coordination layer for AI coding
  agents"; "It's like gmail for your coding agents!" ~2.2k stars. The Python version is no longer maintained (moved to
  Rust). Agents get identities, inboxes, threads, and advisory file leases. Messages are markdown in a per-project
  git repo, with SQLite FTS5 for search. "Beads owns task prioritization; Agent Mail carries the conversations and
  artifacts." Shared `bd-123` IDs tie tasks, threads, and commits together.
- Vibe Kanban (BloopAI): "Get 10X more out of Claude Code, Codex or any coding agent". Apache-2.0, 28.4k stars. Each
  workspace gets a branch, terminal, and dev server, and PRs are opened from the UI. Bloop shut down 2026-04-10. Hosted
  services (kanban issues, orgs) were removed after 30 days, and the project continues as community open source with
  local-only use.
- Other worktree-per-task runners (Conductor, Crystal, Sculptor, cmux-type tools) isolate parallel agents but keep
  no shared plan. They were not researched in depth.

## Patterns across the field

1. The shared object splits into three camps:
   - Issue graph: Beads, Task Master, Claude Code Tasks, Linear/Jira/GitHub Issues.
   - Spec folder: Spec Kit, OpenSpec, Kiro, BMAD.
   - Messages/workspace: Agent Mail, Traycer.

   No single tool binds all three (intent, plan artifact, git output) to one owned unit.
2. Claiming is explicit only in Beads (`--claim`), Gas Town (sling/hook), and the SaaS trackers (assignee/delegate).
   Spec tools have no owner concept, and the folder is the handoff.
3. Phases with artifacts belong to the spec tools, and they mostly stop at tasks.md. Spec Kit adds a branch.
   Copilot, Linear coding sessions, and Gas Town end at a branch/PR/merge queue but skip the design doc.
4. The 2026 trend is that trackers became agents. Linear Agent writes code (coding sessions GA July 2026), Jira has a
   native coding agent, and GitHub lets you assign issues to Copilot/Claude/Codex. Meanwhile local trackers moved off
   flat files: Beads switched from JSONL+SQLite to Dolt.
5. Volatility: Vibe Kanban's company shut down (Apr 2026), Task Master's team built a SaaS (Hamster), MCP Agent Mail
   rewrote in Rust, and BMAD warns of breaking changes.

## Closest to "phases that each leave durable git artifacts (design doc, branch, commits, PR), one owner/worktree/PR per task"

- Gas Town is closest on mechanics: a persistent worker identity per bead, a git worktree hook, a branch, and a
  merge-request bead into the Refinery queue. It is swarm-oriented: the merge queue lands to main without a reviewed PR
  per task, and it has no design-doc phase. Its molecules give phases as workflow templates, not as artifacts in git.
- Spec Kit is closest on artifacts: a numbered spec folder (spec, plan, research, tasks) committed on an auto-created
  feature branch. It has no claim/owner and no PR step, and a person drives each phase by hand in chat.
- Backlog.md is closest in stated doctrine: "one task = one context window = one PR" with spec, plan, and code review
  checkpoints and branch/worktree-aware task IDs. Its artifacts are a task file plus docs/decisions, and it does not
  manage the worktree or PR.
- GitHub Copilot cloud agent is closest on one-task-one-PR in SaaS: "one branch at a time and ... one pull request per
  task", from an assigned issue, with session logs. It has no durable design-doc phase, and the task lives in Issues,
  not git.
- OpenSpec has the best per-change artifact lifecycle (proposal/design/tasks -> archive merges into living specs),
  with no ownership or branch binding.

None of these combines all of the following: an owned Task with a single worktree and PR, a design doc committed as
a phase artifact, and resumable phases where a later agent builds on an earlier one's git output.

## Sources

- https://github.com/steveyegge/beads (redirects to gastownhall/beads); https://github.com/gastownhall/beads/releases
- https://steve-yegge.medium.com/introducing-beads-a-coding-agent-memory-system-637d7d92514a (403 on fetch; quotes via secondary)
- https://drss.io/feed/npub1na2ppcvpx7m7dg4d2rqelpq4xdj9z40cxu3avh4qcdq8dtupdcmqahmfge/gas-town-from-clown-show-to-v10 (Yegge, 2026-04-03)
- https://www.morphllm.com/beads-agent-memory ; https://paddo.dev/blog/from-beads-to-tasks/ (secondary)
- https://github.com/steveyegge/gastown ; https://www.dsebastien.net/gas-city/
- https://github.com/MrLesk/Backlog.md (+ /releases)
- https://github.com/eyaltoledano/claude-task-master ; https://docs.task-master.dev/capabilities/task-structure ; https://www.tryhamster.com/
- https://code.claude.com/docs/en/interactive-mode (Task list section)
- https://linear.app/agents ; https://linear.app/docs/agents-in-linear ; https://linear.app/developers/agents ; https://linear.app/changelog ;
  https://linear.app/changelog/2025-05-20-linear-for-agents ; https://pricingsaas.com/companies/linear/diffs/2026W26
- https://docs.github.com/en/copilot/concepts/agents/coding-agent/about-coding-agent ;
  https://github.blog/changelog/2026-02-26-claude-and-codex-now-available-for-copilot-business-pro-users/
- https://support.atlassian.com/jira-software-cloud/docs/collaborate-on-work-items-with-ai-agents/ ; https://community.atlassian.com/forums/Jira-articles/Introducing-Agents-in-Jira/bc-p/3253422
- https://github.com/github/spec-kit ; https://github.com/github/spec-kit/blob/main/spec-driven.md
- https://github.com/Fission-AI/OpenSpec ; https://kiro.dev/docs/specs/
- https://github.com/bmad-code-org/BMAD-METHOD ; https://docs.bmad-method.org/
- https://github.com/buildermethods/agent-os ; https://traycer.ai/
- https://github.com/Dicklesworthstone/mcp_agent_mail ; https://github.com/BloopAI/vibe-kanban ; https://vibekanban.com/blog/shutdown
- https://codemyspec.com/blog/spec-driven-development ; https://ssojet.com/blog/best-spec-driven-development-tools (vendor-authored comparisons)
