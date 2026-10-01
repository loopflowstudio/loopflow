# Capture Task

Accepted by Jack Heart — 2026-10-01. Scope, conversation behavior, and Desktop
interaction are agreed. Jack Heart authorized filing under Product and launching
the work on 2026-10-01.

Source: the design conversation in `/Users/jack/src/loopflow.capture-task`.
The working copy after handoff is
`/Users/jack/src/loopflow.desktop-task-capture/scratch/capture-task.md`;
the source checkout retains the accepted handoff snapshot.

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

## Data structures and key functions

Reuse AgentSession, Wave, Project, and Task; add no persisted capture record,
DTO, queue, or draft lifecycle.

Planned Swift value in `ConversationLaunch.swift`:
`TaskCaptureLaunch(repoPath: String, wave: String?)` with
`arguments(lf: String) -> [String]`.
`PodiumModel.taskCaptureLaunch: TaskCaptureLaunch?` derives the selected repo
and owning Wave. A `SessionsView` action opens its retained shell pane using
the existing launcher and configured provider. Desktop carries the skill name,
not a duplicate prompt. Rust `task_create` remains the filing authority.

## Current system and deletion boundary

`ConversationLaunch.swift` supplies inline prompts for general conversations.
`WorkspaceNavigator.swift` exposes those actions; `SessionsView.swift` opens
their panes. Add capture alongside them. `task/skill/capture-task.md` registers
automatically through the builtin scanner. `kickoff` accepts briefs/designs;
`launch-plan` owns execution handoffs. `wave/operate` currently runs one pass
and exits; this feature does not change that lifecycle.

**Delete — do not maintain:** none. Preserve `design`, existing general
conversations, Task conversations, and Flow review boundaries.

**Forbidden outcomes:** capture starts a worker; a Task needs the capture
transcript to make sense; filing permanently binds the capture Session; a
second prompt copy or `create-task` alias; a new background operating loop.

## Internal slices

1. **This slice:** define the self-contained capture skill and its shared
   conversation contract; verify builtin discovery without phrase-matching tests.
2. Wire the visible button and Wave action to the existing Session surface;
   preserve context, unrelated panes, and launch errors. Update `swift/README.md`.
3. Verify the production controls headlessly and review sample conversations
   for vague ideas, already clear requests, duplicate work, multiple owners,
   and failed filing.

## Done when

`cargo test -p loopflow --lib engine::builtins` discovers `capture-task` while
retaining `design`. `scripts/test_desktop.sh` builds Desktop and proves real
button actions open capture panes with correct repo/Wave context, preserve
existing conversations, and expose launch failure. Use the existing Task
creation API; do not add redundant tests for unchanged filing machinery.
Conversation quality and actual owner handoff belong to demo/review, with no
display session or live provider required for gate.

Check: 2026-10-01 source inspection confirmed launch, filing, retry, and permanent Session binding behavior; prose-only changes, no builds run.
