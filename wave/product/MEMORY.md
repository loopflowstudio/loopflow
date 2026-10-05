# product wave memory

Renamed from `concerto` in the 2026-07-08 wave/project/task restructure. The wave's
scope widened past the Mac app: product now owns the shared API and every surface
(CLI, Mac, iOS, agent turns, workers). Older notes below still say "Concerto" where
they mean the Mac surface.

## Task conversation correction (2026-10-04)

Jack Heart requested one ongoing Task conversation for interactive design/review,
with ordinary `lf -b` headless work. Product labels are Session for interactive
and Run for headless; both retain AgentSession identity/history through mode changes.
Any number of Task conversations remains valid, independently of primary selection.
One Waiting state and `--waiting` replace needs-me; `--interactive` stays a mode
filter. Provider signals plus quiet time without outstanding tools may yield false
positives. Claude uses only binary stream-json, never SDK/hooks/permission hosts.

Jack requested deleting Task-worker/managed-Flow authority, mutable Flow switching,
and automatic database-backed recovery; callers inspect history/effect evidence
and own recovery. Delete `lf session ready`, `lf session complete` and their review
handshake, including other completion consumers. Conversational feedback supplies
the exact boundary result without closing the conversation or a renamed handshake.
These decisions supersede conflicting older constraints below, not retained
membership, native surface ownership, historical attribution or proof obligations.

Jack subsequently approved conversation-stage workflow nodes with operational-Flow
edges. The native Task conversation owns navigation, without a shared playhead or
review-settlement API. Operational Flows contain autonomous loops/XORs only and
cannot nest human workflows. Autonomous deciders use `loop-or-next`; duplicate
authored id/name fields are unnecessary. This supersedes the sleeping-runner and
single-capture human-segment proposals preserved at
`5090f672e:scratch/focus-on-your-own-work.md`.

Design review is approved; the full runtime cut remains. Scheduling/enrollment
and five scheduling columns are deleted; Task/PR checks share delivery reconciliation
and per-landing locks, preserving CI repair holds. Ready/Complete CLI/store/Desktop
controls, `exec_driver` and review-completion restart are now deleted. The draft archives feedback
in Session observations before dropping its live column; historical feedback and
completion remain readable without readiness or navigation authority. Task-worker
claims, managed selection, saved resume and new review launch still exist.
Compression removes duplicate Flow feedback plumbing; Session observations retain
the history. It does not remove those controllers. Ordinary detached launch, Task
primary selection, Waiting and workflow/loop authoring remain unimplemented.
Focused migration, projection, CLI, Desktop and crash/effect checks establish local
behavior only. Preserve main's confirmed-dead completed-provider admission exemption
and Session fencing outside async waits. No live Home migration or configured
acceptance follows from these checks.

Saved pursue retains demo although
current source does not: templates cannot establish a saved invocation's shape.
Main `16fa97425` adds native-human-input recency for `lf resume`; reuse that ranking
for initial Task selection, with unfinished Task membership and explicit-primary
precedence. Resume also admits completed history. This supplies no configured
workspace proof. Remote/performance/defaults/website scope stays LOO-353;
LOO-366/367 retain separate policy scope.

## Capture and configurable New Session (2026-10-03 UTC)

Jack Heart accepted capture on October 1, then selected New Session and the
Linear-style compose row on October 2. Jack approved the searchable skill-picker
prototype and requested pursue through its human demo boundary, preserving the
existing opening prompt. New Session launches; choosing a skill only updates the
next launch and persists per repository. The default is `capture-tasks`.

Tasks express intention. Favor cohesive behavioral promises; several Tasks across
Waves/repositories are supported, never a quota. Launch scope is a revisable clue,
not a filing boundary. Capture stays in its repository/Wave conversation, files
self-contained briefs without workers, and leaves operation to owners. Wave
conversations read the same skill without changing their existing authority.
Keep design and general/Task conversations. After an uncertain write, reconcile
in its destination and retry identical input there; preserve each successful filing.

The branch uses lf-owned discovery and explicit `skill <name>` launch syntax to
avoid Flow-name collisions. Repository presentation retains `(Home, checkout)`
identity and both layouts without changing Task selection. A successful app build
proved insufficient when its configured CLI lacked the skill: configured runtime
and shipped packaging need their own evidence. Six focused post-sync tests cover
selection, scope and production-control layout/error behavior; native picker
interaction, provider/draft continuity and real cross-repository capture remain
unproven. Prototype approval supplies no native acceptance or sustained-use KR.
The accepted design and dated demo evidence are retained at
[the branch checkpoint](https://github.com/loopflowstudio/loopflow/tree/25f183992969f59b42f9765d7594638cc91652dc/scratch).

## Session working set (2026-10-03)

Jack Heart requested unfinished interactive participation in ordinary API and
Desktop Session navigation (LOO-372). Task association still includes all work.
`--needs-me` narrows the selected mode; explicit headless/history filters retain
full inspection. Filtered absence cannot release native surfaces or clear drafts
and selection. Explicit completion remains separate from turn completion.
Local headless regressions cover visibility, counts and refresh/completion races;
they establish no sustained-use KR or configured provider acceptance.

## Current Tasks and completion history (2026-10-02)

Jack Heart requested current work without obsolete duplicates, completed Tasks
hidden initially, and Show completed with 7 days, positive N days and All time
(LOO-369). Coordinator guidance limits that control to successful completions;
it is not attributed as a separate decision from Jack. Canceled/duplicate history
remains in the shared inventory and Linear. Window-local filters affect displayed
rows and their count, never Task lifecycle, Flow settlement or Session membership.

The observed Growth snapshot already contained canceled states; the Task summary
dropped them. Preserve provider state separately from successful completion and
carry actual completion time. Unknown dates belong only in All time; reopening
makes planning current despite an old timestamp. Equal-revision storage enrichment
may fill a genuinely absent completion-date key, but cannot change an observed
null or date. Snapshot acquisition and unrelated updates cannot establish recency.

Jack relayed the coordinator finding that settled Tasks with removed checkouts
still carried recovery conditions. That condition alone must not bypass history
filtering. Current planning, nonterminal runtime, unresolved pinned Flow execution
and observed unsettled files/commits remain visible; open Sessions stay reachable
independently. This agrees with the Task workspace's checkout association contract.

Jack Heart subsequently approved the inline checkbox/number prototype and
requested Desktop implementation. Completed enables the remembered range
(initially seven days); the active label edits N Days, with zero shown as All
Tasks. Enter/blur applies, Escape cancels, and invalid drafts preserve the range.
One native text surface handles display and editing to preserve glyph placement;
long integers scroll within the compact viewport. Wave identity bounds drafts.

The earlier affected gate and configured app compilation passed. The later inline
revision and compression have seven focused passing tests and SwiftPM compilation;
queue gate and native visual/focus review remain separate. Jack's Growth screenshot
shows six current Tasks without the seven duplicates. The isolated offline capture
has no successful completions or Sessions, so it cannot prove recency or native
Session continuation. Live refresh and retained-Session review remain unproven.
See [the review evidence](../../docs/reviews/task-history.md). These observations
provide no sustained-use KR or external-progress credit.

## Ask removal decision (2026-10-01)

Jack Heart requested ordinary headless failure without an escalation conversation,
reporting command, notification queue, or required handoff. The responsible Wave
operator reads existing status and logs, resolves authorized impediments, and
uses its ongoing Wave chat for necessary judgment. Taskless failure returns to
its caller without creating a Wave or conversation. Known failed work stays
stopped until explicit recovery; retry retains the Flow position and conversation.

Jack clarified “no, Task sessions stay”: Task conversations and authored human
reviews retain their feedback and completion contract. Persistent versus one-off
Task conversations remains open. Historical Ask rows become ordinary conversations
without replacing the Wave chat or granting Flow settlement authority. Older Ask
identity and caller-release notes below describe the retired model.

The local Ask removal implements this boundary with provider simulations and
store recovery coverage; full Rust and Desktop acceptance remains with gate.
A successful provider turn is not reclassified from prose alone. This records
Product’s shared Session contract, not a Task or Wave placement decision.

Reconciliation with the October 1 Session attention change preserves `--needs-me`
for current reviews, ready conversations and recorded interactive replies.
Converted Ask history follows ordinary conversation rules; its former kind alone
creates no attention obligation. Conversion itself never completes a Session.
Later confirmed owning-driver exits can retire unassigned, non-primary
conversations; Task/Wave conversations and Flow reviews remain open. Missing
process evidence grants neither retirement nor Flow settlement. This is source
inspection, not full Desktop acceptance.

## Task workspace and primary Sessions (updated 2026-10-01)

Jack Heart accepted a Task workspace that keeps Sessions, shells and files
together while background Flows prepare the next interactive stage. Direct
Flow reviews remain their own conversations. Jack’s October 1 correction makes
all conversations in a Task checkout Task Sessions, regardless of attention or
Flow membership. Repo and Wave Sessions retain their explicit scopes and are
excluded. Any number of Task Sessions is allowed. `session list --orphan` filters
Sessions without Task association; it provides no creation opt-out. Desktop keeps
that diagnostic inventory under Debug → Sessions, outside ordinary navigation.

Earlier primary-runtime planning selected eager repo/Wave Sessions and a Task
conversation on launch. The October 1 correction supersedes any interpretation
that a Task has only one Session or that it owns every other conversation. Repo owns onboarding and last-resort help,
including without Waves or PM; Wave combines autonomous operation and emerging
design; TaskSession operates the existing Task Flow authority. Primary repo/Wave
Ctrl-C replaces the conversation. TaskSession Ctrl-C remains undecided and must
never silently restart its Flow. Ordinary reads remain read-only.

Jack's “switch now” and “finish, then switch” replace a captured invocation while
retaining Task identity, TaskSession and files. “Finish” stops at the next loop
point of the innermost active loop before its decider. Durable acceptance must
serialize with worker claims and retain the captured successor across recovery;
checkpoint final writes after confirmed stop. Crossing repeat intervals and an
already-claimed decider need explicit resolution, not an invented inner boundary.
These are required implementation mechanics, not delivered behavior.

Earlier planning accepted automatic Wave wakes for operational blockers and
reading existing Task/Session output. The Ask removal decision above supersedes
this escalation plan and its direct-Ask caller contract. A repo-associated Session may request repo attention with
original evidence and a reason. These asynchronous requests never release direct
Asks and do not introduce a messaging UI. Upstream `3dc89bc9a` already removed
the resident/listener, external chat bridge and turn claims. The outbox survives
without a production dispatcher. Establish primary-owned claims and receipts;
do not restore that service or assume its journal still provides recovery.
Prove structured wakes and interactive draft input through one native execution
owner before primary cutover. A second Harness or terminal keystroke injection
does not prove that integration.

The Task workspace implements Rust-derived checkout association
and a paged directory browser independent of Project hydration and PR diff bases.
Keep `(Home, resolved checkout)` location separate from Run attribution and Flow
membership. Batch placement reads must propagate failure, preserving stale UI
inventory. Readable symlinks are read-only; refresh access before equal-revision
shortcuts so an identical-content replacement disables editing and autosave
without losing drafts. Comparison failure must leave directory browsing usable.

Home-aware retained panes, collapse/focus, reassociation, participation projection
are implemented on the branch, not shipped. Jack requested retiring Ask; this
branch’s raw retry-key API and Ask caller-link additions have been removed.
The separate Task terminal owner is deleted. Remove only confirmed absent Sessions
from panes and Undo; a repository-local inventory cannot establish absence in
another repository, and a membership move must keep the same native surface.
File visibility derives from retained preference plus zoom, without a second
focus backup. Interaction edges exclude routes through another interactive visit;
otherwise repeat work leaks into the exit edge.

Recorded Rust/Swift proofs, shared fixtures, Clippy and both native build paths
support the implementation; transport fixtures remain simulated. On October 1,
Jack Heart approved a workspace checkpoint and requested LOO-353 continue, then
stopped merging until the revised Task Session model is implemented. The PR was disarmed during the correction. Earlier approval does not certify the new revision or missing proof. Real provider continuation, remote
association and measured layout/idle behavior remain explicit LOO-353 work.
The retained signpost is scheduling evidence, not compositor presentation.

Jack's revised workspace uses a compact toolbar and split-owned selection. One
existing Session starts without the Sessions sidebar; multiple Sessions expose a
collapsible sidebar. Ordinary selection preserves other splits; Command-click
changes visibility without terminating Sessions. New arrivals preserve focus and
manual layout. Primary Sessions runtime moved to LOO-364; LOO-353 retains UX,
remaining proof, Flow defaults/source editing and the website pass.

Jack also accepted ordinary Projects independent of optional chapter coordination
(LOO-366), and a Task admission/completion audit (LOO-367). Task supplies purpose and
continuity to Flows, not separate execution semantics. See
[the dated review and retained design references](../../docs/reviews/task-workspace.md)
and [research](../../docs/reviews/independent-operations.md). These decisions do not
authorize an automatic Project reset, Task cancellation or claims of measured gains.

## CI watcher decisions (2026-10-01)

Jack Heart's decisions on [LOO-365](https://linear.app/loopflow/issue/LOO-365),
in the order he revised them:

- The watcher is an optional helper Desktop owns, or a command a person runs.
  "not this always on 24 7 server that we expect to always be running."
  Correctness never depends on it.
- CI repair needs no owning conversation: the clock starts one ci-fix run,
  deduplicated per PR and failing head. TaskSessions stay conversations with Jack.
- It is repo-wide and does one job. "A PR with no Task is reported, not
  repaired." Any future watcher is its own program, not a plug-in to this one.
- One command, three ways to run it: a terminal, a launchd service, and Desktop
  per open repository. A second copy never repeats a fix.
- "i would prefer this watcher service to the one minute cron." The branch
  retires the cron's repair path only; the cron still resumes Flows and settles
  merges. Deleting the cron outright remains Jack's stated preference.

`lf ci watch` implements this on the LOO-365 branch: REST polling with ETags
detects, and the existing landing check confirms and admits the repair, so the
landing lock, generation and incident reservation are the only claim. Not yet
proven against a real failing landing. Open choices the branch made without
Jack's confirmation: a standalone `lf land` PR with no Task is still repaired;
a failing Task PR nobody armed is only reported; watcher state has no Desktop
view yet (LOO-353).

## Skill reduction decisions (2026-09-28)

Jack reported uncertainty among repair and implementation entry points and
requested a smaller library. Preserve useful operations before composing
process, consistent with Intelligence's
[realign direction](../intelligence/MEMORY.md#reconciliation-and-reusable-skills-branch-evidence-2026-09-28).
Design draws out intent; kickoff turns that intent into an implementation plan
and remains the first product-Task step. Jack confirmed code stops locally,
pursue at reviewed progress, and feature after kickoff/design review, pursue,
queue and Task-completing landing. These endpoints must stay distinguishable.

Jack subsequently requested bringing debug back and making it the default in
demos and examples. Debug investigates a reported failure and fixes its cause;
unbreak prioritizes restoring the broken workflow.
This distinction is the implementation interpretation of that request, not a
new distinction explicitly stated by Jack. Neither requires clipboard input.
Explicit systemic causal investigation stays 5whys. Incident composes
unbreak → 5whys → launch-plan; launch-plan is a planning skill, with no same-named
landing Flow. Expand is removed. Reduce and polish stay optional surveys; Jack
has not selected their removal or conversion into editing passes. Research also
answers conversational codebase questions. Realign owns memory reconciliation;
PR authorship belongs to pr-message and delivery commands consume it.

The local consolidation applies
[Retire duplicate memory skills · PR #1319](https://github.com/loopflowstudio/loopflow/pull/1319)
and removes the older Task-specific and governance report pipelines. QA selects
proof from the affected behavior and repairs authorized defects; an independent
audit stays read-only. Delivery skills leave mechanics to lf and retain separate
publication, reviewer-owned merge, bare landing and Task-completing landing.
Retained launch history supports prioritizing code, queue, design, ci-fix and
review-open-work; missing history does not establish disuse or caller authorship.

Jack requested single S1–S5 skills and clarified that their sequence is VSM.
The source `vsm-operate` Flow composes five sequential agent invocations for
delivery, coordination, capacity, adaptation and identity. Each skill can
investigate and act independently; one shared pass note carries evidence forward.
Repository scope remains the default despite ambient Wave attribution, unless
the request narrows it. Whether `wave/operate` uses that sequence remains open;
its separate operation is retained. Jack's “soften, dont harden” correction
rejects turning “one or two useful moves” into a numeric cap. No action is valid.
Task findings can challenge Wave purpose and Wave findings repository direction;
accepted decisions return to affected owners without acquiring another control
authority or making operation a prerequisite for independent Tasks.

The direct finite passes used installed reads on September 28 local time
(September 29 UTC). The repository roster crossed the invocation's Product
attribution, but chapter/Task reads were unavailable. That was incomplete
planning evidence, not an empty backlog or permission to rotate a chapter.
The cross-Wave judgment reconciled Product usability, Intelligence's reusable
instructions and Infrastructure's execution boundaries; Jack's accepted direction
was returned to this local memory. It did not demonstrate a launched VSM Flow
or a planning write.

The [committed design](https://github.com/loopflowstudio/loopflow/blob/9619d803ee9765de907cc529791507e822c21b57/scratch/skill-consolidation.md)
and [evidence record](https://github.com/loopflowstudio/loopflow/blob/9619d803ee9765de907cc529791507e822c21b57/scratch/skill-consolidation-evidence.md)
preserve the source decisions, scenario simulations and verification limits.
Private-Home checks covered catalog/export pruning with personal overrides,
Flow endpoints and review returns, assembled prompts, alignment and docs;
formatting and all-target Clippy passed. Compression's focused proof passed
26 Rust and four Python checks. These are recorded branch checks, not new
configured acceptance. Five-step VSM scope/evidence handoff and the incident
handoff still need configured proof. Installed adoption, live planning changes
and delivery are not established by this consolidation; no prompting-outcome
KR or external-product progress is earned by source checks alone.

## Named participants and review feedback (curated 2026-09-25)

Curated from the retired [name-attribution record](https://github.com/loopflowstudio/loopflow/blob/1a691ac6a222b95c46859c9c06d162d6442950a4/.lf/name-attribution.md)
and [continuation record](https://github.com/loopflowstudio/loopflow/blob/1a691ac6a222b95c46859c9c06d162d6442950a4/.lf/directions/task-continuation.md).
These preserve detailed dated evidence; curation changes no Task ownership or
acceptance state. AGENTS.md and operating guidance already own Jack's naming rule.

- Personal `user.name`, the current participant and each request's original
  author are different facts. Repository config cannot name every caller.
  `LF_USER_NAME` carries launch participation; forwarded empty stays unknown
  instead of adopting a remote Home owner's name. Detached work starts unnamed.
  Native resume must deliver corrections and clearing to the model, not merely
  change its environment; the update grants no review approval.
- Store each text's author in its existing journal/provider record. Preserve
  unknown on replay; provider IDs own identity and names are display data.
  Publishers/editors need not be authors. No second name store, registry,
  historical backfill or output-replacement filter is needed. Chat projections
  must not fill absent historical names from the current participant.
- Capture a name only for authored text. A preference-read failure once blocked
  a bare Mac interrupt. Control inputs without text must remain independent of
  display preferences. The repaired production loopback path demonstrated that
  regression, not just a mock call.
- Review Complete returns feedback for a following deciding occurrence. The
  recorded pursue loop has review/concept decisions and a separate outer
  demo-feedback decision, both able to return to implementation. A completed
  failed-demo conversation is useful evidence, never a success claim. Runnable
  prototyping and Flow alternatives belong to
  [LOO-297](https://linear.app/loopflow/issue/LOO-297); parallel variant retention
  and joins remain proposals.
- Wave playhead removal leaves Session semantics intact. Task-shell NSView pools
  need per-workspace ownership: a global pool steals terminals between windows.
  Recorded scope retained Session code and background tmux process wrappers.
  Fixture renders do not prove tab/window interaction or native recovery.

The prior name-attribution gate passed 799 affected Rust tests, 229 Python tests,
78 website checks (three skips), 112 focused Swift tests, static checks and the
Mac build. A full Swift package run stopped completing and was terminated;
focused stream checks passed later, but full-package and native-rendering proof
remain absent. Fresh real CLI generations demonstrated Jack/Maya/unknown prose,
not a live Task write or the review-bearing design Flow. Native review resume
still needs anonymous → named → corrected → unknown participant acceptance on
a differently named Home, preserving historical authors and review authority.
LOO-297's attempted read failed on missing Linear credentials; local Task absence
did not establish remote absence. Any sentence repair requires a fresh authorized
issue read. No Task was filed or changed by these records.

## PR authorship and retired UX research (2026-09-25)

- PR openings must explain the user's experienced change. “Try it” is a user
  action and visible result; automated evidence belongs in Checks. Prompt,
  cached copy, renderer and refresh behavior all participate in this contract.
  The [authorship slice review](https://github.com/loopflowstudio/loopflow/blob/033e0758504390e5db9d254fed4964b0dd6da4bc/scratch/prs-and-tasks-review-slice.md)
  records local proof and an agent reader exercise, not human reader validation
  or a live handoff. The [frozen sample](https://github.com/loopflowstudio/loopflow/blob/033e0758504390e5db9d254fed4964b0dd6da4bc/scratch/prs-and-tasks-sample.json)
  and [research](https://github.com/loopflowstudio/loopflow/blob/033e0758504390e5db9d254fed4964b0dd6da4bc/scratch/prs-and-tasks-research.md)
  retain unresolved roadmap wording and PR #1276/#1277 overlap questions;
  sampled rewrites are proposals, not shipped claims.
- Chapter display switches to live state after a complete successful status read,
  including no chapter. Cached authored content is fallback for a failed read
  with a visible stale warning; history remains available independently.
- Retired the local four-stage `ux-research` flow: every checked orientation path
  was gone, and it treated scratch personas/guidelines as durable despite landing
  cleanup. Future UI research should start from current Product code and a real
  user question. Simulated personas generate hypotheses, not customer evidence;
  enduring conclusions belong here after validation.

## Workspace redesign decisions (2026-09-26)

Jack rejected the first native build of the calmer workspace on sight: "It just
looks a lot worse than the mocks. Plus I bet we can even improve on the mocks."
The retro found the build structurally faithful to direction D and tonally
unfaithful: neutral ink where the mock was warm, a shadow halo under every
panel, chrome no mock ever had (July's `WaveLensView`, system buttons, a rose
tab strip, a blue-grey pane header, a green Complete capsule), and a sidebar
assembled from four passes with no reconciliation. LOO-291 implements the fix as
four slices: warm token ramp and surface ladder; one sidebar row component;
one Session toolbar; the seven-step type ramp on Wave and Task pages.

Decisions that outlive the branch (Jack's, verbatim where quoted):

- Light mode only for now; dark is a follow-up "when we're at a happy place".
- Serif for the repo name and page titles, including the Task title ("I'm
  kinda liking the serif task title now"). Sidebar Wave rows are sans
  ("sans-serif wins") with no glyph, no count, no dot ("nothing at all is
  even simpler"); a needs-you dot may return later.
- The earlier orphan control-room direction is superseded by Jack's September 30
  Task workspace and primary-Session direction above. Rust now derives Task
  grouping from checkout identity, including previously unbound Sessions; Swift
  consumes that association without rewriting Run attribution. The global
  Session browser becomes diagnostic when primary navigation is implemented.
- One universal bind from the room, ⌘K and Task rows ("worth making the
  design and architecture simple and universal if it takes a little extra").
  Bind changes a Run's Task, never its name, panes or Flow membership.
  September 30's location grouping is independent of this attribution operation;
  it needs no bind to present a Session under its checkout's Task.
- The Session view has one toolbar: the breadcrumb bar, worktree as a quiet
  mono chip, no pane header with one pane, split/close/zoom by keybind and a
  hover trio, terminal edge-to-edge in warm charcoal. "Similar tool sets on
  both the run and the session" must be collapsed or progressively disclosed.
- Two Flow views: the Task page shows the folded Flow template until a Run
  exists, then the fully unrolled Flow invocation. Both draw the same
  flattened graph in this baseline. September 30's accepted design makes the
  interactive-stage projection primary, with actual automated work inspectable
  on its edges; exact captured occurrence identity remains required.
- The September 30 primary-Session direction supersedes deferring the waveless
  beginner: repository onboarding must work before any Wave or Task exists.
- Perf continues in a follow-up; the Session read-path changes follow the data model.

State-of-the-art baseline adopted: one row component per sidebar level;
state glyphs only when they mean something; a surface ladder instead of
shadows; a seven-step type ramp (Cormorant 34/26/17, Lato 13/11, mono 12);
blue means running and loop region and nothing else; only the running node
animates. Research sources and Jack's decisions are preserved in
[the branch's design history](https://github.com/loopflowstudio/loopflow/tree/be7a02db0/scratch).

Historical follow-up split: LOO-299 (control room, bind, Flow views, ⌘K, dark
mode) after LOO-298's data model; LOO-300 for performance. The September 30
direction supersedes the control-room approach; this local reconciliation does
not establish the current scope or status of those external Tasks.

Jack separated those three follow-ups so LOO-291 can deliver S1–S5, the recorder,
signposts and assign-on-change fixes. The accepted Run/Session tables and universal
bind are future implementation, documented in [Infrastructure memory](../infrastructure/MEMORY.md#data-model-and-performance-decisions-2026-09-26).
Chapter history becoming navigable is part of that approved model; the current
Wave → Task UI is a scope limit, not a prohibition on a Chapter object.
The [scope handoff](https://github.com/loopflowstudio/loopflow/blob/be7a02db0/scratch/deferred-work.md)
records the split. Local proof does not establish merger or Jack's acceptance.

Deleting the colored pane borders also made the six-color allocator, per-pane
dictionary and color fields in close-undo snapshots unused. Compression removed
them, preserving layout, focus, zoom and shell-command replay ownership; its
15-test MultiplexerStore proof covers those behaviors. Trace removed visual
features through their state and undo model so unused bookkeeping disappears too.
See [the reduction receipt](https://github.com/loopflowstudio/loopflow/blob/536b0fd56/scratch/compress-pane-state.md).

## Task files beside the conversation (2026-09-28)

[Read and edit task files beside the conversation · LOO-327](https://linear.app/loopflow/issue/LOO-327/read-and-edit-task-files-beside-the-conversation)
implements Jack Heart's minimal native browser. Jack approved the updated demo
and authorized queue preparation, publication and landing in the
[recorded approval](https://github.com/loopflowstudio/loopflow/blob/611ed031a031e6798652a15b5d250ba0f132a4e8/scratch/file-browser-demo-approval.md).
That supersedes earlier publication holds; approval alone does not establish
merge, Task completion or individual scenario coverage. The
[design and evidence ledger](https://github.com/loopflowstudio/loopflow/blob/611ed031a031e6798652a15b5d250ba0f132a4e8/scratch/file-browser.md)
and [demo feedback](https://github.com/loopflowstudio/loopflow/blob/611ed031a031e6798652a15b5d250ba0f132a4e8/scratch/file-browser-demo-feedback.md)
preserve the failed attempts and superseded proposals. Current usage lives in
[the Mac README](../../swift/README.md) and [CLI reference](../../docs/lf.md).

- **Task placement owns both surfaces.** The September 30 internal cut expands
  the original `scratch/` and `diff` navigator to paged immediate directories,
  with optional Changes over the same documents. It sits beside retained
  Ghostty, with the recorded PR link above only navigation
  and aligned headers. Task context shows its recorded worktree without an
  independent selector. One document per Task/path in the window's existing
  workspace registry shares draft, selection and Undo across both sources,
  File/Diff, sheets and Sessions. Browser visibility survives relaunch; unsaved
  drafts remain window-local. File browsing, read and save do not require a PR
  or hydrated Project; comparisons still require their recorded base.
- **Comparison authority stays in `lf`.** Parent means the active Task PR's
  recorded base, HEAD its resolved worktree commit. The exact SHA pins list and
  patch reads; membership is net difference, including staged/untracked changes
  and rename source paths. Read-only Diff includes the unsaved File draft without
  modifying disk or index. Local reads use recorded placement directly, without
  planning sync, Task reconciliation or Run creation.
- **The filesystem is the shared editing protocol.** Jack chose autosave by
  default, an explicit-save option, and disk winning overlapping edits without
  a conflict dialog. Preserve disjoint local edits. Incoming revisions rebase
  Undo around the surviving local draft, coalescing earlier fine-grained history
  so Undo cannot restore replaced LLM text. IME defers application and Save.
  Observe files and parents across atomic replacement/recreation; a quiet period
  coalesces writes but cannot prove an arbitrary writer has finished. Deletion,
  invalid bytes and I/O failures retain drafts and truthful unavailable/error state.
- **Atomic exchange provides recovery, not compare-and-swap.** A writer with an
  old descriptor can mutate the displaced inode after a successful revision
  check. Deleting that inode can lose the later bytes. Retain displaced files,
  submitted drafts and receipts under Git metadata; never automatically restore
  over another writer. Recovery is uncapped and inspected explicitly, outside
  ordinary content reads and typing. Preserve complete UTF-8/BOM/line endings
  within the 1 MB bound; previews never authorize Save. No arbitrary-writer
  exclusion, extended-attribute or power-loss guarantee follows from this design.
- **Measure first selection without pre-reading the target.** Jack required
  ordinary local file selection under 250 ms. Corrected review measured 54/54
  selections at 60–97 ms for 72 B, 13,594 B and 265,764 B files at 520/800-point
  widths after real startup reads. Exact bytes were checked after bitmap capture.
  These are AppKit endpoints with unflushed OS caches, not physical clicks,
  compositor frames or p95. Startup remained 4–5 seconds. Earlier 2,652 ms cold
  command and 12/18 and 18/18 failing populations remain unexplained; later passes
  do not erase them. ARM SHA acceleration reduced measured executable-hashing
  cost while retaining install authority. Priority/QoS/load controls did not
  explain the slow interval.
- **Native primitives and overall approval have separate evidence.** TextKit
  proved useful selection, Find, Undo and retained-terminal behavior; pathological
  million-byte lines remained slow. CodeEditTextView's comparison failed endpoint
  visibility and wrapped-line responsiveness, so no dependency or smaller file
  bound was adopted. Real filesystem and configured CLI/native checks covered
  reconciliation and Save. Physical IME/mouse, compositor performance and nested
  provider Home continuity were not individually established by those checks or
  Jack's overall demo approval. A wrapper's returned launch argv must retain its
  Home context; successful initial launch does not prove nested commands do.

## Accepted calmer workspace (2026-09-25)

Repo → Wave → Task → named Session is the public hierarchy. Project is internal
chapter ownership. Optimize one repository: connected header/sidebar, started
Tasks only, bottom search. Starting work is durable evidence, independent of
Session/process liveness; inspection never starts Task execution. The accepted
September 30 desktop discovery operation ensures primary repo/Wave conversations;
ordinary list/status reads remain read-only. Wave detail holds its
objective, Current KRs and complete Task plan, including unstarted Tasks.

The accepted visual direction is D: sidebar A, center structure B, mood C.
Use warm cream/burgundy, light selected-row tint/edge, serif page titles, sans
body and literal lowercase mono Flow skills. Task title appears once; its issue
ID in the breadcrumb links to Linear. Use Description; collapse real Comments
with their count below it. Dated updates belong in comments, current scope and
blockers in Description. Omit ELSEWHERE, Local work evidence and preview jargon.

Show the pinned Flow and both Feature return edges, with demo between deciders
and final Advance leading to queue → land. Blue loops/running, yellow pending
humans, green completed steps (including humans), red blocked, neutral stopped.
Each edge counts its own returns; labels use the ordered tuple, including nesting.
Boundary identity remains separate from that display count. Flow name opens
search/typeahead; Start/Resume follow shared legality. Stop & restart requires
confirmation. Pause is deferred; no worker at a saved step means Stopped.

New session sits beside the Task title, independently prepares its checkout and
opens a conversation without starting its Flow. One open Session enters directly;
several lead to named choices. Session names seed from the invoked skill or the
recovered magical-musical generator (history had no animal list); inline rename
and later agent suggestions preserve human names. Session view omits Description.
Exact membership distinguishes current, earlier, past, independent and unknown;
its chip reveals the exact graph occurrence, never a newer same-named invocation.
Recent Runs load on demand; provider and summaries come from recorded evidence.

Navigation retains the existing checkout layouts and native surfaces. Explicit
conversation focus moves breadcrumb and Rename together; companion focus keeps
conversation context. Outstanding rename and delayed New-session feedback retain
the originating identity. Neither navigation nor a matching cwd authorizes client
transfer. Task/Session completion and closing a view remain different actions.

Chapter ownership and active-Run streaming are implemented in this branch;
Sessions still poll. The September 25 signed promotion reused the retained
chapter-bearing Home and preserved all six chapter receipts. Installed-app reads
and a native capture showed real Loopflow/Etude planning; Kata's missing chapter
remained explicit. See [the configured receipt](https://github.com/loopflowstudio/loopflow/blob/be7a02db0/scratch/demo-ready-evidence/README.md).
This predates S1–S5. Jack rejected that composition on September 26; configured
provider input/retention and the two-loop discussion remain unproven. A missing
local chapter may mean the wrong Home is selected, not that provider planning
needs rotation. Preserve the chapter-bearing Home before proposing a new chapter.

### Identity, retention, and counterexamples

- `SessionRecord.run_id` is required for Interactive, Ask, and FlowStep. Prepare
  a resolvable Run before publishing a human boundary; launch consumes it and native
  resume retains it. Ask's caller Run is separate. Boundary IDs still target actions.
  Preparation proves identity, never liveness. Legacy unbound boundaries require
  explicit JSON open; listing stays read-only and reports that recovery instead
  of silently dropping them. Release the preparation lock before waiting on a
  resumed provider, or another metadata open waits for the conversation to end.
- `lf runs --active [--task …]` joins exact capture intervals and existing native
  client receipts to one verified process observation, then resolves typed Work.
  A new marker requirement hid live clients from older launchers; native discovery
  must continue using their existing receipts independently. One Exec can host
  successive Runs. Deduplicate by Run; old live Runs survive history age/count caps.
  Waiting clients remain active; dead clients disappear; read/ownership gaps stay
  explicit. Cwd, unresolved Sessions, and unterminated metadata prove none of this.
- Native interactive history can follow an originally headless Run. Retained
  client namespaces preserve Session discovery after exit; explicit resolution
  removes it. Preserve launch provenance and resolve declared issue/slug subjects
  through shared Work binding. Passive output must also follow native continuation;
  choosing journal output solely from original launch mode loses that history.
- Rust projects Session legal actions, labels/help/reasons, and Work display paths;
  Swift dispatches them. Ready decisions are checked before client stop and again
  at settlement; Iterate also requires a preceding autonomous step. Shared Wave
  ancestry keeps historical Project-bound Sessions reachable across chapter moves.
- Pane ID alone cannot restore a Task's choice: the pane may now contain another
  Task's Session. Retain expected content and save direct Sessions only for their
  exact selected Task subject. Refresh retained Task evidence with each successful
  planning publication; selection-time text can be stale by a transfer gap. Cancel
  asynchronous historical-reference reads on navigation so late results cannot
  replace the newer Task or repository selection.
- Monitor needs its own AppKit responder: clearing focus to nil can select a visible
  terminal. Hidden terminals release input; resize/polling must not steal search
  focus. A retained native view owns focus requests and latest title. Verify ordinary
  dispatched input stays out of both PTYs while Monitor is focused, then require
  exact retained draft and child replies on return. PTY echo alone is insufficient.
- Completion/Approve/Iterate rejection must coexist with a live terminal and survive
  polling; local `resolutionError` is separate from opening state. Complete uses
  reconciliation, not undoable Close view. External disappearance also invalidates
  hidden Undo. Completion after repository navigation cleans its originating
  workspace without changing the current repository's reading or companions.
- Shell-to-Session attachment uses actual client terminal identity verified against
  stdin's PTY, never cwd/title or an inherited marker after external handoff. Multiple
  attached Sessions keep individual actions. Completing one leaves its shell usable;
  closing/restoring a launch pane must not replay the original command.
- Align configured read/action Home and database paths explicitly. The recorded
  mixed-Home app combined successful planning/Session reads with another Home's
  process observation. Preserve selected Home/account context while clearing inherited
  execution/terminal markers at GUI launch; `roadmap --all` ignores ambient Wave scope.
- Ghostty mapped CoreVideo display-link error -6661 to OutOfMemory. The same artifact
  created surfaces with its existing timer renderer. Probe that capability once and
  fall back only on failure; nil surfaces alone do not diagnose physical memory or
  an unavailable desktop. Resolve appearance at the window so native controls and
  custom foreground/background colors agree.

### Measurements and acceptance that survive scratch cleanup

- Keep **hierarchy_interaction_ms** and **task_workspace_ready_ms** separate. Measure
  accepted input to correct rendered/usable rows or destination content, with exact
  identities, truthful active/empty/error state, retained focus/input and frame hitches.
  Retained terminal switches are not provider startup. Capture read/decode/projection/
  layout/presentation phases under one interaction ID without adding product widgets.
- The opt-in `uv run python scripts/desktop_performance.py run --output <new-dir>`
  uses 8 Tasks/4 Sessions and 256 Tasks/128 Sessions, two checkouts and three owned
  cat PTYs. Its eleven scenarios use synthetic active-Run DTOs and forced native
  bitmap capture/text verification plus PTY replies. This is an intrusive capture/input
  endpoint, not compositor presentation or hitch proof. The concurrent writer
  completed 462/462 source-stable observations; the hash-verified [baseline](../../scripts/benchmarks/desktop-performance/20260924-capture-input/README.md)
  now survives scratch cleanup. Preserve begin/end records, failed and
  unstarted attempts, host/build/population/endpoint compatibility, observer overhead,
  and source drift. Twenty successful comparable samples are required for its p95.
- One retained Rust reader now performs explicit cold/recovery receipt scans and
  uses filesystem invalidation for warm observations. Podium retains one foreground
  stream after first Monitor demand; navigation does not restart discovery. Keep
  original native receipts authoritative: the rejected OS-environment locator hid
  an owned live client. Bound pipe decoding/delivery; drain old readers on Home
  replacement, preserve last-good evidence during recovery and the actual fatal
  diagnostic. Capture/input and scroll-refresh receipts do not establish compositor
  hitches, configured provider costs or defensible budgets. Those remain required. Five adjacent panes clipped
  empty-state text in one benchmark trial; changing the journey did not repair it.
- Jack's September 26 split assigns further performance work to existing LOO-300,
  superseding the earlier proposal to file two optimization Tasks. Keep hierarchy
  navigation and Task workspace interaction as separate measurements, consume the
  stable runner and baseline, and preserve identities and retained terminals in
  comparable before/after evidence. The split does not waive missing proof.
- The human rejected the accumulated composition as confusing. Earlier provider,
  editor Cancel/rejection, viewport, and empty-Monitor receipts are bounded evidence,
  not approval of the simplified UI. Preserve configured positive Run appearance/exit,
  combined-pane input and human composition confirmation as remaining acceptance.
  AXWindow role, exact input focus, lock status, and permission are separate facts;
  old failures do not diagnose a new runner. Never replay retired mutation probes.
- LOO-291 retains ten human-selected external-work trials, an authorized directive
  edit, and twenty long-lived-registry trials against published budgets. The external
  workflow/text remain unprovided. LOO-251’s promoted-Ask blocked-caller proof
  was superseded by Jack Heart’s 2026-10-01 Ask removal; retained Task reviews
  still require completion proof. D2’s fourteen-day/twenty-open readiness
  obligations remain. Local PTYs, one cached
  population, or AX count timings do not satisfy these. Earlier fallback attempts
  stopped at resource preflight; the September 25 supervised Xcode compile later
  passed. Neither compile supplies the missing verdict or authorizes removing
  another checkout's active build.
- LOO-185 remains parked until human-selected Discord use is blocked by provisioning
  friction; canvas work supplies no new activation evidence.
- LOO-284's shared contract is implemented within LOO-291, with remaining configured
  all-kind acceptance; keep its identity and prevent duplicate implementation until
  delivery is reconciled. LOO-293 retains passive output, Flow history, all-provider
  continuity, paging/bounds and its configured human demo. Adapt its presentation
  into Monitor's multiplexer instead of restoring a separate Watch route/tree.
  Neither Task is complete because primitives were integrated here.
- Current-Wave filtering belongs to shared CLI reads. Historical `list` retained
  abandoned Task/Project/snapshot evidence and could not be forgotten; exclude it
  from current navigation without deleting history. `engbot` was removed through
  the supported operation in the earlier authorized installation. No raw store
  cleanup or new installation follows from memory curation.

## Chapter decisions and review lessons (2026-09-23)

- **The sealed chapter records the accepted starting plan**, not the provisional
  summer drafts: [.lf/chapters/20260923T000959Z-502f011b/start.md](../../.lf/chapters/20260923T000959Z-502f011b/start.md).
  The interval is 23 September–21 October, starting at 00:09:59 UTC. Gate 2
  accepted the plan; it did not authorize publication. The 24 September human
  amendments above govern current hierarchy and ownership. The summer review judged
  39 KRs (2 hold, 16 do not, 21 unknown); the start freeze contains 42 because
  List adds three previously unreviewed claims. Those are different populations.
- **Product value is explicitly chosen external progress.** Current Work
  direction plus material Task progress in any three of Cube, Etude, Kata, and
  Hootro makes a successful week. An open Session, refreshed plan, settled Run,
  or Loopflow self-hosting repair is insufficient. Small/Medium/Big are company
  review heuristics, never runtime limits. Preserve capacity outside Loopflow.
- **The summer reset clarified ownership through real use.** Product dogfood
  built much of the execution foundation; that was useful discovery, not simply
  work in the wrong Wave. Infrastructure now owns execution and self-hosting
  repair, Intelligence owns evidence, and Product owns the external experience.
  The validated Sessions design is a foundation to finish, not restart.
- **Review rows before telling the story.** Freeze the exact Project/KR union,
  including retired or rewritten claims; keep each report's Run id and recompute
  totals from rows. A definition verdict is not another KR. A complete current
  roster cannot establish complete historical lineage. Missing duration proof is
  unknown unless a dated in-scope counterexample disproves the claim.
- **Keep evidence and user consequences together.** Each Project report needs
  both exact observations and a concise account of who benefited, what changed,
  and why the KRs prove it. Carry, learned, not actually prioritized, and misplaced
  work are separate judgments; review proposes, accepted start applies.
- **Chapter review now aggregates one report per Wave.** The former direct
  Project-review shortcut belongs to the multi-Project chapter. Preserve exact
  historical KR membership and dated reports; current operations must not recreate
  a Project operator. Run settlement and the report together establish completion;
  unknown liveness remains unknown. Archive authority follows explicit Task binding.
- **The baseline cannot recover missing KR history.** PM retains current text
  and `holds` with one overwritten snapshot timestamp. The new start ledger
  establishes a forward boundary; retain later wording changes as dated evidence.
  Append-only PM revisions or provider KR identities remain an instrumentation
  option when a real review needs lineage the archive cannot supply.

## Work and continuity (reconciled 2026-09-23)

- **Work is stable identity, not a process.** Wave, Project, and Task are the
  three Work kinds. A Run records Home-local launch provenance and can be prepared before launch; attribution
  does not grant Work mutation or process-control authority. Provider attempts and observed
  Turns remain execution evidence, and the provider owns Session continuity.
- **Domain structure carries continuity.** A Wave owns `GOAL.md`, `MEMORY.md`,
  cadence, Chat, and metric instruments. One internal chapter Project owns KRs,
  metric targets, and Tasks; the Wave retains the objective. A Task owns its directive, worktree, and serial PR chain. Project and
  Task do not copy parent context or inherit recent Wave conversation.
- **Steer is the one durable authored input.** Chat is its interactive Wave
  presentation, not a second mailbox or history truth. Radio, agent channels,
  machine bylines, and the database message bus are deleted.
- **Another Work perspective is an ordinary Run; interactive work is a
  Session.** Launch `lf --as <work> : <question>` when another agent perspective
  is useful. Ask was removed by Jack Heart’s 2026-10-01 decision. Failed work
  retains its normal logs and outcome; necessary judgment belongs in the existing
  Wave chat. Authored review Sessions return feedback to their recorded Flow
  boundary, with navigation owned by the following decider. There is no agent
  exchange row, answer lane,
  or dedicated answer controller.
- **Wave memory is file-only.** Applicable ancestor `MEMORY.md` files are read
  oldest-first. There is no live memory stream, and recent Wave Chat is not
  ambient Project/Task prompt context.
- **Environment configures a process; it never decides what the process is.**
  Work identity comes from durable state; execution and signal authority must
  be established at their owning boundary, never inferred from a Run id,
  inherited endpoint variables, or a surviving terminal.
- **Backlogs are allowed.** Linear Tasks may exist without a Run; open Runs are
  not the Wave's roadmap.

### Earlier runtime findings (July–August evidence)

Detailed scheduler/server proposals and dated LOO-167/193/195/207 failures remain
at `b908182f5:wave/product/MEMORY.md` under this heading. October 4 retires their
automatic-recovery approach. Preserve the lessons: repeated missing-Flow failures
must not silently retry; containment liveness proves neither provider ownership
nor progress; unknown observation cannot authorize duplicate execution. Durable
Steers must survive provider/app exit, with mid-turn delivery provider-dependent.
The old resident topology is historical evidence, not a second runtime design.

## Model (design invariants)

- `lf` and durable store projections define the product API; Mac and iOS
  consume that model rather than inventing a parallel lifecycle.
- **Terminal-only and agent-embedded Loopflow are normal primary modes.** Chat
  is an optional shared steering and observation surface, never a prerequisite
  for operating Loopflow or an onboarding funnel every user must enter.
- **Wave Chat is asynchronous steering, not a low-latency support chat.**
  Durable inputs preserve order and may wait behind existing Wave work; surfaces
  show delivered, queued, and working state instead of implying an immediate
  conversational reply.
- App surfaces navigate, present, and Steer Work. A view, terminal, provider
  process, or listener is never the source of Work or review-playhead truth.
- A provider session is AgentInvocation continuity, not Work identity.
- Home-local execution, Work continuity, provider history, and direct process
  control have separate owners. The current boundary is documented in
  `docs/architecture/{execution,planning,homes}.md`; earlier open topology
  questions below are historical context, not an alternate authority model.

### Planning authority and historical migration lessons

- Linear owns authored chapter and Task content. No local `projects/*.md`, issue
  mirror, or roadmap table is authoritative. Shared current reads expose one chapter
  summary and direct Tasks; historical Project identity remains readable provenance.
- PM writes resolve stable provider edges, not names or Issue prefixes. Task creation
  selects its Wave's current Project; ordinary Task saves cannot restore an old
  parent after transfer. Wave-linked Initiative identity remains in GOAL frontmatter;
  repository configuration owns the Linear Team.
- `lf pm show --no-sync` is cache evidence, not proof of current provider state.
  `synced_at`, snapshot generation, and source freshness have different meanings.
  A rejected write can follow a successful provider mutation if readback failed;
  retain the draft and report the uncertainty instead of asserting nothing changed.
- Repository Team migration uses the supported `lf pm reteam` operation and its
  complete ownership preview, reconciliation and receipts. July's pending PRD-44
  migration was a dated state, not standing permission to modify a live store.
  Retain old completed/canceled Work and original provenance across migrations.

### The `lf` / Home spine

- **`lf` is the single command implementation.** Local reads query the durable
  registry directly; CLI and app actions call the same Work operations.
- **There is no agent messaging substrate.** Radio commands, channel identity,
  bus tables, cursors, retention, and subscriptions are gone. Durable Steers and
  Work state replace message delivery as product truth.
- **Company Discord is the canonical Wave Chat backing when configured
  (settled 2026-07-21).** An inbound user message becomes one durable Wave
  Steer and the Wave reply returns to the same channel. Restart catch-up,
  deduplication, self-echo rejection, and outbound receipts preserve one
  conversation. The active backing and conversation epoch are explicit: local
  and Discord compose never operate simultaneously, and product surfaces must
  not persist a second transcript. This listener is not generalized into
  Project/Task communication and Discord history does not become ambient prompt
  context.
- **Remote execution runs the target Home's `lf`.** SSH is transport; `lfd`
  keeps Home services and receives webhooks. Neither a remote presenter nor a
  telemetry row acquires execution authority from observing the target.
- Resident crons evaluate in **UTC**, so the product `wave` flow at `0 0 8 …`
  fires 08:00 UTC regardless of host timezone.

## Shared planning and runtime vocabulary

Wave → Task is the public planning model. Chapter/Project identity remains internal
and historical. Shared status, roadmap, cached plan and Rust/Swift fixtures must
change together; hiding a Project array only in Swift retains the obsolete contract.
Run records launch provenance and can be prepared before a provider starts; Session
names human continuity/boundary; Exec supplies process ownership evidence. None can
substitute for another merely because identifiers coincide.

## Swift data path — RegistryQuery is the single reader

- **All data reads converge on `RegistryQuery`** (subprocess `lf … --json`,
  daemon-less) — including Waves, status, roadmap, Sessions, Activity, usage,
  and recent Runs. The
  HTTP-to-lfd-as-API path is **deleted**: `LocalWaveService` (~1500 lines) and
  `WaveServiceProtocol` are gone; ~22 consumers rerouted onto RegistryQuery.
- **`RunStatus` biases to `lf`** — align to `lf`'s lowercase tokens (`running`,
  `ok`, `waiting`, `failed`, `pending`), not the lfd int enum. No invented
  `cancelled`. An unknown status must be **loud** (surface it), never a silent
  `?? .pending`. When `lf` and lfd disagree, `lf` wins.
## Performance — launch renders before any read (2026-10-04)

Jack Heart requested Desktop open on a usable workspace with one loading
vocabulary and one refresh path (LOO-376). On Jack's Home, installed `lf`
0.12.32, single samples: `roadmap --all` 14.5 s, `session list` 12.4 s,
`wave list` 2.4 s, `home id` and `ps` 1.2 s each. No read fits the 1000 ms
budget, so a returning launch shows saved wire text first; that CLI latency has
no owning Task (LOO-375 owns `wt list` only).

- Save the reads' wire text, restore through the live decoder, and strip
  liveness and legal actions before saving. Display evidence, never authority.
  Keep it inside the Home; a different `lf home id` drops content and selection.
- One model owns refresh; views that each start a loop supersede each other's
  reads. A view that builds its own model reopens the blocking path.
- The in-process replay (9.6 ms saved vs 13.2 s uncached, 20 samples) is
  lower-level timing. Agent runs have no Aqua session: first frame, stalls,
  CPU and the 400/1000 ms targets are unmeasured.
- Jack's delivery contract (2026-10-04): land on autonomous checks and honest
  benchmark evidence; rendered startup is post-merge validation, not a gate.
- PR 2 has the app journal real launches and reads in the Home; `timings.py`
  reports them. No real launch is recorded yet: read it before completing.

## Sessions projection and native resume (reconciled 2026-09-24)

The older Ask/Ready/Complete and Task-review control contract is superseded by
October 4's Task conversation correction. Its dated implementation and proof
notes remain at `16fa9742591e3edfc0ed42c913c64347c02ffeb5:wave/product/MEMORY.md`
under this heading. Retain these independent constraints:

- `lf session list --json` owns the shared Session projection; Desktop owns no
  second queue, title store, liveness model or resolution state.
- Provider history owns native resume. Row selection focuses the retained
  terminal; **Move here** explicitly replaces an exact Loopflow-owned client
  using PID and birth evidence. Resume preserves history, not unsent TUI input.
  `session open --json --replace` prepares argv without stopping the old client.
  Closing a pane or provider exit resolves nothing.
- Desktop retains terminals across selection, with explicit splits and reversible
  Close view/Undo. Viewing, running, elsewhere, opening and retry remain distinct.
  Canonical Git common-directory identity keeps Task checkouts out of repo roots.
- Configured provider continuation and native focus/pane reconciliation remain
  unproven by mocks, launch screenshots or empty inventories. The deleted review
  handshake is no longer an acceptance requirement.
- LOO-251/284/291 retain native, shared-action and planning proof obligations;
  the archived reconciliation establishes neither completion nor new follow-ups.

### Task observation and Watch (2026-09-23)

- **Incomplete observation caused duplicate implementation.** Desktop launched
  two bound `implement` helpers into LOO-293 while its original worker was still
  producing output. `ready`, a completed launcher, and a missing `lf ps` row
  cannot establish idle execution or resolved review work. Recover advancement
  through idempotent Task controls; ordinary bound Runs remain intentional
  independent contributions with attribution but no Flow claim.
- **Use the shared execution reason in every surface.** Task status, Wave
  conditions, roadmap, and action recommendations now project FlowPosition and
  exact worker evidence. Running, starting, unknown, blocked, and waiting for review
  evidence takes precedence over dirty files or a future-launch refusal.
  Done/Abandoned Work can still retain an unresolved review boundary; preserve
  its waiting condition without reopening the Task. Runtime `project_id` is the
  owning Project; the unread duplicate `routing_project_id` is removed from Rust,
  Swift, and current fixtures.
- **Watch belongs to Product / Desktop's existing LOO-293.** A provider refresh
  during this curation confirmed it remains open and already embeds the full
  accepted draft, assumptions, and newer native-read contract. That supersedes
  this branch's unresolved-placement/native-contract notes. Keep one connected
  stage diagram and labeled all-Run feed, stage/Run filters, Follow live, and
  completed history together. Watch includes passive native interactive output;
  terminal attachment, final-only output, or autonomous-only coverage cannot
  satisfy acceptance. Existing Session controls own review decisions.
- **Preserve history at its owning transaction.** Exact invocation/stage/attempt
  bindings must survive FlowPosition replacement and completion. Do not join by
  skill name, infer completion from the cursor, or reconstruct an old plan from
  current YAML. Native output requires source/item revisions beyond Run event
  sequence numbers. UI state and polling remain presentation, never authority.
- **Keep proof boundaries explicit.** The observation repair has source-binary
  discovery/prune evidence and isolated status/DTO tests; it does not demonstrate
  Watch or deployment to the older installed runtime. LOO-293 retains native
  capture, bounded history/discovery, and configured live/demo-review obligations
  in its current Linear directive. No duplicate follow-up or completion is
  warranted. Historical `f56f457a0`/`c3bd1fdb3` show Wave output and stage pills,
  not confirmation of the exact remembered Task screen.
  Reconciliation on 2026-09-24 confirmed the PM directive still retains these
  obligations and newer foundation evidence from other work. This branch's
  observation repair and prompt cleanup do not validate that concurrent Watch
  implementation or authorize settlement of LOO-293.

### Terminal ownership and input (branch evidence, 2026-09-22)

- Each window owns a repository workspace registry, which retains checkout
  layouts and one terminal view pool across outline/content and repository switches.
  Each view owns its Ghostty surface. A global Session-ID surface registry caused
  one window's release to destroy another's terminal; it is deleted. Do not
  share an NSView between windows or evict a hidden Session to save memory:
  freeing a surface ends its PTY. Window close/app quit still end embedded
  clients; history resumes, unfinished drafts do not survive.
- Session identity, provider process, local opening/error state, terminal view,
  and pane placement have different lifetimes. Keep them separate. In particular,
  `.prepared` bridges async opening and surface creation, and an open failure
  stays visible until retried or superseded. Shell panes also survive navigation,
  and closing an outer checkout slot only hides its entire inner layout. Explicit
  inner close/process exit closes that shell. September 30's Task workspace removes
  the separate Task terminal owner and uses these retained shell/Session lifetimes.
- `TerminalIdentity` carries Session or shell purpose through
  views, pools, and bell/title/close notifications. Input policy follows the
  enum case, never string prefixes. Resolve callback identity from surface
  userdata while its handle is valid; deferred notifications carry values.
  Late callbacks must not discard replacement views. A dead/released view must
  never relaunch a retained `--replace` command and steal a client back.
- The displaced launcher needs **stop intent after liveness ends**. Record
  moved/completed before signaling; consume it to print a clean handoff message.
  An unmarked SIGTERM remains an error. Clear stale intent when publishing a new
  client so PID reuse cannot inherit it. This record is not duplicate liveness.
- AppKit offers key equivalents to sibling views. Only the first responder may
  consume terminal Command-V; a focus border alone proves nothing. The regression
  dispatches through a parent and reads both real PTY buffers. Copy reads actual
  Ghostty selection; core clipboard requests must receive a completion callback.
- Session clipboard images use the provider's Ctrl-V image shortcut. Copied file
  URLs win over image bytes and insert escaped paths; raw dropped image data is
  saved as readable PNG before insertion. Retain temporary data through provider
  consumption. A path appearing is not proof of an image attachment: inspect
  the composer. Shell paste uses paths, not provider shortcuts.
- The 2026-09-22 interactive demo confirmed retained Session switching, copy/image
  input, and explicit Warp handoff. A later demo exposed split paste misrouting;
  the automated real-PTY correction passes but its final reviewer confirmation is
  still pending. Latest identity pass recorded 45 tests in four suites, then 24
  affected tests after review. These are prior-run receipts, not fresh update-wave
  validation or an all-provider matrix. Hosted UI initialization was canceled by
  LocalAuthentication; it supplied no behavioral result. JSON prepare-without-kill
  has source/Swift contract coverage but no focused Rust behavioral proof yet.

### Shell command blocks and build fidelity (2026-09-22)

- Reviewer acceptance is **visible grouping before interaction plus one ordinary
  click anywhere in a completed command/output region selecting both**. Invisible
  OSC 133 metadata, triple-click gestures, or tooltips do not meet it. Warp is the
  benchmark: full-width groups, persistent separation, restrained tint and left
  accent, whole-surface hover/selection, and a distinct fresh prompt. Preserve
  Loopflow's palette; context/timing and richer actions are options, not mandatory
  copies of Warp. Provider Session panes remain outside shell-block semantics.
- The checked-in Ghostty patch exposes visible block geometry and a separate
  read-block API. Block Copy deliberately avoids native character highlighting:
  the first demo showed two competing highlights. Keep geometry and hover separate
  from one selected `(id, text)` snapshot. Page/pin IDs are not durable across
  reflow/recycling; selection clears when its block leaves the visible list.
- SwiftPM pins the published, checksum-verified patched artifact, not a local
  build path. `swift/GhosttyKitPatches/` plus `loopflow-dev.py ghostty-build`
  carry the reproducible source patch. Use a new artifact version on patch changes;
  publication is a separate authorized action. The upstream surface API did not
  expose semantic geometry; a newer standalone VT API is not automatically an
  embedded-surface replacement. Avoid synthetic multi-click API workarounds.
- Real marked-output PTY tests prove parser→click→pasteboard block copy, clearing
  at the live prompt, window-local release, and title delivery. Wait for parsed
  completed blocks, not incidental prompt text. They do not prove automatic shell
  hook injection or final appearance. `shellIntegrationEmitsSemanticMarks`
  manually invokes zsh hooks; custom prompts and other bundled shells remain
  unproven. Upstream excludes macOS `/bin/bash` from auto-injection. Provider
  `TERM` stays `xterm-256color`; changing it needs separate fidelity evidence.
- **Build parity remains broken.** `project.yml` copies resources but still builds
  Ghostty-disabled stubs, unlike SwiftPM. The later fresh Xcode build-for-testing
  failure supersedes earlier compile-success notes. Binary/resource revision facts
  also remain duplicated, the build recipe does not regenerate the shell payload,
  and missing resources disable all terminals. Give both builds one dependency
  and generated provenance, degrade missing shell resources to blockless terminals,
  and contain resource-environment mutation. Copying payload alone is no proof.
- Remaining block risks are hypotheses to measure: centered-grid math is shared
  by geometry helpers and tests rather than checked against rendered padding;
  per-row prompt scans at 10 Hz hold the renderer mutex and may repeatedly walk
  long history. Patch Zig tests lack a recorded run. Final block appearance and
  corrected split paste need configured-app confirmation. Keep separate command,
  output, and last-command actions, multi-selection, bookmarks, and sharing
  deferred until the core interaction is proven. Upstream API contribution may
  reduce patch maintenance later (research reference: ghostty-org/ghostty#11747).

### Shared viewing boundary

Native launch plus explicit Move here remains the main path. The requested behavior is
optional simultaneous Warp/Loopflow viewing: second attachment view-only, then
explicit **Take control**. Earlier research recommending default tmux presentation
is superseded. `4d5e96383` shared raw resume argv, not a live PTY; `90c871805` and
`7889d65bc` established native presentation. Separate feedback reported tmux
color distortion and terminal bugs; those were not stated in the commit messages.

Compare an opt-in tmux configuration with a transparent PTY relay before changing
that contract. Prove truecolor, keyboard/image input, independent sizes, late
attachment, one provider PID/draft, clean takeover, and view-only enforcement at
the owner. Client-local scroll/selection is separate from durable process state.
Native concurrent resume cannot prove shared PTY continuity. The prior research
reported clipboard-image failures inside tmux (anthropics/claude-code#25672);
retest the exact stack. Control-mode integration requires its own protocol/render
client; a broker also owns replay, flow control, resize, and failure recovery.
App-quit survival and remote Home attachment remain separate scope decisions.
Client provenance is absent today; keep ELSEWHERE generic until the shared API
can name the recorded terminal/location.

The 2026-09-22 reconciliation filed these remaining concrete gaps under Mac
Surface UX: [LOO-280](https://linear.app/loopflow/issue/LOO-280) for build/resource
parity, [LOO-281](https://linear.app/loopflow/issue/LOO-281) for real-shell blocks,
geometry, long-output measurements, and visual proof,
[LOO-282](https://linear.app/loopflow/issue/LOO-282) for client provenance, and
[LOO-283](https://linear.app/loopflow/issue/LOO-283) for the bounded shared-viewing
comparison. This branch does not establish any Project's week/month evidence
window; definitions and KRs remain unchanged. No open Task had enough evidence
to close during this reconciliation.

## Historical remote client

The June HTTP-to-lfd, bearer-token, and Concerto build recipes are superseded by
shared `lf` projections and explicit Home transport. Their dated observations are
preserved in the [pre-chapter memory](../../.lf/chapters/20260923T000959Z-502f011b/sources/wave/product/MEMORY.md)
and the [execution synthesis](../../.lf/chapters/20260922-manual-baseline/execution-architecture-synthesis.md).
They explain the topology change; they are not current setup instructions.

## Historical Wave controls

The retired listener controls, failed-turn presentation and dictation decisions
are preserved in `12016c6d6dd34a4c1a553c25b5cf2529ee93615b:wave/product/MEMORY.md`
under “Historical Wave controls & truthful failures”. They do not authorize
restoring the listener. Attempt failure remains distinct from Work failure;
Wispr Flow owns dictation; configured UI execution requires a capable host.

## Learnings

- **Reshape proven code; don't rebuild beside it** (code only — a rewrite loses
  hard-won correctness). Does NOT apply to the charter: stale framing is a
  liability, so GOAL.md/roadmap get rewritten freely while MEMORY is curated. The
  fresh `RepoSidebarWindow` re-derived the burgundy sidebar / create sheet /
  terminal panes and got each subtly wrong; the proven components already encode
  the right style + behavior — adapt them.
- **`loopflow-dev.py` builds from the worktree it runs in.** Run it from the
  branch checkout. Repository discovery collapses linked worktrees to the
  canonical main checkout through the Git common directory; Task Work remains
  the only surface that presents its worktree.
- **Interactive provider clients resume natively.** Reuse `SessionRecord` and
  `lf session open`; do not restore lfd terminal attachment, a tmux presentation
  path, or Ask-specific Swift plumbing.
- The high-value review move was catching invented fields that duplicate existing
  ones (e.g. `RunStatus`), not re-litigating the approach.
- `cargo test -p loopflow dto_fixtures` filters by test name; use
  `--test dto_fixtures` to run that integration file. Headless runs set
  `LF_RUN_ID`; Rust tests asserting generated journal ids / branch-derived ingest
  must clear it or full `cargo test -p loopflow` fails only under agent runs.
- **Historical migrations demonstrated the shared-store blast radius.** Product
  and Intelligence collided on `061`; editing an already-applied migration left
  existing databases without a required column. Preserve released migrations and
  test upgrades from the released frontier. The specific 057/061 incident and
  proposed remedies are retained in the prior memory linked above; they are not
  current repair instructions. AGENTS.md owns the current one-draft-per-Task rule.
