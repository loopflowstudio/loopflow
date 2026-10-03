# Capture Tasks and New Session

Reconciled 2026-10-03 UTC against `25f183992` (including the main integration).
Task: [LOO-368](https://linear.app/loopflow/issue/LOO-368).

## Accepted direction

Jack Heart accepted capture on October 1 and authorized Product placement and
execution. On October 2 Jack selected **New Session**, the Linear-style compose
row beneath repository identity, and preservation of the existing opening prompt.
Jack then approved the skill-picker prototype and requested pursue through its
human demo boundary. The latest steer supersedes the original feature launch and
the Task snapshot's singular `capture-task` and provisional Create Task wording.
The builtin is **capture-tasks**, without `capture-task` or `create-task` aliases.

New Session launches the selected skill; the adjacent skill name opens a searchable
picker. Selection does not launch. Start with capture-tasks and remember the choice
per repository using real lf discovery. [session-picker.html](session-picker.html)
is the accepted visual reference; [capture-task-demo.md](capture-task-demo.md)
retains dated feedback, failed launch evidence and the prototype approval.
Prototype approval does not establish native acceptance.

Tasks express intention; no separate intention record is introduced. Capture can
produce several Tasks across Waves and repositories, correcting the initial scope.
The dedicated conversation makes Tasks for their owners to operate. Existing Wave
conversations share capture without changing their operating authority. Keep
`design` and general/Task conversations. Retiring design, persistent operational
conversations and background operating loops remain outside this Task; LOO-353
owns the broader workspace experience.

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

Capture consults `lf wave list --json` and relevant `lf wave status <wave> --json`
in each destination repository for objectives, current plans and overlap. Each
scoped CLI call uses that destination as its working directory; the capture
Session itself stays in its original checkout. File through
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

Wave conversations share the capture skill when an idea needs shaping. They retain their existing operational authority;
capture neither grants nor withdraws permission to operate other Tasks.

## Implemented Desktop contract

The split row below repository identity and the Wave context-menu New Session
entry launch an ordinary interactive skill conversation. Overview uses repository
scope; a selected Wave uses itself; a Task uses its parent Wave. An explicit Wave
menu action captures its own Wave without selecting it. Launch remains in the
repository checkout, never the selected Task checkout. An unresolved Wave falls
back to repository scope. No repository means no entry point.

`SessionSkillLaunch` emits `lf --mode interactive [--wave W] skill <name>`.
Explicit `skill` disambiguates skills from same-named Flows. It replaces the
fixed `TaskCaptureLaunch`; `ConversationLaunch` remains separate because general
conversations have inline prompts and Task checkout/binding behavior.
No Desktop copy of the capture prompt or new greeting was added.

`RegistryQuery.sessionSkills` reads `lf list --json` in the selected repository,
filters skill entries and sorts names, retaining repository-local and namespaced
skills. `SessionSkillPicker` uses `PodiumReading` for loading/error/available state,
searches names/descriptions, and implements arrows, Return and Escape. Opening
refreshes discovery; failed reads expose Retry without selectable stale entries.
Selection persists in native preferences keyed by canonical repository identity.
Writes merge saved repository choices. A disappeared skill stays selected and
surfaces the CLI error on launch rather than silently substituting another skill.

`SessionsContentView.launchSessionSkill` resolves the configured control CLI before
changing presentation. It selects the repository's `(Home, checkout)` workspace,
sets the existing navigation owner's `showsRetainedTerminals`, and creates a shell
in that workspace. Task selection and both checkout layouts remain intact; normal
subject selection restores ordinary Task presentation. A helper-resolution failure
uses the existing launch alert without layout changes. CLI/provider failures after
launch remain visible in the shell. No new execution owner, DTO, queue or schema.

The native row uses the accepted 34-point height, matching 13-point labels,
12-point skill padding, seven-point gap, raised 12-point chevron and five-point
corners. The sidebar is 320 points wide to fit the default skill. Native font weight
and proportions remain subject to appearance review. Edit skill was discussed but
was not accepted scope.

The builtin is registered automatically with a catalog description and
`requires: none`. Wave/session's former competing direction paragraph now reads
that skill in its existing conversation. The Mac README describes discovery,
selection and launch. Capture leaves filed Tasks to ordinary roadmap refresh and
returns links without redirecting Desktop.

## Evidence and remaining acceptance

Recorded checks before reconciliation: 14 builtin tests, initial SwiftPM build,
26 navigation tests, formatting and all-target Clippy passed. The latest compression
and post-sync Desktop rebuild each passed all six TaskCaptureTests. Those tests
cover catalog decoding/failure, per-repository selection without launch, scope,
explicit Wave launch values, absent repository and the production button's layout
preservation/helper error ordering. The production test uses an owned helper host;
it does not establish provider launch, native keyboard dispatch or live drafts.
No code changed during this reconciliation, so these focused results were reused.

Remaining work preserves the complete original acceptance:

- Gate: affected automated suites and configured app/build coverage. Confirm the
  configured runtime supplies the skill and explicit launch syntax; a branch-built
  app alone did not do so in the failed demo. Development workarounds are not
  release packaging proof.
- Demo/review: judge the native row against the approved prototype. Exercise
  search, arrows, Return, Escape, empty/error/retry states, remembered selection,
  and the actual Wave context menu while another subject is selected. Choosing
  changes only the next launch; New Session launches the chosen skill.
- Configured Desktop: launch from a Task with existing Task and repository panes.
  Show the repository conversation without changing Task selection, preserving
  both layouts and live drafts. Prove real provider continuation and launch errors.
- Capture-to-owner: explore vague and already-clear ideas, reuse duplicate work,
  correct initial ownership, and file useful self-contained Tasks across Waves
  and repositories without workers or binding the capture Session. Continue with
  a second idea in that same conversation; owners can act from Task text alone.
- Partial/uncertain filing: preserve a successful Task when another fails;
  reconcile the uncertain destination before retrying identical input there.
  Prove no duplicate batch and no substitute filing in the launch repository.
  An inaccessible destination leaves an explicit unfiled brief.

No unresolved product decision blocks the accepted implementation. Native
appearance and the real handoff still require the authored review. The recorded
managed/direct Flow discrepancy remains unresolved runtime evidence in the demo
note; source reconciliation establishes no new worker state or Flow settlement.
No sustained-use KR, external-product progress or delivery is established here.

Review found stale fixed-launch types, provisional labels and completed build steps
in the previous plan; these are removed rather than maintained as a second design.
Historical implementation sketches and receipts remain at
`25f183992969f59b42f9765d7594638cc91652dc:scratch/capture-task.md`.

Check: `git diff --check` passed; prose-only reconciliation reuses the recorded six-test post-sync pass; configured interaction and capture handoff remain with gate/demo.
