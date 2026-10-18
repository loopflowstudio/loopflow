# LOO-489 — Native navigation implementation

Current target: Console hybrid after the Claude pass, October 17, 2026.
Jack Heart approved the current navigation-studies source, stable Task order,
quiet Dark titlebar and truthful previews, then authorized pursue and a Swift app
demo. This guide supersedes older presentation briefs. The HTML is a study,
not native proof. Shortcuts: keyboard-shortcuts.md. Pre-implementation detail is
preserved by the evidence reference below.

## 1. The experience

Open a repository window and immediately see the working set. A Task has one row,
a stable position, a recognizable title, a small truthful activity preview and
useful metadata. Click it or use its number to return to its retained input.
Waves and repository switching help find more work without taking over the list.

One titlebar contains native traffic lights, collapse, Back/Forward, repository,
optional Wave, Task ID and title, then Shell / Files tool toggles and contextual
actions. Remove the second global heading/navigation band. Preserve compact pane
headers only where multiple existing panes need identity; do not duplicate the
same header around a single terminal. Keep native tabbing disabled as currently
configured. The repository names the window; no extra Loopflow product label.

The center remains the existing native pane tree and Ghostty terminal. The sample
HTML transcript and editable prompt are scaffolding for navigation/draft review;
never port them as a chat renderer or add a second composer to a provider terminal.
Each Task presents its Session, one Shell tool and one Files tool. Shell holds
multiple shell tabs; Files holds document, Brief and Changes tabs. Titlebar Shell
and Files controls add/remove those tools from the visible arrangement. Hiding a
tool collapses its split, preserving its tabs, selected tab, drafts and native
clients; showing it restores them. Compact pane headers appear only when multiple
panes need identity. No separate Brief/Changes companion slot remains.

Jack Heart accepted LOO-497's shared tab model on October 17; the operator's
native-tool-handoff.md now supplies the bounded snapshot. `ShellTab.id` owns
terminal identity and Files owns ordered `{id,path}` tabs. Delete the predecessor
split APIs listed below; no new lifetime/layout owner or multi-work grid follows.
Native toggles preserve clients, caret, selection, scroll and drafts; the HTML's
DOM rebuild and copied prompt text are not retention proof.

Repository and Wave conversations remain first-class Work. The repository name
opens its conversation; the adjacent chevron and Cmd-K open navigation. Do not
add a duplicate repository sidebar row. An empty repository opens on its
conversation/entry surface, not an empty Task dashboard. A Wave pairs its
conversation on the left (~62%) with all current Tasks on the right (~38%).
Each scope owns its own conversation and draft. Neither repository nor Wave scope
offers Shell/Files tools. Opening a scope reuses its existing conversation/entry
path; navigation alone must not launch an agent.

## 2. Visual contract

| Element | Target |
| --- | --- |
| Sidebar | Initial 240 points; resize through the existing divider; prototype explores 200–420. Retain native safe center-width constraints. Persist per repository window using the existing owner. Explicit collapse restores width and scroll. |
| Working row | Current study rows are 84–109 points. Title ≤2 lines; Available gets up to two activity lines, Working/Idle one; full-text help and one metadata line. The unconditional two-line Available rule remains an explicit visual-review question in the prototype notes. No fixed three-line tail reservation or branch line. |
| Text | Lato for names and controls, JetBrains Mono for excerpts/IDs. Prototype title 13pt/1.3, excerpt 11pt/1.4, metadata 10pt/1.4; map through native Typography and honor accessibility. |
| Selection | Neutral fill and outline; distinct from amber attention and blue working edges. Light study #e1d9ce/#c4b8a8; Dark #515a65/#727b85. Map to semantic theme tokens, not per-view literals. |
| Titlebar | Light burgundy #722f37. Dark deeper #43282f with #53343c lower border and #ded3d2 text. This replaces the brighter 70% burgundy blend in the earlier hybrid. |
| Surfaces | Preserve Loopflow Light paper and Dark slate tokens. No burgundy wash on the selected Task. Focus/active titlebar control can quietly echo burgundy without competing with state. |
| Breadcrumb | One text baseline across repository, Wave, mono Task ID and title; icons and the chevron center on the row, not on a glyph. Repo name opens the repository conversation; its chevron opens the navigator. Wave crumb opens its conversation and Task list; Task ID stays visible, title truncates first and exposes full text on hover/accessibility. No inline Cmd-K path segment. |
| Waves | Plain Wave rows immediately below working Tasks in the same scrolling sidebar. One click opens a Wave. Bare counts mean all current Tasks in that Wave; expose “N Tasks” in accessibility/help. Use the native water.waves icon rather than a diamond. No pinned footer or Wave dialog. |

Validate the current row treatment at 1000×760 and 1440×1000 at default text size.
Paused Tasks no longer occupy working rows; do not reuse earlier six-row screenshots
as proof of this revised layout. Larger working sets
scroll normally; do not shrink all text to force twelve into a fixed height.
Preserve row order and a visible scroll anchor as activity changes. No timer-based
pulsing, animated sorting or window-wide redraw on each output chunk. Header
controls must survive the narrow window without overlapping the Task ID; hide an
intermediate Wave crumb before the repository or selected Task identity.

## 3. State and navigation ownership

Use `WorkNavigation` in `WorkProjection.swift` and `WorkModel.select/goBack/goForward`
in `WorkModel.swift`. Reuse `SessionsWorkspaceRegistry`, `SessionsWorkspace` and
`SessionsStore` in `Views/SessionsView.swift`, with existing `WorkLayout`,
`WorkSurfaceView` and `WorkspacePanesView`. No new retained
terminal registry, parallel selection variable or second pane-layout store.

Working-set presentation is a stable ordered list of Task references, separate
from selected Work, history and the sidebar query. Opening a started, unpaused Task adds it once at
the end; reopening an existing Task retains its position. Opening an unstarted
Task previews its brief without adding an empty working slot, creating a checkout
or acquiring/starting a Session. Starting is a separate explicit Task action. Use existing retained
Work references to seed it, then persist the order through the existing window/
workspace state owner. Do not use live attention order as its sort key. Navigator
recency is allowed to change without changing working-set positions.

Membership policy in this revision: explicit started, unpaused Task opening adds; ordinary
selection, filtering, idle state, completion, Wave navigation and closing a pane
do not remove it. Do not add automatic completed-Task cleanup or a new close-Task
operation during this change. A future removal action needs explicit semantics
separate from completing, deleting, stopping or closing a pane. Unreadable work
remains represented as unavailable rather than silently disappearing.

Explicitly paused Tasks are the exception: they leave working rows, Cmd-number
slots and the navigator’s working group, but remain in their Wave and search.
Preserve their retained workspace and stored relative order, so resume can restore
membership without discarding drafts or panes. Pause is the shared Work paused
flag, not an inference from inactivity, a stopped process or Idle. No live line
while paused; display Paused with the pause mark. Opening paused work never resumes
it. Slot stability is guaranteed across attention/filter updates, not an explicit
membership change such as pause/resume.

`Sidebar shows` offers Working tasks / Available / Running inside the navigator.
This choice filters the eligible working set; explicit pause also changes eligibility. Use current shared state/query semantics;
unknown activity cannot match Running merely because a client process exists.
Filter changes preserve selected Work, focus, draft and retained panes even when
the selection is absent. Show a small “Open Task is outside this list” with “Show
working tasks” in that case. Never auto-clear the filter after a number shortcut.

Wave navigation opens its conversation beside a quiet Task list, not a grid or
sidebar filter. Dim other Waves’ working rows while here without moving or hiding
them. Maintain readable contrast and make keyboard focus fully visible. Use the
same body typography: title, useful objective/status and **all current planned and
started Tasks**. Keep Started above Not started, both expanded. Paused Tasks remain in Started
with Paused visibly distinct from Session activity.
Started is shared Task execution evidence, independent of Session activity.
Preserve authored plan order within each group. A Wave with no started Tasks must
still show its plan and cannot render as an empty workspace.
An “In working set” annotation is optional context, never a reason to hide a Task.
No collapsed duplicate group, oversized serif hero, WAVE eyebrow or external
arrow for in-place navigation. The prototype supplies fixture counts; native
uses existing Wave data and actual overview actions.

Rust `work/selection.rs::default_plans` excludes Tasks
without Started and terminal Tasks. Its `ViewPlan.members` is **not** the Wave’s
planning inventory. The Wave inventory uses the existing Wave/project Task reading,
including unstarted Tasks; do not widen the working-set query to all backlog as a
shortcut. Keep repository/Wave counts consistent with the displayed current plan.
Retain completed work through existing history; this revision does not invent a
historical chapter browser. Query/plan read failure is an error, not zero Tasks.

Clicking an unstarted row opens a Task brief in place, with no terminal or pretend
Session. Show a separate Start Task action using existing TaskRunControl/Workflow
eligibility and current review requirements. It must not bypass a required review,
automatically choose a consequential edge or create another conversation just by
viewing the brief. Once execution starts, add the Task to the working set through
the same existing work-open path and keep identity/history. The prototype's Start
Task action previews intent only. LOO-489 does not change execution semantics.


Repo selection uses `RepositoryWorkspace`/`WorkLinkRouter`: focus an existing repo
window or open one, preserve the source, and route a specific Task only when that
was requested. Native Cmd-` cycles windows. Repository-name navigation and the chevron switcher
are separate targets within one crumb. No per-repo process/Dock identity.

## 4. Session previews that stay truthful

A Session is a better source than a shell title because turn boundaries and
availability can invalidate old text. Merely having a Session is not enough:
process liveness is not evidence that a particular summary describes its current
turn. Show the latest agent line while its source is fresh, then fall back to a
clearly labeled summary. Never infer freshness from `readySummary`, title, branch
or an unqualified terminal line. Truthfulness must not remove the activity signal.

The row is derived from one coherent observation. Keep Session identity, provider
attachment/generation, turn identity, phase and preview provenance together.
A preview must identify the Session and turn that produced it; age means the
source event's age, not time since the UI received or rendered it. These are
requirements on the existing shared projection, not a second Swift status engine.

| Observation | Display |
| --- | --- |
| New turn starts; previous turn had a result | Invalidate the old line immediately. Keep Working and show “Summary · …” until this turn supplies fresh output. |
| Current turn has fresh agent output | Show its latest useful line and source age. This is the default preview, not a permanent Task summary. Skip blank/control-only output and streaming partial words. |
| Working Session becomes quiet; its line ages out | Fall back to “Summary · …” without changing Working to Unknown. Freshness of text and liveness of the Session are separate facts. |
| Available for a person | The validated latest response from the current available turn may remain with its age; it is a result waiting for a person, not claimed as live progress. Fall back to the summary if its provenance is missing. |
| Explicitly paused | Paused with summary fallback; no live line. Keep it accessible under its Wave while excluding it from working slots. |
| Finished/idle | Show “Summary · …”; keep prior raw lines in Session history. Task completion is independent. |
| Observation unavailable/disconnected | Unknown status, summary fallback and “Activity unavailable” in the tooltip. A fresh line from an earlier observation cannot claim current work. |
| No usable summary or line | Generic state / “No current activity” or “Activity unavailable”; never manufacture content. |
| Late event from previous turn/provider | Retain in history if appropriate, but never overwrite the current preview. |

For running progress, start with a presentation freshness window of 30 seconds
from the source event, invalidated earlier by turn/provider replacement or lost
observation. This only selects line versus summary; it must never decide execution
state. Treat this as a tunable presentation constant, not a new user setting or
server status. A current Available result is not subject to a running-progress
TTL, but shows its actual age. No fake “live” dot or renewed timestamp on redraw.


The shared Session producer owns freshness; a Program Status reporter ID is not
a native turn ID. Preserve report clear/reset semantics and consume provenance
through Work observation, without acquiring providers for sidebar display.

Implement the latest-line path and summary fallback together. Enable the latest
line when the producer proves its Session/turn, source time and observation validity.
Generic labels are a missing-evidence fallback, not the intended steady state.
If projection needs more evidence, add the minimal optional provenance through the
existing Rust/Swift DTO and fixtures in this integration, coordinating with the
Session owner. Do not invent a public navigation API or reimplement provider
stream parsing in Swift. Missing provenance must use the labeled summary fallback, never an optimistic
guess. An age bound alone cannot prove freshness; turn and observation validity
must also hold. Do not add an LLM summarization loop to produce the fallback.

For multiple Sessions on one Task, state and excerpt must describe the same
Session. Preserve existing deterministic status selection: attention-bearing
Session first under the shared precedence, then task-primary where applicable.
When the preview isn't from the displayed/primary conversation, identify its
Session in the tooltip/context action; expose multiple waiting conversations in
existing Task actions. Do not concatenate one Session's status with another's
text or redirect ordinary Task selection to an arbitrary newly active Session.

Do not acquire a Session, scrape a terminal or subscribe to every transcript just
to paint the sidebar. Consume the existing Work observation. Strip control and
format characters using the existing display-safe function, preserve source text
in its owner and expose a bounded full preview on hover. Keep updates bounded and
preserve row height/scroll. Reuse the existing Session reader for a bounded latest
agent line rather than mounting or acquiring every provider terminal.
Use current data snapshots, not a new summarization agent or periodic LLM calls.

Prototype's study-only selector demonstrates fresh Session line, quiet summary
fallback, new turn / old result and observation loss on LOO-489. Sample source ages
are fixtures; this is outside product chrome and is not a native setting.

## 5. One navigator and contextual actions

Extend `Views/WorkPalette.swift`, keeping its focus restoration and routing.
Empty search starts with working Tasks by recency, then other Tasks, Waves and
repositories. Exact issue IDs outrank broad matches. Label groups and repository
identity so same-named Tasks cannot be confused. Opening the current repository
focuses its window; it must not reset to a blank home.

Reuse one small native action description (identity, label, eligibility, shortcut,
invocation) across native menus and Task context menus. Cmd-K navigates Tasks,
Waves and repositories. Cmd-/ opens the same **Task-only** action menu as ••• and
row context click, targeted to selected/focused Work. No Navigate, Hide Sidebar
or other app commands in that menu. Keep generic commands in native menus and
keyboard reference. No Wave chooser and no separate action-search dialog.
Keep query syntax/results separate from changing the sidebar's filter: typing a
search never closes the selected workspace or launches its matches.

Actions for a Task: New conversation, Open in editor, Move to Wave, Complete Task,
open owning Wave when present, copy Task ID/link, copy branch, open recorded PR URL,
and existing Session actions where eligible. No fake PR buttons when publication is absent. Context
menus target the clicked/focused row without changing selection, input focus or
starting execution. The prototype’s external/mutating actions preview their exact target and intent
without launching providers, opening editors or changing Work. Native actions must
execute through existing owners and show eligibility/errors, not leave toast stubs.
`WorkModel.completeTask` / `RegistryQuery.completeTask` already exist; use their
authoritative completion semantics. **`moveTask(to:node:)` moves a Workflow node,
not Wave ownership**: do not wire it to Move to Wave. Bind ownership reassignment
to the shared planning operation from LOO-505; inspect installed support before
choosing the call. New conversation uses the existing explicit conversation entry;
Open in editor uses the recorded checkout and configured editor, with an honest
unavailable reason if no checkout exists. The PR numbers in the study are fictional. Waveless Tasks omit Wave actions/crumbs; consume LOO-505
rather than adding ownership rules here.

## 6. Focus and keyboard routing

Full map and source audit: [keyboard-shortcuts.md](keyboard-shortcuts.md).
Final adjacent binding is **Ctrl-Tab / Ctrl-Shift-Tab**, cycling stable working-set
order. This replaces the earlier Shift-Cmd-brackets proposal; history remains
Cmd-[ / Cmd-]. Cmd-1…9 addresses stable working-set slots even under a filter,
with hover/held-Cmd hints replacing the age in a reserved metadata slot. They must
never overlay the title or change row width. Clear held-key presentation on keyup
and window blur. Missing slots are a consumed no-op.

Route exact modifiers in the active repository window. Transient dialog > native
editing responder > eligible workspace action > terminal passthrough. Ctrl-Tab and
Ctrl-` are explicit workspace exceptions (terminals otherwise receive Ctrl-` as
NUL, which some users bind); check them against the shortcut map before shipping; other Ctrl/Option editing and printable keys
stay with the terminal. Respect IME composition and keyboard layouts. Do not
consume sidebar/list arrow keys while the terminal is focused.

Opening a Task focuses its retained input only after the target is mounted and
ready; failed/opening states do not send keystrokes to the previous Task. Never
synthesize Enter. Arrow focus in sidebar changes neither selection nor Session
acquisition. Return activates once. Escape returns to the prior responder. Palette
cancel/filter-only choice restores caller focus; accepted navigation focuses the
destination. Background updates never steal focus. Collapse transfers focus to
the center only if the hidden sidebar owned it.

Keep existing pane keys, move Reopen Pane from Cmd-Z to Shift-Cmd-T, and remove
Portfolio/Telemetry/Snapshot number collisions. Keep Files Cmd-P/Cmd-S local.
Check actual Ghostty Find and scaling ownership before adding new handlers. Use
native menu equivalents instead of a second local monitor wherever possible.

## 7. Menus and Settings cleanup

The implementation now groups appearance, autosave and the keyboard reference in
Settings, removes duplicate Open Repository and diagnostic number bindings, and
retains theme customization through the existing owner. The accepted map remains:

| Location | Final contents / change |
| --- | --- |
| App → Settings, Cmd-comma | One Settings entry. Appearance (System/Light/Dark, selected light/dark themes), Editing (existing Autosave Task Files), Keyboard Shortcuts reference. No new preference store. |
| Appearance | Live preview through existing theme workflow, predictable Cancel/Apply. Theme editing/import under a secondary Customize action. Show raw config paths only in advanced detail or actionable errors. |
| File | Open Repository Shift-Cmd-O; existing New Terminal, Close Pane, Reopen Closed Pane, Close Window. Remove duplicate Cmd-N repo command. Do not invent New Task to occupy it. |
| View / Go | Navigator, sidebar visibility, Back/Forward, adjacent Task, Task 1–9, existing pane zoom/focus and companion commands. One label/binding per action. |
| Window | Existing repo windows and standard macOS cycling; Portfolio and Telemetry remain reachable without number shortcuts. |
| Help | Keyboard Shortcuts; Snapshot for Review/diagnostic functions appropriately labeled. Keep macOS Help search. |

Remove duplicate appearance/menu entry points once Settings covers them. Do not
remove theme editing/import, diagnostics or autosave just because their current
placement is noisy. Do not change a user's saved themes or autosave setting as
part of implementation. Fixed bindings and their reference belong here; rebinding,
profiles and user-defined shortcuts stay with LOO-452. Prototype does not pretend
to implement native Settings.

## 8. Implementation status — reconciled October 17, 2026

Implemented: stable retained working order/cache and pause eligibility, direct Wave
rows, number/adjacent navigation, filters, shared Shell/Files tabs and toggles,
passive repository/Wave conversation entry, full Wave inventory, one titlebar,
neutral selection, native menus/Settings, and a shared Task action surface. Codex
native-turn excerpts use the existing activity row and optional Session DTO;
new turns clear them, and attachment/thread changes fence them. Other providers
without qualified evidence show the labeled summary.

Upstream `616d73b4dc` supplies standalone Tasks and membership moves; the combined
code uses `task move --wave/--no-wave` and optional Wave crumbs without a second
ownership model. Compression removed the unused compact/grid renderer. Realignment
adds missing Zoom Focused Pane and directional Focus Pane menu entries through
the existing pane actions; their keyboard handlers already existed.

Remaining acceptance is not a smaller delivery boundary:

- Open in editor remains unavailable: no configured editor operation exists in
  the current CLI/Desktop. No editor preference store was invented. LOO-505 is
  integrated through the shared Task membership command, including No Wave.
- `WaveTaskInventory` renders the complete current Task list and its read failures,
  but only the Wave name above it. The accepted objective/status context and
  overview actions still need integration from shared Wave detail. A complete
  inventory alone does not satisfy the full Wave experience.
- Sidebar Cmd-F opens the general navigator, without the promised list scope;
  Shift-F10 has no explicit sidebar handler. Resolve these against native responder
  behavior before claiming the complete keyboard map. The unconditional two-line
  Available preview remains a visual-review choice, not a new accepted rule.
- Sidebar arrow/Return/Escape and focus memory are implemented; prove native
  focus restoration, narrow titlebar fitting, keyboard precedence, two-window switching and
  mounted terminal/draft retention in demo. Current headless results are not
  evidence of those mounted behaviors.
- Gate owns affected suites and automated acceptance once. Preview producer
  tests and migration-frontier checks are part of implementation verification.

Unresolved product choice: supply an editor-launch contract or explicitly revise
the Open in editor acceptance with Jack Heart. Its disabled item is not delivery.
No other deferred acceptance above has been waived.

## 9. Acceptance and failure cases

Verification: `scripts/test_desktop.sh --filter 'ConsoleWorkspaceTests|WorkspacePanesTests' -Xswiftc -gnone` — pending menu-repair check; prior focused Swift passes (65 and 47 tests) and unchanged Rust build/preview/frontier/DTO/fmt/clippy evidence remain applicable, with pre-compression commands at `34955867e280f1fa523ce32337775cbb740c5302:scratch/navigation-implementation.md`; gate owns broader suites, demo owns mounted behavior.

Automated checks must prove behavior through existing production owners:

- Repository and Wave conversations retain independent drafts from Tasks. Repo
  crumb opens its conversation; chevron opens navigation. Empty repo is usable;
  no Shell/Files controls appear at repository/Wave scope.
- Paused work is absent from working rows/digit slots, remains in Wave/search and
  never resumes just by opening it. Paused state is distinct from stale observation.
- A Wave with both planned and started Tasks shows Started then Not started and all members;
  a Wave with no started Tasks still shows its plan. Opening planned work displays
  its brief, preserves working positions and causes no Session/checkout/run effect.
- Stable working positions during recency/attention updates, filtering, new Task
  arrivals and repository switches; scope changes never change center identity.
- Fresh latest agent output replaces the summary; quiet/stale output falls back
  with an explicit Summary label. No stale line revives after a new turn.
- Status/report transition: available → new turn → working → result; out-of-order
  prior-turn events; provider replacement; clear/reset; disconnected and recovered
  observation; no-provenance fallback; two Sessions with conflicting status.
- Cmd-/ and ••• expose the same Task-only actions, with no generic navigation or
  visibility commands. Context action targets the right Task without selecting it. Copy confirmation
  follows successful clipboard write. PR/branch absent stays absent.
- Shortcut exact modifiers, active window, missing slot, forward/reverse wrapping,
  modal precedence, focus restore and no duplicate terminal input. Preserve Cmd-Z
  editing and Files/Find behavior. Browser key events are not native proof.
- Shell/Files toggle into the Task arrangement, with Brief/Changes in Files tabs.
  Preserve mounted Session input, shell tabs, file tabs, caret, selection, drafts
  and scroll through toggles, tab changes, Task switching and pause/resume. Hiding
  a tool changes the arrangement without stopping its clients.
  The prompt is the final terminal line, with no rule/box/placeholder call to action.
- Sidebar width/scroll and draft retained through collapse, Wave → Task → Back,
  resize, query refresh/failure and theme change. Stale reads retain last list with
  an error rather than falsely emptying the working set.
- The working set at 1000×760 shows every fixture row without scrolling (five after
  pausing LOO-497); long titles/localization/large accessibility
  text stay usable; 12+ working Tasks and 6+ Waves scroll in the same list. Waves
  have direct rows and Task counts; the Wave page shows every member by default.
- Existing WorkCache restoration expectations must be reconciled with current
  behavior, not excluded to obtain a green run. Keep failed native proof visible.

Headless leaf-view and source-owned logic checks cover changed chrome. Mounted
terminal input and window cycling remain native-demo evidence; screenshots alone
cannot establish Session retention or provider correctness. Rust DTO verification
covers shared Session tests, fixture agreement and the released migration frontier.

Native demo: two repository windows; six then twelve Tasks; draft in A; switch
by click/number/Ctrl-Tab to B; filter/collapse, Wave, Back; toggle tools, tabs,
Brief/Changes and terminal Find; change theme and cycle windows. Return to the
same Session/pane/draft. Observe new turn → working → Available and observation
loss without stopping work. Record failures and limits; restart retention is LOO-416.

## 10. Boundaries and review

LOO-497 owns dashboards/public APIs; LOO-416 attachment lifetime; LOO-505
ownership; LOO-452 rebinding. No parallel retained client, ownership rule,
acquisition path or preference store enters this cut. Review preserves display
state separately from execution authority and clears output at its source boundary.

Publication belongs to pursue's delivery step; merge and Task completion remain separate.

## Delete — do not maintain

Removed: filtered Wave detail/plan rows and model history filters; attention-ordered
sidebar hierarchy; duplicate repository/Session chrome; `WorkSessionToolbar`,
`WorkGlyphButton`, `WaveObjectiveView`, `WaveWorkflowView` and its unused
`setWorkflow`/error plumbing. Workflow inspection/editing tests now exercise
`WorkflowCatalogInspector`/`WorkflowEditor`. Portfolio readiness/history remain.

Removed: shell-per-pane `split` APIs/allocation tests, pane-strip trio proof and
Control-Cmd-S/Cmd-Z bindings/assertions. Shell tabs, Files drafts, pane focus and
Close retain coverage on surviving owners; mounted interaction remains demo-owned.

Removed during compression: unused `WorkspacePanesView.Presentation.taskTile`,
`TaskPaneProjection.swift`, projected-surface branches and their exclusive tests.
Only tests called the compact renderer; the accepted Console renders full layouts.
`WorkspacePanesTests` retains full-layout focus, zoom and document retention coverage.
The sidebar now captures one Session reading for both status and preview, and
both status and fallback selection use the same primary-first ordering. Removed
unused pane setup values and the renderer's forwarding body property.
No remaining predecessor deletion targets. Sidebar scroll uses one per-repository
value; WorkLayout removes exact Work directly; Codex has one qualified activity
write path. Shell rendering binds one tab's identity/location/command. Native
menus own Back/Forward without hidden duplicate buttons.

All pre-implementation tracked/untracked source and complete notes are preserved
at `refs/loopflow/evidence/loo-489-before-console-implement`, tree
`4c6c6cc9cfaceb3387af16139b5dd5944f9768ff`. The working index was preserved.

Source before compression: `refs/loopflow/evidence/loo-489-before-compress`
(commit `34955867e280f1fa523ce32337775cbb740c5302`); index preserved.
