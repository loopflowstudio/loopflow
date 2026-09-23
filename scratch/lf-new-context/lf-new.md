# Worktree workspaces

## What to build

Open a conversation whose default prompt follows the current repo/Wave/Project/Task scope, using the configured app or terminal; retain two nested layouts for working contexts: worktrees outside, terminals inside.

User anchors:
- “a single buttom taht creates a new-worktree then opens lf design in that new worktree”
- “Use the configured app or terminal”
- “new shell *which is a truly flesible shell*”
- “grouped together so when you switch between sessions you also switch your extra terminals”
- “the first is the woktree graph and then within the worktree you have the terminals. splits can be at either boundary level.”
- “maybe the new worktree shoudl also create a new task that this gets marked as fixing”
- “amybe we do need unfiled tasks / taskless sessions”
- “or alternatively it just opens the master `lf` (or lf wave-operate/lf project-operate instead of lf design”
- “so like hwoever zoomed in you are, the default prompt for the new shell changes”

Current direction: the launch action follows navigation scope. Opening a conversation need not immediately create a Task or checkout. Task-first and taskless-worktree creation below remain alternatives under exploration; neither is mandatory. The initial always-create-a-worktree-and-run-design behavior has been reopened. Final label and placement remain proposals.

## Scope chooses the starting conversation

| Visible scope | Default conversation context | Checkout behavior |
| --- | --- | --- |
| Repository | Master Loopflow conversation: shape an idea and find its home | Use the repository context; no automatic new checkout |
| Wave | This Wave's direction, priorities, and Projects | Use its existing repository context |
| Project | This Project's intent, progress, and possible Tasks | Use its existing repository context; Projects do not own worktrees |
| Task | Work on this Task, with its directive and current evidence | Use its owned checkout |

These are semantic defaults, not settled command spellings. Capture the visible scope when the action is invoked; do not substitute the last visited Project. Show that scope in the launch affordance. Honor the configured app or terminal at every level. A separate ordinary **New terminal** action adds a flexible shell within the active workspace without an injected agent prompt.

Opening a scoped conversation can reuse main-view's current human Session for that subject. Whether an explicit New action instead starts a fresh conversation remains open; do not silently replace an existing live Session. Starting a scoped conversation alone creates no planning issue. Once an idea becomes concrete, an explicit Task creation step selects its Project, prepares its checkout, and opens the Task conversation with the agreed brief and a reference to the source conversation. Seamlessly moving a live provider thread across checkouts is not assumed.

`wave/operate` and `project/operate` currently describe autonomous operating passes that can launch work and deliver Wave chat updates. A human conversation opened from this button must not inadvertently execute those passes. Reuse the appropriate subject context with interactive entry semantics; settle the exact prompt/command before implementation. The current `loopflow` skill launch also forces TUI in `run.rs`, which must be reconciled with the user's configured-destination requirement if that entry is used.

The decisive demo for this direction: invoke the action at repo, Wave, Project, and Task scope; each conversation receives the displayed subject and opens in the configured destination. No new issue, checkout, controller, or chat delivery occurs merely from opening it. Then create a Task from an agreed idea and verify the new Task conversation receives its brief in the owned checkout. Existing conversation/shell grouping and both split levels still apply.

## Fit with main-view

Read `/Users/jack/src/loopflow.main-view/scratch/main-view.md` and its second-round mockup on 2026-09-23 at the user's request. That design owns navigation: one Task list with minimal collapsible Wave/Project headings, plus full-width focus or an optional visible work list. This design owns creation and the selected workspace's two-level layout. Do not ship a competing worktree inventory or require a permanent sidebar; the worktree rail in the first sketch is superseded as navigation.

Every normal human conversation has a subject: repo, Wave, Project, or Task. Main-view proposes one current human Session per subject, independent of autonomous Runs. **New task** gives an open-ended design its subject immediately; **New shell** adds a terminal within the Task workspace, not another planning row or automatically another human Session. The outer worktree graph remains a layout of opened workspaces, not a second hierarchy beside Wave/Project/Task.

The earlier proposal to remove all normal unattributed-session navigation is reopened. An **Unfiled** section could hold explicitly taskless workspaces, each named after the idea or checkout and containing its conversation and shells. It is a derived navigation group, not an orphan Linear Task or another durable Work kind. Unknown attribution caused by a failed lookup must not silently become Unfiled. Repo/Wave/Project sessions keep their named entry points.

An unfiled workspace may become a Task through **File as task…**: select the existing Project and title, create/bind the Task to this checkout, and move the same workspace into its Wave/Project group. Preserve the branch, files, terminal processes, conversation/history, split layout, and focus. Do not create another checkout or restart the agent. Existing `task_prepare` must be assessed for explicit adoption support; this behavior is a requirement, not a claim that it already works. Preserve original Run provenance while recording the new Task relationship.

This exposes a real conflict with main-view's proposed one-current-Session-per-repo rule: several independent exploratory worktrees need several conversations. Keep the repo's general conversation separate from per-workspace exploratory conversations, or revise that cardinality rule. Do not force unrelated ideas into the same repo chat to preserve a prior model.

Preserve truly flexible shells. If a user manually starts an additional independent agent inside a Task shell while its current human Session already exists, the one-current-Session rule needs an explicit policy. Do not enforce it by killing the existing client, hiding the additional Run, or pretending every shell is another Task. The normal Open session action must create or resume the subject's current Session; arbitrary command handling is still an open boundary to reconcile with main-view.

## Placement

Implementation Wave/Project unresolved: neither is named in the request. A workspace is a UI grouping around a checkout, not a fourth planning kind. Filed workspaces belong to real Tasks under exactly one existing Project and its Wave. The current proposal also permits deliberately taskless workspaces without fabricating Project ownership.

## The demo

The following creation demo specifies the earlier Task-first/taskless alternatives. The scoped-entry demo above is the current starting-point proposal; resolve the entry choice before implementing creation behavior.

1. With an existing Project selected, click **New task**. Loopflow creates the Task, prepares its owned sibling checkout, and runs `lf --task <issue> design` there with no launch-destination override. Its configured app or terminal opens. The workspace header shows the Task identifier and title.
2. Inside that workspace, click **New shell** twice. Both open ordinary interactive shells initially rooted in its checkout; run a server in one and arbitrary commands in the other.
3. Create a second workspace. Its conversation and shells have their own layout.
4. Switch back. The first workspace's conversation, shells, running server, terminal output, layout, and focus are intact.
5. Launch `lf design` manually in one of those shells. Its sidebar row associates with the terminal already here, without Move here, replacement, or a duplicate provider.
6. Split at the worktree level and show both workspaces side by side. Split a terminal inside the first: only that workspace's internal layout changes. Switch the second outer slot to another worktree and back; both inner layouts and running processes survive.
7. Proposed unfiled path: start another idea without selecting a Project. Later choose File as task, then verify that it moves into the chosen Project with the identical checkout, files, conversation, running shells, and layout.

## Interaction

**New task** creates a tracked Task, then lets Task preparation own branch/worktree placement and PR identity. Start with a provisional title such as “New design”; the design conversation refines that Task's title and directive as the idea becomes clear. Updating its title does not move the checkout or replace its identity. Launch one bound design Run; do not implicitly start the autonomous Task controller.

Proposed two entry paths: **New task** under an explicit Project creates filed work immediately; repository-level **New design** starts a taskless workspace without a Project picker. Both reach the configured design destination. **File as task…** supplies ownership later. Whether both verbs are needed, or one New action adapts to scope, remains open. Do not silently file under whichever Project was last visited.

The Task reference follows the work into Runs and the eventual PR. Reuse the existing managed Task link and PR lifecycle: the PR is work toward that Task, while completion happens only at the explicit completion/merge boundary. Opening a design or publishing a first PR must not mark the Task done. Additional shells belong to the same workspace and create no Tasks.

**New shell** is workspace-scoped: another terminal in the active worktree. It creates no checkout and starts no agent. It accepts arbitrary commands, `cd`, subprocesses, and manually launched agents. Changing its cwd does not move it into another workspace.

Selecting a conversation in another worktree selects that entire workspace in the focused outer slot, then focuses its conversation. Conversations in the same worktree share companion terminals. Switching groups never replaces the contents of an unrelated group's terminal pane.

**Split worktrees** divides the outer layout horizontally or vertically. The new slot can select an existing worktree or create new work. Each visible worktree has its own identity/header and complete inner layout. **Split terminals** divides only the focused worktree's inner layout. Its new leaf can open an existing conversation or an ordinary shell in that checkout. Place outer split controls on the workspace header and inner split controls on terminal headers; an unlabeled global split button is insufficient.

Focus is a pair: the active worktree slot and the active terminal inside it. Outer resize/close operations affect workspace views; inner resize/close operations affect terminal views. Closing an outer split hides that group without ending its terminals. Do not flatten worktrees and terminals into a single mixed split tree. Opening a worktree already visible in the window focuses its existing slot; do not mount the same native terminal surface twice.

A workspace remains available when its agent exits or completes: its shells still belong there. Closing one shell ends that shell; switching or hiding a workspace ends nothing. Closing a group view never deletes its checkout. App-restart process persistence is outside this first design.

External app launches remain external. Workspace selection restores Loopflow's local group; explicitly opening its conversation uses the existing launch/resume behavior. App handoff may legitimately be elsewhere. An embedded live conversation must be recognized as here.

## Current system and observed failure

`SessionsWorkspaceRegistry` currently keys one multiplexer and surface pool per **repository**, so all that repository's shells share one layout. `SessionsStore` independently reconciles Session records; `load(sessionId:)` may replace the focused Session pane. This does not express worktree groups.

`SessionsStore.reconcile` treats an active Session as local only when `surfaces.hasSurface(.session(record.id))` succeeds. New shell creates `.shell(pane.id)`. A manually launched agent has no association between those identities.

The user's 2026-09-23 screenshot confirms the failure: two visible embedded design conversations, both marked ELSEWHERE. Their sidebar titles also both expose `<lf:skill:design>`, obscuring which worktree each belongs to. Use worktree names as an immediate distinguishing label; do not render prompt markup as a conversation title.

`lf task prepare <issue>` already establishes Task Work, its checkout, and serial PR identity without starting a worker. `lf task start` creates an issue but also starts a controller, so it is not the desired design-only launch. `lf pm task create` creates the planning issue independently. Compose creation and preparation through lf; do not create a raw worktree first and try to attach a Task afterward. The existing PR code already supplies Task identity and distinguishes publication from completion.

Normal skill launch owns provider/account/configuration and app/TUI handoff. The TUI path, including app-open fallback, needs a real terminal host.

## Data structures and key functions

- `WorktreeLayout`: recursive outer split tree with leaves containing a worktree key or an empty selection slot. Repository/window state owns this layout and the focused outer slot.
- `WorktreeWorkspace`: exact checkout identity/path, its inner `MultiplexerStore`, retained terminal surfaces, and session membership derived from shared records. Window-local state, keyed by checkout rather than repository. Existing `LayoutNode`/`PaneContent` remain the terminal layer; worktree leaves are a separate type.
- `SessionsWorkspaceRegistry.workspace(forWorktree:)` and `selectWorkspace(_:sessionId:)`: restore the group, then focus an existing member. Use shared Task/workspace references for attribution and exact checkout roots for terminal grouping; cwd alone never establishes Task ownership. Never collapse sibling checkouts to the main repository for grouping.
- `splitWorktree(slot:axis:)` changes the outer layout; `splitTerminal(worktree:pane:axis:)` changes only that worktree's inner layout. Splitting does not implicitly duplicate or restart a process.
- `createTaskWorkspace(project:requestId:) async`: an lf-owned create-and-prepare operation returns the Task identifier and its authoritative workspace. Compose existing issue creation and `task_prepare`; then launch `lf --task <issue> design`. Swift must not reconstruct placement. The earlier proposal to add a worktree-only receipt is superseded.
- `newShell(in:)`: add a normal shell to the selected workspace using its checkout as initial cwd.
- `TerminalAttachment { sessionId, terminalId }`: association to an existing live terminal, not another Session record. `terminal(forSessionId:)` and pane selection resolve this association as well as directly opened Session surfaces. Preserve shell lifetime and identity when an agent runs inside it.

Attachment must reflect the actual live provider client, not its originating cwd, title, or an inherited marker after external handoff. The existing one-shot `LF_HUMAN_SESSION_RUN_BIND` handshake is insufficient for successive arbitrary launches from a long-lived shell; the repeatable terminal/client binding transport still needs a focused design proof. Rust retains Session/client authority; Swift owns view association. Any new wire fields need DTO fixtures and explicit required-or-optional types.

## Failure recovery and constraints

Allocate a distinct creation request ID per intentional click; disable duplicate submission during setup. Repeated provisional titles must still create distinct Tasks, while retrying one request must recover the same Task and checkout. Show creating Task/preparing workspace/opening/handoff states based on actual results. If issue creation succeeds but preparation fails, retain the Task reference and resume preparation. Task-backed creation must not silently fall back to untracked work on PM failure; explicitly taskless creation should not depend on PM availability. Failed launches retain error details and an ordinary shell for recovery. App-open acceptance is not proof that an agent is interactive.

Do not re-key or recreate a live terminal merely to register its agent. Manual `lf design`, the new-work button, and ordinary Session opening must converge on the same ownership representation. Native app launches must not be mislabeled as embedded because they inherited launch context.

## Shape and internal slices

The nested workspace interaction is an indivisible change: internal slices, one PR. Scoped conversation entry may be an independently useful keystone, but its boundary is not yet selected. The earlier implementation sequence below is provisional until that choice is resolved; a renamed button alone does not prove the workspace behavior.

1. **This slice:** resolve the shell-to-Session attachment mechanism against the supplied screenshot; prove two shell-launched agents map to their existing panes and external handoff stays external.
2. Replace the repository-wide terminal layout with an outer worktree layout and per-worktree inner terminal layouts; preserve free shells, both split levels, switching, and conversation focus.
3. Add New task with shared issue creation/preparation, configured bound-design launch, progress/retry, and workspace-scoped New shell.
4. Verify the full demo and update `swift/README.md`.

Later possibilities, not filed Tasks: conversation-derived names, artifact browsing within the group, cross-device terminal persistence. Their scope is not decided here.

## Done when / forbidden near-misses

Focused behavioral proofs cover two worktrees with two shells each; switching preserves process identity, output, cwd, layout, and focus. Show both worktrees in an outer split and split a terminal inside one: the other inner tree is unchanged. Closing/restoring an outer slot preserves its complete inner state. A shell may leave its initial directory without regrouping. Ending its LLM does not destroy sibling terminals. Two Sessions sharing a checkout share one workspace.

Registration proof covers manual shell launches, repeated launches in the same shell, direct Session opens, another window, and external app handoff. Clicking a local row focuses its existing terminal and never invokes Move here or starts another provider.

Real configured-path demo creates a checkout, opens design there, and writes its artifact there. Verify both app configuration and terminal configuration. Mocked argv, renamed buttons, isolated passing helpers, or correct grouping with false ELSEWHERE labels do not count.

Task proof: each intentional creation produces one issue, one Task Work, and its one active checkout/PR identity; retries after each partial failure produce no duplicates. The design Run is attributed to that Task, and the published PR links the same issue. Neither design startup nor publication completes it.

For the proposed taskless path, no issue is created until filing. Filing creates exactly one Task and adopts the existing checkout without interrupting its terminals or replacing history; retry after a partial filing failure must recover the same Task.

## Evidence ledger

- Source inspection established repository-keyed layouts and the `.shell` versus `.session` lookup mismatch.
- User screenshot supplied the two-session counterexample; generic prompt-markup titles are a second observed UX defect.
- Reference research and initial visual comparison: `research-new-task-ux-69c7ed21.md`, `new-task-ux.html`. Research predates the explicit group-switching requirement.
- User clarified two graph levels, each independently splittable. `worktree-layout.html` explores this structure; the earlier button-placement sketch is historical research, not the full target architecture.
- User proposed Task-backed creation. Source inspection confirms create, prepare-without-worker, and managed PR linkage already exist separately. Project selection is the remaining product decision.
- Read the adjacent main-view design and round-two visual study. Its accepted navigation is an annotated Task list with optional visibility; its proposed current-Session cardinality and repo/Wave/Project entry points eliminate the need for a normal unfiled Session inventory. No files in that checkout were edited.
- User subsequently reopened unfiled/taskless work. The removal of an Unfiled section and mandatory Task-first creation are no longer settled; taskification preserving the existing workspace and per-repo exploratory cardinality require reconciliation. Existing HTML sketches predate this latest fork.
- User then proposed the master/repo, Wave, or Project conversation and clarified that zoom level changes the default prompt. Current direction is scope-sensitive conversation entry; immediate Task/worktree creation is optional and remains unresolved. Inspected operating skills as reference only: they have autonomous side effects, so their exact invocation is not an approved interactive launch design. No operating skill was run.
- No production implementation or live launch proof performed.
