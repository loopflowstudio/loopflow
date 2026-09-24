# Research: starting new work in Loopflow

## System understanding

The user wants one button that creates a worktree and opens `lf design` there. They subsequently asked to study Notion, Superlogical, and HumanLayer before settling the UX. Confirmed launch preference: “Use the configured app or terminal”.

### Architecture and data flow

Loopflow already separates repository navigation, Sessions, embedded terminal panes, and tracked Work. The current new-shell action fills an empty pane or splits the focused pane, then starts a shell at the repository root. Worktree creation is a Rust CLI operation; provider routing belongs to the normal skill launch path. A new entry point should compose those existing capabilities.

`lf design` honors the skill/repository launch configuration. The existing Rust launcher opens a vendor URL for `ide`; `tui` executes the provider in its host terminal, and an unsuccessful app handoff falls back to TUI. Consequently, “honor configuration” cannot be implemented by capturing a subprocess's stdout and assuming every launch is a short operation. It needs a terminal-capable host for TUI and fallback. There is no evidence in this inspected path of a separate external-terminal preference.

### Evidence and counterexamples

- Source: `swift/LoopflowMac/Views/SessionsView.swift`, `swift/Loopflow/Models/MultiplexerStore.swift`, `rust/loopflow/src/lf/commands/ops/mod.rs`, and `rust/loopflow/src/lf/commands/{run,util}.rs`.
- Installed Loopflow 0.12.19 rendered through its built-in `session-fixtures` snapshot mode on 2026-09-23. Image: [Sessions surface](ux-reference-images/loopflow-sessions-fixture.png). This is layout evidence, not a live health/status assessment, nor a screenshot of the user's precise current pane arrangement.
- Direct window capture failed with `could not create image from window`. The successful self-snapshot ran in a separate app process and exited. No existing session was selected, moved, or completed.
- The screenshot places New shell in the empty pane and sidebar footer. The user describes bottom right; treat that as their reported working context, not proof that every app arrangement places the action there.

## Observations from reference products

### Notion: create before organizing

[Official guide](https://www.notion.com/help/create-your-first-page), including its [creation screenshot](ux-reference-images/notion-new-page.png), shows a compose action beside workspace navigation and documents Cmd/Ctrl+N. The new page accepts writing immediately; templates and structure are offered afterward.

Interpretation for Loopflow: a stable New action can inherit the selected repository and lead directly into the conversation. Naming, issue filing, and workflow configuration need not precede the idea. This does not require imitating Notion's document editor.

### Superlogical: sessions and their views have different scopes

[Alasdair Monk's UI demo](https://x.com/almonk/status/2087533118429294920) was accessed through X's public syndication response and its linked video. Inspected frames at 00:15, 00:35, 01:15, 01:50, 02:30, and 03:15. At [00:35](ux-reference-images/superlogical-035.png), a compact session switcher contains a filter/create field, Default, and New Session; terminal tabs and a separate plus remain in the window header. At [01:50](ux-reference-images/superlogical-110.png), the named Website session contains distinct shell and top tabs.

The [company's published direction](https://www.superlogical.com/) describes persistent sessions spanning environments and clients. This is an announced goal, not independently verified production behavior.

Interpretation for Loopflow: distinguish starting new work from opening another terminal view. Keep creation at the level of the work's context. Session naming/grouping is a useful longer-term reference; importing Superlogical's session domain into Loopflow would duplicate existing concepts.

### HumanLayer: make workspace setup and handoff visible

The [official walkthrough](https://docs.humanlayer.com/tutorials/first-session) and [form screenshot](ux-reference-images/humanlayer-new-task.png) expose directory, issue source, description, generated name/slug, workflow, worktree timing, and agent settings. Setup progress precedes the task page and first session. The [product page](https://www.humanlayer.dev/) illustrates tasks grouping sessions and artifacts, including design discussions.

Interpretation for Loopflow: borrow a coherent task-to-workspace-to-conversation transition and visible progress. The full configuration form conflicts with the requested one-click path; use the already-selected repository and existing launch defaults. These are official screenshots and descriptions, not a hands-on authenticated HumanLayer trial.

## Tensions

- **Creation versus terminal management.** The current prominent verb offers a shell; the desired user outcome is a design conversation in an isolated checkout.
- **One click versus useful naming.** An automatic checkout name preserves immediate entry. A descriptive name before launch requires another interaction. HumanLayer resolves this through an initial description form; Loopflow can let the conversation discover the name later.
- **Familiar location versus navigational consistency.** A footer keeps the action near the position the user described. A compose action beside repository navigation communicates what context it inherits and remains stable as panes change.
- **Everyday task language versus tracked Task identity.** “New task” is a user-facing proposal; creating an unbound design must not silently mint a PM Task or a second durable task model.
- **Configured destination versus a uniform app experience.** External handoff is intentional. Loopflow must report it clearly and avoid forcing an embedded conversation for visual consistency.

## Quality and potential

In the captured Sessions surface, two burgundy header bands dominate the top while the empty pane centers a large “No session open” message. A modest New shell action appears below, and bare terminal management is also offered at the bottom of the sidebar. This hierarchy puts an absent session ahead of the opportunity to begin work.

The app already has durable sessions, terminal surfaces, repository context, and a launcher. These reduce implementation scope. The difficult boundary is correct launch ownership and recovery, especially when app opening falls back to TUI; it is not drawing the button.

## Recommendations

1. **Use one stable creation action and a shortcut.** Compare repository-header placement with the bottom-right footer. Cost: small UI wiring; shortcut must coexist with standard New Window behavior. Benefit: creation stays discoverable across pane states.
2. **Open first; organize in the conversation.** Generate the checkout name, then run plain `lf design`. Cost: preserve launch/checkout identity across retries. Benefit: fulfills the original one-click request without a PM or naming form.
3. **Show real setup and handoff progress.** Creating workspace → opening design → handed off, or an actionable error. Cost: bind feedback to actual receipts. Benefit: avoids duplicate clicks and ambiguous apparent success. App-open success does not prove the agent is interactive.
4. **Separate larger navigation changes from this change.** Conversation grouping, artifact browsing, and header consolidation may be worthwhile follow-ups, but this research does not establish their exact scope.

## Review artifact

[Interactive comparison](new-task-ux.html): A places New task beside the repository; B places it in a fixed bottom-right footer. Clicking either simulates setup and configured-destination handoff. Screenshots and source links sit below the sketch. It creates nothing. The simplified header, example conversation titles, and layout are proposals, not existing implementation.

Rendered using `lf screenshot` and visually inspected at 1440×1100. This is visual QA, not proof of the native launch path.

## Open questions

- Final placement and label: repository header versus bottom-right footer; New task versus New design.
- The exact Wave/Project owning implementation remains unspecified.
- A later conversation-derived rename or artifact view needs its own design if selected; neither is required to deliver the initial button.
