# v0.13.10

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.13.10 keeps conversations usable as Tasks move through execution, delivery and recovery. Interactive Flows carry the native agent conversation into each step, while Task decisions no longer depend on resolving stale execution records. Named remote Machines and resident provider logins make work on another computer easier to start, and settled installs begin removing superseded binaries and app bundles.

## Keep talking while work advances

Flow steps now preserve the caller's interactive, batch or TUI mode instead of forcing skills into headless execution. Claude and Codex also receive assembled context through a system instructions file, removing large Wave memory and scratch notes from command-line arguments.

- Run `lf -i -a claude run my-flow` to answer in each skill's native conversation. A successful exit advances to the next step; an interrupted provider exit stops the Flow. An attached terminal selects interactive mode when neither mode flag is supplied.
- Context files carry skills, Task briefs and the current request across terminal, headless and persistent launches, preserving source boundaries and budget accounting. Files remain available for native resume.
- Task conversations retain provider identity and observed launch/exit evidence, allowing the same Session to reopen after a failed spawn or a terminal exit without history. Unknown legacy providers and active owners remain protected.
- Desktop reads validated terminal Program Status reports and displays state, progress and messages in panes and the workspace header. Blocked reports and interactive idle feed the shared Waiting state, including `lf session list --waiting`; reports never complete work.

Jack Heart observed second-step native conversations in cmux and herdr, shell return in herdr, and Claude reading supplied context. Codex context readback, native resume, Claude plan mode and cmux final exit remain unverified; the recorded Codex interactive attempt encountered a duplicate wrapper flag. Program Status requires Desktop to observe a reporting terminal; detached observation and Loopflow's own status emission are not included.

## Manage Task position and delivery independently

Workflow selection, Task position and Flow execution now have separate catalogs and controls. Completion and cancellation record the Task decision without requiring stale Session inputs or unknown historical Process exits to be acknowledged individually.

- Use `lf project workflow list/show/set/customize` to manage definitions and Project selection. Flow and Workflow definitions with the same name remain distinct in discovery, Desktop search, inspectors and source editing.
- Use `lf task workflow show` to inspect the captured graph. `lf task workflow restart` moves to `start` without reloading the graph or starting execution; history remains intact. Changing a Project's selection leaves existing captured Task Workflows unchanged.
- Delivery reconciliation completes Tasks by default after verifying a merged PR. Explicit remaining work and separately started or committed next-PR work keep the Task open; overdue evidence checks remain unresolved.
- Terminal or canceling Tasks no longer receive advice to resume after a parent PR is abandoned. Historical uncertainty and live Process controls remain intact, and cleanup may retain the checkout independently of the recorded decision.
- Completed Flow processes retain their graphs. Successful steps no longer appear failed merely because their driver stopped.

Installed recovery of legacy Sessions and settlement of previously blocked Tasks still require acceptance on a published repair. These changes preserve historical uncertainty rather than inventing missing provider exits.

## Run work on a named remote Machine

Save a remote connection once, then use its name to run commands in the saved repository. Provider logins can now reside on the target, allowing later commands to authenticate without a foreground laptop credential broker.

- Register an existing SSH destination and checkout with `lf machine add mini --repo '~/src/project'`. Inspect, rename or remove the saved connection without deleting remote work or Machine history.
- Run commands such as `lf --machine mini session list`. Task, worktree, Wave and account selectors resolve on the target; remote exit codes reach the caller and local Process history.
- Use `lf machine connect mini codex work@example.com` to authorize a login from the laptop. Claude, Codex and Linear create fresh logins; GitHub installs the laptop's selected `gh` token. Credentials transfer through SSH stdin after host and Machine identity checks.
- Account-selected remote launches use the selected identities and connect missing logins when foreground authorization is available. Headless callers receive a connect command when authorization is needed.

SSH access, a trusted host key and an existing remote repository are prerequisites. Remote accounts retain refresh authority; other services need credentials configured on the target. Task transfer, repository cloning, detached process survival and reattachment are not included. A real Codex demonstration on isolated source deployments established remote login reuse; other providers, later refresh independence and default installed-runtime acceptance remain unverified.

## Operational notes

This release changes command paths and JSON vocabulary. Update scripts and consumers to the final interfaces:

- Select the harness with `--agent` / `-a`, replacing Loopflow's `--model` / `-m`. Native provider model flags and the `agent` configuration key remain. The `--ide` launch path and `session.launch` are removed; arbitrary third-party skill discovery and native dispatch remain unfinished.
- Installation maintenance and skill export live under `lf self`; `lf install` and `lf doctor` remain shortcuts. Display-name lookup is `lf config user`, with `lf user` retained. Use `lf open` instead of `lf desktop`.
- Execution locations are Machines. Replace `lf home` and the interim `lf installation` paths; use `--machine` instead of `lf ssh` or `machine observe`. `lf screenshot` is removed. Retired command paths have no compatibility aliases.
- Recorded commands are Processes, with durable `lfid` separate from optional Unix `pid`. JSON references use `process_lfid` and `parent_process_lfid`; inherited identity uses `LF_PROCESS_LFID`. Historical records retain `pid: null`. Public wire aliases are not retained.
- Flow inspection uses `--processes`; Project Workflow selection uses `lf project workflow set`. Task projections replace the combined `flow` payload with separate Workflow selection, latest Flow process, execution evidence and run controls.
- SQLite renames preserve stored identities, ancestry, outcomes and history, including opaque `home_…` Machine IDs. `LF_HOME`, installation paths and released receipt formats remain intact. Installed migration and recovery are not established by the supplied evidence.

Settled installs now remove superseded content-addressed CLI binaries and published/development app bundles while preserving artifacts named by the active install or used by live processes. Promotion and recovery retain the published fallback and the one it replaced. Foreign names and switch receipts remain untouched; an unreadable process table skips cleanup, and removal failures wait for a later settlement without failing installation. Actual reclamation remains unobserved, and capture/store retention is unchanged.

The supplied PR evidence records focused checks and broader branch suites, but does not establish a passing full matrix for the final release candidate. Some repaired heads still awaited CI, and the latest context-delivery CI result preceded its final fixture correction.

## Small changes

- Misfiled Tasks can move to another Wave with one command.
- The homepage leads with runnable commands, explains how the planning and execution concepts fit together, and gives the Mac app its own section. Long command blocks scroll within mobile layouts.
- Design, kickoff and implement instructions now identify earlier attempts and carry their removal through the same change. This is agent guidance, not automated enforcement.
- Help remains quiet when optional Process history is unavailable.