# Capture Tasks

## Current entry-point decision

Jack Heart selected **New Session** in the Linear-style compose row beneath the
repository header, and explicitly requested keeping the existing opening prompt.
Use the same label for the Wave menu action. Compact row spacing and the compose
icon implement that visual direction; final appearance still needs review.
This supersedes historical provisional naming below; capture behavior is unchanged.


Accepted by Jack Heart — 2026-10-01. Scope, conversation behavior, and Desktop
interaction were initially agreed. Jack Heart authorized filing under Product
and launching the work on 2026-10-01. Review clarifications below supersede the
initial naming and decomposition guidance; the Desktop button label is open.
Jack Heart approved continuing with the revised design on 2026-10-01 after
clarifying Task sizing. That approval does not select a final button label.

## What to build

Desktop's **Create Task** button opens **`capture-tasks`**, a conversation that
shapes intention into any number of Tasks across Waves and repositories for
their owners to operate. Launch context does not constrain filing destinations.

## Accepted direction

Jack Heart revised the skill name to **capture-tasks** on 2026-10-01. The
Desktop button label is under discussion; **Create Task** below is provisional.
Jack Heart suggested **New**, **New Session**, or **Design**; **Explore an idea**
is an agent proposal, not an accepted label. There is one skill, with no
`capture-task` or `create-task` alias.

Jack Heart clarified that Tasks are the data model for expressing intention.
One conversation can produce many Tasks, cross Wave boundaries, and span
repositories. The initial Wave attribution may be wrong; it is a starting
context, not an ownership decision. No separate intention record is introduced.

The dedicated session's job is to “make tasks that the right owner will later
operate.” Wave conversations also capture Tasks while “making sure all tasks
progress as far as possible.” Capture is shared behavior; the dedicated session
does not take over operation.

Keep `design`: “I dont think we need to delete design yet, but this largely
will replace design, if it works.” Automatic operational conversations are
partly future design, not an implementation prerequisite here.

## Placement and scope

Wave: `product`. Its current Project recommends `feature`, which begins with
kickoff and design review. That Flow retains the review boundaries for this work.

Task: [Capture new Tasks from Desktop conversations · LOO-368](https://linear.app/loopflow/issue/LOO-368/capture-new-tasks-from-desktop-conversations).

Related: [Focus on your own work: the Task workspace and primary Sessions · LOO-353](https://linear.app/loopflow/issue/LOO-353/focus-on-your-own-work-the-task-workspace-and-primary-sessions)
owns the broader ongoing-session experience. Capture adds the dedicated entry
point and shared skill without taking over that work or requiring its completion.

Additive keystone, shipped in one PR: the builtin skill, Desktop entry point,
shared launch behavior, documentation, and focused verification. Potential
later work: retiring `design` after experience with capture; persistent
operational conversations. Neither is required or allocated by this design.

## The demo

Click Create Task from one Wave, describe an idea, and explore it into several
Tasks. Discover that some belong to other Waves or repositories, correct the
initial attribution, and file each in its agreed destination without launching
workers. Continue with another idea in the same conversation. Each owner can
operate its Tasks without reconstructing the conversation.

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
insufficient handoff. Reuse or refine an existing Task for the same work rather
than filing a duplicate.

Jack Heart clarified Task sizing on 2026-10-01: favor larger, cohesive Tasks
with obvious behavioral promises and benefits. Early decomposition often fails
to match how implementation actually unfolds. Choose boundaries carefully;
do not apply a formula such as one Task per independently useful outcome.
Useful opportunities to pipeline or parallelize work can justify splitting,
but they are considerations, not requirements to fragment an idea. Keep speculative
implementation steps inside the Task or its design until a useful boundary is
clear. Several Tasks are supported, never a quota or a goal of capture.

Resolve each Task's repository and owning Wave as the idea develops. Treat the
selected Wave and repository as clues that may be wrong. Do not force related
intent into one Task or one owner, or split a cohesive outcome merely because
it touches multiple repositories. A Task retains one owning Wave; several Tasks
can express distinct outcomes with dependencies and shared context made explicit.

Consult `lf wave list --json` and relevant `lf wave status <wave> --json` in
each relevant repository to check objectives, current plans, and overlap. Use
the destination repository as the working directory for each scoped CLI call;
the capture Session itself stays in its original checkout. File through
`lf task create --wave <owner>` without `--run`; use `lf task edit` for subsequent
authorized refinements. Return each actual Task link, repository, and owner.
Do not treat a same-named Wave in another repository as the same destination.

An unavailable destination repository, missing ownership, or an unavailable
Project leaves an explicit unfiled brief
in the conversation. A provider failure never becomes a success claim. After
an uncertain write, reconcile before retrying; retain the identical creation
input and destination repository when retrying the existing CLI's idempotent
creation path. Report each Task separately: successful filings remain successful
when another fails. Reconcile uncertain writes in their destination before retrying;
never recreate the entire batch or silently file in the launch repository.

Wave conversations use the same skill within their current conversation when
capture is requested. They retain their existing operational authority;
capture neither grants nor withdraws permission to operate other Tasks.

## Desktop interaction

Expose a visible Create Task button below the navigator's repository header,
plus a Wave context-menu action. From the repository overview, launch in repo
scope. With a Wave selected, use that Wave; with a Task selected, use its parent
Wave. The explicit Wave action captures its own target before launch.

Open an ordinary interactive Session in the repository checkout with either
`lf --mode interactive capture-tasks` or
`lf --mode interactive --wave <wave> capture-tasks`.
Keep the Session at repo/Wave scope after filing, so it can produce several
Tasks. Do not bind it to the first Task, switch checkouts, or change focus after
filing. Repository-specific CLI calls may target other checkouts without moving
the Session. Existing roadmap refresh exposes filed Tasks in each destination
repository's Wave plan; return links without redirecting Desktop to those plans. Launch errors appear
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
  `move_default_agent_to_worktree`. `lf --mode interactive [--wave W] capture-tasks`
  takes the same path as today's `lf --mode interactive [--wave W] : <prompt>`
  with a skill name in place of the inline prompt. The skill declares
  `requires: none` and no scratch artifact, so nothing asks for a worktree.
- **Creation retry is keyed on exact input.** `task_create` derives its marker
  from `sha256(wave \0 title \0 report)`. Rerunning the identical command
  reuses the committed issue; a brief reworded between attempts files a second
  Task. The skill states this plainly: after an uncertain write, read
  `lf wave status <wave> --json` in the destination repository first, then rerun
  the same command unchanged in that repository,
  and refine through `lf task edit` only once the Task is confirmed.
- **Registration is automatic and flat.** `build.rs` registers
  `task/skill/capture-tasks.md` as `capture-tasks`. The catalog needs a
  `description:` frontmatter line. No registry edit.
- **Wave conversations already carry a competing paragraph.**
  `wave/skill/wave_session.md` has a "Develop direction" section that writes
  ideas to `scratch/` and files "only once the direction is ready and the user
  has agreed". Leaving it beside capture would be the second prompt copy this
  design forbids. That section shrinks to: when the user brings an idea, read
  `lf skill show capture-tasks` and follow it in this conversation; operating
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
/// Create Task: the capture-tasks skill at repository or Wave scope, always in
/// the repository checkout. Desktop names the skill; lf owns its prompt.
struct TaskCaptureLaunch: Equatable {
    let repoPath: String
    let wave: String?
    func arguments(lf: String) -> [String]
    // [lf, "--mode", "interactive"] + (wave.map { ["--wave", $0] } ?? []) + ["capture-tasks"]
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
`workspaces.workspace(for: launch.repoPath).multiplexer.newShell(command: launch.arguments(lf:))`.
Address the destination workspace explicitly, matching the existing conversation
launcher. A throw sets the
existing `launchError` alert and mutates nothing. `launch(_:in:)` and
`ConversationLaunch` are untouched; the two do not merge, since capture has no
inline prompt and no Task scope.

## Affected surfaces

| Surface | Change |
| --- | --- |
| `rust/loopflow/src/engine/builtins/task/skill/capture-tasks.md` | new skill |
| `rust/loopflow/src/engine/builtins/wave/skill/wave_session.md` | "Develop direction" defers to capture-tasks |
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
- Unavailable destination repository, no owner, no current Project, or a provider failure: the skill leaves an
  explicit unfiled brief and says so. Never a success claim.

## Current system and deletion boundary

`ConversationLaunch.swift` supplies inline prompts for general conversations.
`WorkspaceNavigator.swift` exposes those actions; `SessionsView.swift` opens
their panes. Capture sits alongside them. `kickoff` accepts briefs/designs;
`launch-plan` owns execution handoffs. `wave/operate` runs one pass and exits;
this feature does not change that lifecycle.

**Delete — do not maintain:** the "Develop direction" body of
`wave_session.md`, replaced by the pointer to capture-tasks. Nothing else.
Preserve `design`, existing general conversations, Task conversations, and
Flow review boundaries.

**Forbidden outcomes:** capture starts a worker; a Task needs the capture
transcript to make sense; filing permanently binds the capture Session; a
second prompt copy (in Swift or in `wave_session.md`) or a `create-task`
alias (including the former `capture-task` spelling); a new background operating loop; a capture launch that changes
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

1. Write `capture-tasks.md` from the conversation contract
   above, including exact-input retry; point `wave_session.md` at it; extend
   the builtins registration test. Focused check:
   `cargo test -p loopflow --lib engine::builtins`.
2. `TaskCaptureLaunch`, model derivation, navigator button and Wave action,
   `SessionsView.captureTask`; `swift/README.md`. Focused check:
   `scripts/test_desktop.sh --filter TaskCapture`.
3. Demo/review: sample conversations for a vague idea, an already clear
   request, duplicate work, wrong initial attribution, several Waves and repositories,
   and partial or uncertain filing, in the
   branch-built app.

## Done when

- `cargo test -p loopflow --lib engine::builtins` passes with `capture-tasks`
  registered, described and resolvable by bare name, `design` still
  registered, and neither a `capture-task` nor a `create-task` skill. Tests assert registration, not
  phrases.
- `scripts/test_desktop.sh` passes with tests showing: tapping
  `workspace-create-task` yields repository scope from the overview, the Wave
  for a selected Wave, the parent Wave for a selected Task, all with the
  repository path and without changing `model.selection`; the Wave action
  yields its own Wave while another subject is selected; the resulting command
  is exactly `lf --mode interactive [--wave W] capture-tasks`; opening it beside
  a live shell and another conversation leaves both intact; an unresolvable
  `lf` throws before any pane exists.
- Exercise launch from a Task checkout with existing panes there and in the
  repository checkout: capture opens in the repository workspace, preserves
  both layouts' existing panes, and leaves the selected Task unchanged.
- `swift build --package-path swift` succeeds.
- Demo/review, not gate: a real conversation produces multiple Tasks across
  Waves and repositories, corrects a wrong initial ownership guess, and files
  each in its agreed plan with no worker started. A second idea is captured
  in the same Session; each owner can act from Task text alone.
- Exercise partial filing: one Task succeeds, another fails or has an uncertain
  result. Preserve the successful Task, reconcile the uncertain destination,
  and retry only that exact input there without duplicates. An inaccessible
  repository leaves an explicit unfiled brief, never a substitute local Task.

Check: 2026-10-01 kickoff source inspection of launch, navigator, headless harness, skill registration and creation marker; plan-only change, no builds run.

## Capture launch review (2026-10-01)

Jack Heart clarified the scope during review: many Tasks, cross-Wave and
multi-repository capture, revisable initial attribution, and Tasks as the data
model for intention. The skill name is now `capture-tasks`. Source review supports
keeping capture separate from `ConversationLaunch`: the latter derives a Task
checkout and Task binding when a Task is selected. Capture needs neither.

The implementation detail above now names the repository workspace explicitly,
following [the existing launcher](../swift/LoopflowMac/Views/SessionsView.swift).
The added cross-checkout acceptance case proves preservation at the production
entry point, beyond `MultiplexerStore.newShell` in isolation. This is a technical
clarification of the accepted behavior, not a new product decision.

The reversible assumptions remain in [questions.md](questions.md). The Desktop
button label remains unresolved. Remaining work is the builtin skill,
Desktop controls, destination-scoped filing/retry verification, focused checks,
and the real capture-to-owner demo described
above; none is established by this source review.

Check: 2026-10-01 review of ConversationLaunch, SessionsView, WorkspaceNavigator and wave_session; prose-only changes, no builds run.


## Reviewable implementation (2026-10-02)

Jack Heart authorized this manual contribution after ordinary Task launch failed
before claiming a worker. The pre-edit and final Task status reads both report
the existing feature Flow idle at implement with no advancement worker claimed.
This contribution changes neither the Flow cursor nor its review boundaries.

The builtin `capture-tasks`, Wave conversation pointer, Desktop button and Wave
action, launch scope, and user documentation are implemented together. Create Task
remains a provisional label, not a recorded final naming decision.

Code review found that the original launch sketch selected a repository slot but
Desktop still rendered the selected Task's checkout. `showsRetainedTerminals` now
records this presentation choice on the existing repository navigation owner;
capture shows the repository layout without changing Task selection. Selecting a
subject restores normal Task presentation. No execution or persisted owner was added.
The production-control test proves both checkout layouts survive and a missing
helper changes neither layout nor selection. It uses an owned test host and does
not launch a provider or establish live PTY/draft continuity.

Remaining review/demo work:

- Select the final button label and judge its placement in the configured app.
- Exercise the Wave context-menu control in the app; focused tests cover its
  explicit launch value while another subject is selected, not native menu dispatch.
- Demonstrate real multi-idea capture, useful self-contained Task briefs, corrected
  ownership, and filing across Waves and repositories without workers or Session
  binding. Prove partial success and uncertain-write reconciliation at each actual
  destination. The skill specifies these rules; registration and UI checks alone
  do not establish agent behavior or provider idempotency.
- Gate owns the broader automated acceptance suite and configured build coverage;
  demo owns appearance, native drafts and the real capture-to-owner handoff.

Check: 2026-10-02 `cargo test -p loopflow --lib engine::builtins` 14 passed; `scripts/test_desktop.sh --filter TaskCapture` 4 passed (including production launch subprocess); `scripts/test_desktop.sh --filter WorkspaceNavigationTests` 26 passed; `swift build --package-path swift`, `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check` passed; native menu/provider handoff deferred to gate/demo as above.

## Approved next implementation: configurable session skill

Jack Heart requested a pursue Flow to implement the reviewed New Session skill
picker. The self-contained accepted prototype is [session-picker.html](session-picker.html),
now retained in this Task checkout. Read [capture-task-demo.md](capture-task-demo.md)
for the acceptance history and exact final styling.

Implement the compact split row: New Session launches the selected skill;
the adjacent skill name and chevron open a searchable picker. Selection updates
the next launch without launching. Start with capture-tasks and remember the
selection per repository. Use real lf-owned skill discovery, not the prototype's
illustrative hardcoded inventory. Keep existing repository/Wave scope inference,
Task selection and retained panes. Preserve the existing opening prompt; do not
add the proposed “What would you like to work on?” greeting.

Match the accepted readable label weight, tight left-aligned spacing and chevron
alignment. Provide keyboard search/navigation/selection and Escape dismissal.
The prototype's browser storage and simulated launch are only demonstration aids;
use the native app's existing state and launch ownership. An Edit skill action
was discussed as a possibility, not accepted required scope.

Verify actual configured Desktop launch as well as focused headless behavior.
The current machine has a temporary ~/.lf/skills/capture-tasks.md symlink to the
Task's builtin source to unblock demo discovery; that is not shipped packaging
or proof that the released runtime contains the skill. Preserve authored pursue
review boundaries. Jack's launch request does not constitute demo acceptance of
unimplemented native picker behavior.
