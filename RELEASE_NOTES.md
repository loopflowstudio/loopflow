# v0.12.29

v0.12.29 gives conversations and Flows durable records so reconnecting preserves conversation identity and resuming a Flow uses its captured progress. Task and taskless execution now share one driver, with retries retaining the conversation and history keeping failed-turn evidence. Upgrade for clearer recovery and discoverable history, but review the cutover limits below before converting an existing installation.

## Pick up the same conversation

Conversations now retain their identity independently of the command process or client driving them. Discovery and history make it easier to find prior work and understand what happened across reconnects.

- Find conversations with `lf session list`, name one with `lf session rename SESSION "Review notes"`, and reconnect with `lf session connect SESSION`. Read its recorded history with `lf session history SESSION --json`.
- Codex clients can reconnect without restarting the live engine. Feedback, failed-turn evidence, and original usage attribution are retained.
- Sessions can be permanently bound to Tasks, including completed Tasks. Desktop preserves terminals and drafts through discovery and pane replacement.

## Resume Flows from recorded progress

Task and taskless Flows use the same driver and captured progression. A retry keeps the conversation, while only the selected successful native completion advances the Flow.

- Skills and mechanical steps execute through ordinary commands. Loop passes remain positions within one FlowSession.
- Indexed discovery covers Sessions, command executions, and saved Flows.
- SQLite records give each kind of history an explicit owner: Exec records an actual `lf` process, AgentSession owns a conversation, and FlowSession owns a started Flow. These replace the previous Run owner.

## Carry active work into the next plan

Linear Project status now owns chapter planning and history, and Projects supply the default Flow. Plan rotation preserves started work while limiting retirement to backlog proven untouched.

- Started Tasks survive rotation.
- Backlog with uncertain evidence is not automatically retired.

## Operational notes

- **Cutover:** historical import and intermediate-schema compatibility have been removed. Finished history is not imported. Installed conversion still requires a consistent database/filesystem backup and rehearsal of current-state retention; see the [cutover status](docs/architecture-reference.md#cutover-status).
- **Acceptance still open:** configured provider, Desktop, and Linear acceptance remain pending. The Session commands above are a suggested walkthrough, not evidence of completed acceptance.
- **Validation:** recorded gate evidence includes passing Python and website suites, formatting, and all-target Clippy. Full Rust and Swift runs had failures followed by focused repair checks; those repairs do not establish a clean full run of the final head. See the [retained gate evidence](wave/infrastructure/MEMORY.md#data-model-and-performance-decisions-reconciled-2026-09-30).
