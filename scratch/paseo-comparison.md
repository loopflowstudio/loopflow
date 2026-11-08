# Paseo and Loopflow

Research · 2026-11-03 · Jack requested this comparison; no Task bound.

Built from Paseo documentation and package manifests; Loopflow README.md, docs/work.md, docs/orchestration.md, docs/ops.md, docs/surfaces.md, swift/README.md, and selected Rust models.

Paseo makes a shared development workspace available across devices. Loopflow makes a Task’s workflow, delivery, and relationship to an ongoing objective durable. Their overlap includes multiple agents, worktrees, review, remote execution, and automation. This is a comparison of documented capabilities and selected source, not a hands-on quality ranking.

```text
Paseo
Project → Workspace → Sessions / terminals / browser / diffs
              ↓
        Directory or worktree
Clients ↔ Daemon → Installed agent providers
             ↑
      Optional Hub triggers and workflows

Loopflow
Repository → optional Wave → Project / Tasks
Task → Checkout + PR + workflow position
     → Sessions + Graph processes + Processes
CLI / macOS / terminal hosts → shared Rust operations and store

Conceptual map; arrows summarize relationships, not database foreign keys.
```

| Dimension | Paseo | Loopflow |
|---|---|---|
| Primary continuity | Workspace and its sessions | Task lifecycle; Wave objective and memory |
| Mobile UI | React Native + Expo, TypeScript | No equivalent mobile client found in reviewed sources |
| Desktop UI | Electron with shared web export | Swift macOS application |
| Automation | Agent delegation, schedules, heartbeats, Hub workflows | Skills and Graphs; Task progression and delivery |
| Extensions | TypeScript UI, provider, and daemon plugins | Markdown Skills, YAML Graphs, CLI and observable Work APIs |

## What “native” means here

Paseo’s mobile app is TypeScript with React Native and Expo. React Native renders platform-native views; “not a WebView” describes that rendering approach, not SwiftUI authorship. Paseo includes Swift native modules for audio. Its app dependencies also include react-native-webview, so the slogan should not be read as a promise that no feature embeds web content. Sources: [app package](https://github.com/getpaseo/paseo/blob/main/packages/app/package.json), [Swift audio modules](https://github.com/getpaseo/paseo/tree/main/packages/expo-two-way-audio/ios), [React Native rendering](https://reactnative.dev/docs/intro-react-native-components).

Paseo Desktop uses Electron. The root build exports the shared Expo app for web and packages it for desktop. Loopflow has a Swift macOS application and Rust core. Sharing the app across platforms plausibly helps Paseo deliver features across devices; that is an architectural inference, not a measured development-speed advantage. Sources: [desktop package](https://github.com/getpaseo/paseo/blob/main/packages/desktop/package.json), [build scripts](https://github.com/getpaseo/paseo/blob/main/package.json), [Loopflow Desktop](../swift/README.md).

## How the work is organized

A Paseo project contains workspaces. Each workspace has a directory and multiple sessions; worktree isolation is optional. Agents, terminals, browsers, and diffs belong together there. Creation of a workspace is separate from starting an agent. This makes the workspace a close comparison to the execution side of a Loopflow Task. [Paseo workspaces](https://paseo.sh/docs/workspaces).

Loopflow additionally gives the Task a recorded workflow position, ownership, tracker relationship, delivery state, and associated history. Its TaskWork model contains multiple Sessions, Graph processes, and Processes alongside the Task’s selected Graph. A Wave supplies the longer-lived objective, memory, and defaults. A Wave is optional, and a Task can exist before it has a checkout. [TaskWork source](../rust/loopflow/src/task_work.rs), [TaskGraph source](../rust/loopflow/src/task_graph.rs), [work guide](../docs/work.md).

## The overlap is substantial

Paseo agents can launch and communicate with other agents through MCP or the CLI, including across providers. Schedules start fresh agents; heartbeats periodically prompt an existing conversation. Its optional Hub routes GitHub, Slack, and Discord activity to daemons, with repository-authored workflows supporting ordered steps, conditions, structured outputs, and deadlines. Sources: [orchestration](https://paseo.sh/docs/orchestration), [schedules](https://paseo.sh/docs/schedules), [Hub](https://paseo.sh/docs/hub), [Hub workflows](https://paseo.sh/docs/hub/workflows).

Loopflow Graphs encode engineering workflows with composition, loops, routing, and explicit steps where Jack participates. Tasks retain their position between runs; delivery operations connect the work to PR checks and landing. The distinction is the Task lifecycle and its durable context, rather than the mere presence of orchestration. [Graphs](../docs/orchestration.md), [delivery operations](../docs/ops.md).

## Where Paseo is especially relevant

Paseo documents desktop, native mobile, web, and CLI clients with device pairing and an optional encrypted relay. Its TypeScript plugins can add UI on connected clients and daemon behavior, including provider integrations and agent lifecycle hooks. That combines a broad client surface with an extension model reaching both UI and execution. [Connectivity](https://paseo.sh/docs/connectivity), [plugins](https://paseo.sh/docs/plugins), [hooks](https://paseo.sh/docs/plugins/hooks).

Loopflow’s current surfaces center on macOS Desktop, terminals, and integrations with existing terminal hosts. Its documented operational scope also includes account-capacity routing, SSH machines, rented compute, and usage attributed to work. These are comparison dimensions to test, not claims that Paseo could never support equivalents. [Surfaces](../docs/surfaces.md), [operations](../docs/ops.md), [usage](../docs/telemetry.md).

## Report card

Jack requested a report card. These are judgments about documented capability coverage and integration, not benchmark results. A means a first-class strength; B means useful support with narrower scope or more assembly. Plus and minus indicate relative depth, not measured precision. The source links elsewhere in this report support the assessment; Hub runtime was not audited.

| Area | Paseo | Loopflow | Basis |
|---|---|---|---|
| Device and platform coverage | A | B− | Paseo has desktop, mobile and web clients. Loopflow has macOS Desktop and macOS/Linux CLI workflows. |
| Workspace tools | A− | B+ | Paseo integrates agents, editing, diffs, terminals, PRs and browsers. Loopflow integrates Task conversations, shells, files, changes and local pages. |
| Provider breadth | A | B+ | Paseo combines built-in adapters, ACP and provider plugins. Loopflow documents Claude Code, Codex, OpenCode and Pi. |
| Agent-to-agent delegation | A | B+ | Paseo exposes worker creation, messaging, completion notifications and subagent UI. Loopflow emphasizes Tasks and Graph execution. |
| Repeatable development workflows | B+ | A | Paseo has delegation tools and optional Hub workflows. Loopflow integrates Graphs, loops, routing and interactive boundaries with Task position. |
| Delivery lifecycle | B | A− | Paseo supports PR-aware workspaces and agent-driven delivery. Loopflow explicitly connects ownership, shipping approval, CI repair and landing. |
| Long-running objectives | B− | A− | Paseo has schedules and heartbeats. Loopflow Waves add persistent purpose, memory, plans and metrics across Tasks. |
| Interface and provider extensions | A | B | Paseo plugins extend clients, providers and daemon hooks. Loopflow has structured reads and pane APIs, but no equivalent plugin surface in the reviewed sources. |
| Authoring team working practices | B+ | A | Paseo uses skills, profiles, plugins and Hub configuration. Loopflow makes Markdown Skills and YAML Graphs central repository artifacts. |
| Operational reliability and polish | Ungraded | Ungraded | Both need the same hands-on exercise to establish recovery quality, performance and interaction quality. |

Paseo earns the stronger assessment as an environment for interacting with agents across devices. Loopflow earns the stronger assessment for organizing repeatable work from an objective through a delivered change. An overall average would silently choose how much each area matters.

## Three lessons for Loopflow

Jack wants device continuity retained as a separate priority, and wants the next comparison focused on workspace setup, Desktop UX, orchestration and settings. Plugins are not a desired direction. The following are proposed experiences, not implemented Loopflow behavior. Priority reflects fit with that direction, rather than a general product ranking.

1. Make each Task checkout ready to run. Paseo offers repository-defined setup/teardown, named scripts and services, port assignment and service URLs; its workspace script controls expose start, stop, restart, terminal and open actions. Loopflow already creates isolated Task worktrees and has shells and a browser. The opportunity is to connect preparation, service state and preview into one visible Task experience. A small version would expose the repository’s existing setup, test and app commands, show setup failures beside their logs, and open the right running app. Prove it by opening two Tasks that run the same app concurrently without manual port edits. This has the strongest concrete capability gap in the reviewed sources. [Paseo worktrees](https://paseo.sh/docs/worktrees) · [Script controls source](https://github.com/getpaseo/paseo/blob/eb8b5b8f9421ded6b0d1237f4ceabffbdab15055/packages/app/src/screens/workspace/scripts/workspace-scripts-button.tsx).

2. Keep delegated work visible beside the originating conversation. Paseo’s Subagents track opens workers as full conversations and retains their parent relationship even when they use another workspace. Loopflow already has Task-associated Sessions, Graph history and supporting panes. A compact Task view could show each collaborator’s assignment, current activity and result, with direct access to its conversation. A bounded review can stay associated with the current Task; an independently delivered change should remain its own Task. Preserve actual control authority separately from this relationship. Prove it by following implementation and review concurrently, redirecting the reviewer, and returning to the main conversation without losing context. [Paseo orchestration](https://paseo.sh/docs/orchestration-workflows) · [Loopflow workspace](../swift/README.md).

3. Make effective agent settings understandable at the point of launch. Paseo profiles bundle provider, model, thinking level, mode and available features, plus notes that help agents choose settings. Loopflow already has repository/Wave/personal defaults and per-Skill agent selection. The first improvement should expose those existing choices: which agent runs each step, where each setting came from, and whether an edit applies once or changes a default. Extend the existing New Task workflow preview with that summary. Named presets would be a later option if they remove repeated choices; they should not introduce competing configuration authority. Prove it by setting one review step to a different provider, understanding its scope, and seeing the effective choice before launching. [Paseo profiles](https://paseo.sh/docs/agent-profiles) · [Loopflow defaults](../docs/work.md#set-it-up-once) · [New Task source](../swift/LoopflowMac/Views/NewTaskSheet.swift).

Generic split panes, file browsing, diff views and PR views are lower-value lessons because Loopflow already documents them. The proposed sequence is ready-to-run workspaces, visible collaborators, then launch settings. This ranking is an assessment from source, pending direct UX trials.

## What scheduling adds to cron

Jack asked what Paseo schedules add on top of “Chrome”; this answer assumes cron was intended. Paseo uses cron timing and adds saved agent configuration and prompts, fresh-agent versus existing-conversation targets, management controls, run records, bounded lifetimes and workspace cleanup. A daemon implements the scheduling; it is not simply a crontab editor. [Schedules](https://paseo.sh/docs/schedules) · [Schedule CLI](https://paseo.sh/docs/schedules-cli).

For Loopflow, the promising part is making recurring work discoverable and editable and connecting each invocation to its result. Existing OS scheduling can supply timing. On macOS, Loopflow documents LaunchAgents for work requiring login-session credentials. This is outside the top three above; no new scheduler is proposed. [Loopflow scheduling](../docs/work.md).

## Evidence limits

No Paseo installation or runtime trial was performed. Mobile reconnect behavior, large transcript performance, crash recovery, and review ergonomics remain untested. No built-in equivalent to the full Wave objective/memory/plan model was found in the reviewed Paseo documentation; plugins or later releases could add one. Source pages describe main/current documentation and may differ from installed releases. The existing Loopflow scratch notes record Desktop failures, so the native Swift architecture alone is not evidence of greater reliability.

## Questions for the next pass

1. How does the same small change feel from workspace creation through review and merge in both products?

2. Which matters most to Jack here: mobile continuity, shared cross-platform UI, extensibility, or durable delivery workflows?

3. How do the products recover after a daemon restart, provider failure, and client disconnect?


