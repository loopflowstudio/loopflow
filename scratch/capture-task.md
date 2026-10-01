# Capture Task

Accepted by Jack Heart — 2026-10-01. Scope, conversation behavior, and Desktop
interaction are agreed. Jack Heart authorized filing under Product and launching
the work on 2026-10-01.

## What to build

Desktop's **Create Task** button opens **`capture-task`**, a conversation that
shapes ideas into Tasks for their owning Waves to operate.

## Accepted direction

Jack Heart settled the naming: “capture-task as the prompt name, create task
is the button name.” There is one skill, with no `create-task` alias.

The dedicated session's job is to “make tasks that the right owner will later
operate.” Wave conversations also capture Tasks while “making sure all tasks
progress as far as possible.” Capture is shared behavior; the dedicated session
does not take over operation.

Keep `design`: “I dont think we need to delete design yet, but this largely
will replace design, if it works.” Automatic operational conversations are
partly future design, not an implementation prerequisite here.

## Placement and scope

Wave: `product`. Its current Project recommends `feature`, which begins with
kickoff and design review. Continue this accepted design through that Flow.

Task: [Capture new Tasks from Desktop conversations · LOO-368](https://linear.app/loopflow/issue/LOO-368/capture-new-tasks-from-desktop-conversations).

Related: [Focus on your own work: the Task workspace and primary Sessions · LOO-353](https://linear.app/loopflow/issue/LOO-353/focus-on-your-own-work-the-task-workspace-and-primary-sessions)
owns the broader ongoing-session experience. Capture adds the dedicated entry
point and shared skill without taking over that work or requiring its completion.

Additive keystone, shipped in one PR: the builtin skill, Desktop entry point,
shared launch behavior, documentation, and focused verification. Potential
later work: retiring `design` after experience with capture; persistent
operational conversations. Neither is required or allocated by this design.

## The demo

Click Create Task, describe an idea, explore it, and capture a Task under the
agreed Wave. The Task appears in that Wave's plan, without a worker launched by
capture. Continue discussing another idea in the same conversation. The owner
can operate each Task without reconstructing the conversation.

## Conversation contract

Start with the idea. Use relevant supplied conversation and existing designs;
do not repeat discovery already settled. Explore the desired experience,
alternatives, constraints, and observable success. Inspect code when it resolves
a real uncertainty. Adapt depth to the idea; a technical design is not a filing
requirement. Preserve ambitious intent rather than prematurely shrinking it.

File when intent is clear, rather than creating a placeholder immediately.
Show the concrete brief in the conversation. Honor existing authorization to
capture; do not add a mandatory confirmation ritual. Exploratory possibilities
remain proposals.

Each Task describes the problem, beneficiary, desired outcome, observable
acceptance, real constraints, and consequential open decisions. Distinguish
accepted choices from possible mechanisms. Use people's names for attribution.
Keep implementation sequencing in a design when one exists. Essential intent
must survive in the Task; a path to another checkout or a transcript is
insufficient handoff. Split only independently useful outcomes. Reuse or refine
an existing Task for the same work rather than filing a duplicate.

Resolve ownership as the idea develops. A selected Wave is the expected owner,
not a restriction if another Wave fits better. Repository capture has no
preselected owner. Consult `lf wave list --json` and relevant
`lf wave status <wave> --json` to check objectives, current plans, and overlap.
File through `lf task create --wave <owner>` without `--run`; use `lf task edit`
for subsequent authorized refinements. Return the actual Task link and owner.

Missing ownership or an unavailable Project leaves an explicit unfiled brief
in the conversation. A provider failure never becomes a success claim. After
an uncertain write, reconcile before retrying; retain the identical creation
input when retrying the existing CLI's idempotent creation path.

Wave conversations use the same skill within their current conversation when
capture is requested. They retain their existing operational authority;
capture neither grants nor withdraws permission to operate other Tasks.

## Desktop interaction

Expose a visible Create Task button below the navigator's repository header,
plus a Wave context-menu action. From the repository overview, launch in repo
scope. With a Wave selected, use that Wave; with a Task selected, use its parent
Wave. The explicit Wave action captures its own target before launch.

Open an ordinary interactive Session in the repository checkout with either
`lf --mode interactive capture-task` or
`lf --mode interactive --wave <wave> capture-task`.
Keep the Session at repo/Wave scope after filing, so it can produce several
Tasks. Do not bind it to the first Task, switch checkouts, or change focus after
filing. Existing roadmap refresh exposes the filed Tasks. Launch errors appear
at the entry point and preserve other panes and drafts.

## Kickoff findings (2026-10-01)

Source inspection only; each finding changes the plan below.

- **The existing conversation action moves selection.** `SessionsView` wires
  `onConversation` as `model.select(work)` then `startConversation()`, and
  `PodiumModel.conversationScope` resolves a selected Task to the Task's own
  worktree with `--task`. Capture can reuse neither: it must stay in the
  repository checkout, bind at most a Wave, and leave selection alone. The Wave
  menu action therefore carries its Wave name in the launch value instead of
  selecting first.
- **A named skill stays in the repository checkout.** Only the bare default
  agent (`run_default_agent` in `bin/lf.rs`) calls
  `move_default_agent_to_worktree`. `lf --mode interactive [--wave W] capture-task`
  takes the same path as today's `lf --mode interactive [--wave W] : <prompt>`
  with a skill name in place of the inline prompt. The skill declares
  `requires: none` and no scratch artifact, so nothing asks for a worktree.
- **Creation retry is keyed on exact input.** `task_create` derives its marker
  from `sha256(wave \0 title \0 report)`. Rerunning the identical command
  reuses the committed issue; a brief reworded between attempts files a second
  Task. The skill states this plainly: after an uncertain write, read
  `lf wave status <wave> --json` first, then rerun the same command unchanged,
  and refine through `lf task edit` only once the Task is confirmed.
- **Registration is automatic and flat.** `build.rs` registers
  `task/skill/capture-task.md` as `capture-task`. The catalog needs a
  `description:` frontmatter line. No registry edit.
- **Wave conversations already carry a competing paragraph.**
  `wave/skill/wave_session.md` has a "Develop direction" section that writes
  ideas to `scratch/` and files "only once the direction is ready and the user
  has agreed". Leaving it beside capture would be the second prompt copy this
  design forbids. That section shrinks to: when the user brings an idea, read
  `lf skill show capture-task` and follow it in this conversation; operating
  authority is unchanged. `lf skill show <name>` prints a builtin body today.
- **Desktop already has a headless interaction harness.**
  `DesktopHeadlessTests` taps `WorkspaceNavigator` buttons by accessibility
  identifier through ViewInspector under `scripts/test_desktop.sh`, with no
  display. `MultiplexerStore.newShell(command:)` is already tested for
  preserving other panes. `LocalWaveAgentLauncher.controlLfPath(bundled:developmentConfig:)`
  takes its inputs as parameters, and `SessionsView.launch` resolves it before
  touching any layout; `launchError` renders as the existing alert.
- **No existing Desktop Task-creation control.** `LocalWaveAgentLauncher.startTask`
  (create + run) has no caller in views; nothing to replace or collide with.
- **The installed `lf` lags this branch.** The demo needs the branch-built app
  with its bundled `lf`, or the pane fails with "skill not found". Gate is
  unaffected.

## Data structures and key functions

Reuse AgentSession, Wave, Project, and Task; add no persisted capture record,
DTO, queue, or draft lifecycle. Rust `task_create` remains the filing authority.

`ConversationLaunch.swift`:

```swift
/// Create Task: the capture-task skill at repository or Wave scope, always in
/// the repository checkout. Desktop names the skill; lf owns its prompt.
struct TaskCaptureLaunch: Equatable {
    let repoPath: String
    let wave: String?
    func arguments(lf: String) -> [String]
    // [lf, "--mode", "interactive"] + (wave.map { ["--wave", $0] } ?? []) + ["capture-task"]
}

extension PodiumModel {
    /// Visible button: overview → repo; Wave → that Wave; Task → its parent Wave.
    var taskCaptureLaunch: TaskCaptureLaunch?
    /// Wave menu action: the named Wave regardless of selection.
    func taskCaptureLaunch(wave: WorkReference) -> TaskCaptureLaunch?
}
```

`repoPath` is `model.repoPath`, never a Task worktree. A Wave or parent Wave
whose name cannot be resolved falls back to repository scope rather than
disabling the button: the skill resolves ownership itself. A selected Project
uses its Wave (assumption recorded in `scratch/questions.md`).

`WorkspaceNavigator` gains `onCaptureTask: ((TaskCaptureLaunch) -> Void)?`,
defaulting to nil like its sibling callbacks:

- a **Create Task** button directly below `header`, identifier
  `workspace-create-task`, calling `onCaptureTask(model.taskCaptureLaunch)`;
  hidden when the callback or repository is absent;
- `Create Task · <wave>` in `subjectActions` for Wave rows only, calling
  `onCaptureTask(model.taskCaptureLaunch(wave:))` without `model.select`.

`SessionsView.captureTask(_:)` mirrors `launch`: resolve `controlLfPath()`,
select the repository worktree slot, show terminals, then
`multiplexer.newShell(command: launch.arguments(lf:))`. A throw sets the
existing `launchError` alert and mutates nothing. `launch(_:in:)` and
`ConversationLaunch` are untouched; the two do not merge, since capture has no
inline prompt and no Task scope.

## Affected surfaces

| Surface | Change |
| --- | --- |
| `rust/loopflow/src/engine/builtins/task/skill/capture-task.md` | new skill |
| `rust/loopflow/src/engine/builtins/wave/skill/wave_session.md` | "Develop direction" defers to capture-task |
| `rust/loopflow/src/engine/builtins.rs` tests | registration + description |
| `swift/LoopflowMac/ConversationLaunch.swift` | `TaskCaptureLaunch`, model derivation |
| `swift/LoopflowMac/Views/WorkspaceNavigator.swift` | button, Wave action |
| `swift/LoopflowMac/Views/SessionsView.swift` | `captureTask` wiring |
| `swift/LoopflowTests/` | launch value, button tap, pane preservation |
| `swift/README.md` | Create Task beside New conversation |

No wire DTO, CLI flag, schema, or fixture changes.

## Absent and error states

- No repository open: no button.
- Planning unreadable or Wave name unresolved: repository-scope capture.
- `lf` unresolvable: alert at the entry point; panes, drafts and selection keep.
- Skill missing in the bundled `lf`: the pane shows lf's own error and returns
  to a shell, as any launched conversation does.
- No owner, no current Project, or a provider failure: the skill leaves an
  explicit unfiled brief and says so. Never a success claim.

## Current system and deletion boundary

`ConversationLaunch.swift` supplies inline prompts for general conversations.
`WorkspaceNavigator.swift` exposes those actions; `SessionsView.swift` opens
their panes. Capture sits alongside them. `kickoff` accepts briefs/designs;
`launch-plan` owns execution handoffs. `wave/operate` runs one pass and exits;
this feature does not change that lifecycle.

**Delete — do not maintain:** the "Develop direction" body of
`wave_session.md`, replaced by the pointer to capture-task. Nothing else.
Preserve `design`, existing general conversations, Task conversations, and
Flow review boundaries.

**Forbidden outcomes:** capture starts a worker; a Task needs the capture
transcript to make sense; filing permanently binds the capture Session; a
second prompt copy (in Swift or in `wave_session.md`) or a `create-task`
alias; a new background operating loop; a capture launch that changes
selection or opens in a Task worktree.

## Alternatives considered

- **Add a `.capture` case to `ConversationScope`.** Rejected: scope means
  "where the conversation is about", and every existing case builds an inline
  prompt. Capture is a different launch, so it gets its own small value.
- **Inline the capture prompt in Swift like the general conversations.**
  Rejected by the accepted design; it also makes Wave conversations unable to
  share it.
- **Embed capture text in `wave_session.md`.** Rejected: two copies drift.
  `lf skill show` is the one source both entry points read.
- **A native Create Task sheet calling `startTask`.** Rejected: it files
  before intent is explored and starts a worker.

## Internal slices

1. **This slice:** write `capture-task.md` from the conversation contract
   above, including exact-input retry; point `wave_session.md` at it; extend
   the builtins registration test. Focused check:
   `cargo test -p loopflow --lib engine::builtins`.
2. `TaskCaptureLaunch`, model derivation, navigator button and Wave action,
   `SessionsView.captureTask`; `swift/README.md`. Focused check:
   `scripts/test_desktop.sh --filter TaskCapture`.
3. Demo/review: sample conversations for a vague idea, an already clear
   request, duplicate work, several owners, and failed filing, in the
   branch-built app.

## Done when

- `cargo test -p loopflow --lib engine::builtins` passes with `capture-task`
  registered, described and resolvable by bare name, `design` still
  registered, and no `create-task` skill. Tests assert registration, not
  phrases.
- `scripts/test_desktop.sh` passes with tests showing: tapping
  `workspace-create-task` yields repository scope from the overview, the Wave
  for a selected Wave, the parent Wave for a selected Task, all with the
  repository path and without changing `model.selection`; the Wave action
  yields its own Wave while another subject is selected; the resulting command
  is exactly `lf --mode interactive [--wave W] capture-task`; opening it beside
  a live shell and another conversation leaves both intact; an unresolvable
  `lf` throws before any pane exists.
- `swift build --package-path swift` succeeds.
- Demo/review, not gate: a real conversation files a Task under the agreed
  Wave, it appears in that Wave's plan with no worker started, a second idea
  is captured in the same Session, and the owner can act on each Task from its
  text alone.

Check: 2026-10-01 kickoff source inspection of launch, navigator, headless harness, skill registration and creation marker; plan-only change, no builds run.
