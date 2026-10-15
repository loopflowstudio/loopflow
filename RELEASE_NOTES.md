# v0.14.0

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.14.0 brings conversations, planning, and delivery into workspaces you can keep using as agents change, runs stop, and PRs merge. Tasks can run without Linear, conversations can continue on another provider, and Desktop keeps files and review beside the work. This is the cycle summary since v0.13.0; many of these improvements arrived in the intervening patches.

## Return to the work in place

Desktop now opens around repository conversations and retained Work. Waves organize Tasks without a separate Session inventory, while files, terminals, and published PRs stay beside the conversation.

- Repository launch opens its configured conversation. Returning launches show saved workspace content while fresh data loads, and a persistent workspace stream follows Task, Session, and file changes.
- Open Work navigation with Command-K and the Files chooser with Command-P; pin either when needed. Available Tasks appear once with their Wave context, using the same availability definition as `lf session list --available`.
- New Task workspaces open Session and Files side by side. Switching companions retains file drafts and hidden shells; command-click opens Work beside the current destination.
- Inspect a published GitHub PR's description, checks, reviewers, change totals, and patches in the workspace. Missing access and incomplete patches remain explicit.
- Open Task links directly or arrange Desktop from `lf`. Reopening an existing destination preserves its selected conversation and panes. Repository and Wave conversations also reuse dedicated worktrees for local plans.

## Continue the conversation with the right agent

Session identity now carries the conversation through provider replacement and recovery. The cycle also removes launch friction that could strand saved work or leave dead agents looking active.

- `lf resume` returns to a conversation in the current worktree. Use `lf resume -a <agent>` or `lf session connect -a <agent>` to continue a Session on another provider.
- Claude and Codex use their ordinary native homes by default, making conversations available from their native CLIs. Installed skills from either provider can run through either harness.
- Global `-a` / `--agent` carries an explicit agent and optional model through Tasks, Flows, and nested commands. Configured launch settings, including permissions and Chrome integration, apply across CLI, Desktop, and headless entry paths.
- Interactive Flow steps retain the native agent UI. Headless Claude answers enter the history used for Flow decisions, and headless Claude or OpenCode conversations can be stopped and resumed interactively.
- Launches put the request first and refresh checkout context after compaction. Codex prompt delivery avoids large terminal-wrapper arguments; failed starts and confirmed provider exits have recovery paths that retain saved conversations.

## Plan locally and keep control of ongoing work

Planning no longer depends on Linear being reachable. Task completion, Workflow position, and execution history have separate responsibilities, so an old process record need not hold delivered work open.

- Create, edit, and start Tasks in the local store. Share saved Waves, Projects, Tasks, and comments through repository-selected Git or Linear synchronization.
- Wave goals and memory come directly from checkout files. Each worktree supplies its own context, and edits appear on the next read.
- Keep design and review in one Task conversation, with a visible Workflow showing position and possible next Flows. Workflow discovery and controls are separate from Flow execution history.
- Pause unfinished Tasks without losing their checkout, PR, conversations, Workflow, or execution history. Paused work leaves active navigation and does not start new automated work.
- Complete Tasks independently of Workflow position and stale execution uncertainty. Direction reaches each associated Session through its own delivery history without starting idle conversations.

## Keep publishing from the checkout you have

Delivery now adapts to more repository conventions and preserves ongoing branch work. Later changes in this cycle allow another PR from the same Task checkout after a merge, superseding the earlier one-PR-per-Task restriction.

- Publish the next change from the same checkout while retaining earlier PR history and carrying follow-up edits past squash merges.
- PR opening and publication resolve comparisons from current Git ancestry when saved bases are stale. Syncing a Task's own remote branch no longer makes its commits look like foreign ancestry.
- Follow-up work survives merges from main, including when main advances again after a sync, without replaying already-integrated upstream changes.
- Repository Git and delivery behavior follows more existing conventions, with explicit review handoffs that do not change PR assignments. Scratch cleanup preserves existing documents and their edits, limiting removal to added files.

## Spend less time waiting and reclaim unused output

The cycle reduces repeated database reads, provider checks, builds, and synchronization requests. It also adds measurements and isolated development data so remaining delays can be investigated without treating every workload as identical.

- `lf wt list` avoids full history scans on store open and runs Git reads and GitHub requests concurrently. Desktop avoids repeated model work, reads, and redraws when returning to saved work.
- Linear checks happen at work boundaries instead of independently in every agent. Rate-limit pauses are shared across repositories, and pending writes remain available for recovery.
- New Session history writes avoid duplicated token storage. Installation maintenance reduces unnecessary database copies, progresses through backups of active stores, and removes superseded binaries and app bundles after a switch settles.
- CI reuses builds and fixtures and overlaps independent checks. Local Rust verification prepares its fixture CLI once; bounded cleanup reclaims completed, inactive build output while retaining checkouts.
- `lf clone create/run/show/dispose` supports writable disposable Machines for offline development and demos. Desktop launch and refresh timing records help separate first launches after updates from repeat launches.

## Operational notes

- Execution locations are called Machines; installation maintenance is grouped under `lf self`. Save a remote connection under a readable name and use `lf --machine <label>`. Remote provider logins can be installed from the laptop for later use on that Machine.
- Agent selection uses `--agent` / `-a`; the unsupported `--ide` launch path was removed. Recorded commands are Processes, with durable `lfid` separate from optional Unix `pid`.
- Authored Wave files moved into `.lf/wave`. Workflow and Flow definitions have separate catalogs; automation should use the corresponding controls.
- Release recovery retains the pending version when preparation needs correction, tracks scheduled opportunities through verification and publication, and retries transient artifact reads. Timed-out notarization can resume using the retained signed DMG and submission evidence.
- Release verification uses required headless checks; installation checks run in disposable containers. An explicit first release version can use repository history without a seed tag.
- Retained Desktop pane layouts persist within the window, not across app relaunch. The supplied record does not establish a final full-matrix pass or all-provider input coverage for this release.

## Small changes

- Copy confirmation stays in the pane header; Find sits outside the terminal viewport and respects Reduce Motion. Exact shortcut matching preserves provider shortcuts with extra modifiers, and contrast correction retains more terminal color.
- Readable Task identifiers appear in generated Session names and new branches. Session renames update cmux titles, while optional naming yields when the store is busy.
- Committing selected scratch files works in ordinary Task worktrees, and failed Git commands retain useful diagnostics.
- Worktree prefixes remain consistent after repository renames. Deleted development CLIs no longer block unrelated macOS installs.
- Task Process history loads when Debug expands. Lifecycle refresh failures do not repeat successful mutations, and dead providers no longer remain active because of stale status reports.