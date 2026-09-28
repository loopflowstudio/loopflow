# Task file browser

Interaction design approved by Jack Heart · 2026-09-28.

## Approved direction

Jack confirmed: “ok. lets start aligned. i approve the design.”

- Keep the file browser beside the retained LLM/Ghostty pane.
- One navigator with `scratch/` and `diff` sections.
- Put the Task's PR link above just the navigator.
- Align the LLM, navigator and file-view headers using one shared height.
- Keep Diff read-only; allow small edits and Save in File view.
- Retain a shared draft per path across selections and views.
- Keep the Parent/HEAD comparison control and minimal chrome.

The approved visual reference is [the web mock](file-browser/index.html).
Native integration and exact Git-base resolution still need implementation
detail; the recorded approval settles the interaction design.

## Intent

Jack Heart requested “One nav window with just those two options as headers basically”:
`scratch/` and `diff`, with possible parent-branch/HEAD comparison. The priorities
are “minmalism, peformance, elegance” and “this is not a full file editor.”
Jack then directed: “start by prototyping in a web mock”.
After reviewing the mock, Jack proposed: “Maybe diff is not editable but file
view is”. The revised mock implements this interaction: Diff is read-only;
File is editable for both scratch documents and source files.
Jack also proposed placing the PR link “above this but in same panel”. The mock
places the PR link above just the navigator, following Jack’s clarification;
the document header stays separate. The PR row, LLM pane header and file-view header share a 36px height so
their text centers and bottom borders align, as Jack requested.

## What to build

Give each Task a small file browser beside its retained Ghostty session, for
reading scratch documents, making small edits, and inspecting changes.

## Placement

Product. Jack requested a product Task and handoff once the design is ready for
implementation. `wave/product/GOAL.md` owns the desktop workspace and shared
user contract. The source checkout contains only the design and web prototype, not a competing
native implementation. Product Task: [LOO-327 · Read and edit task files beside the
conversation](https://linear.app/loopflow/issue/LOO-327/read-and-edit-task-files-beside-the-conversation).

After Jack repaired lf, Product chapter lookup succeeded. LOO-327 was filed and
prepared in `/Users/jack/src/loopflow.task-files`. That checkout owns the working
design after transfer; this source copy is retained only as prototype provenance.

## This slice — interactive web mock

`file-browser/index.html` explores one navigator with collapsible `scratch/`
and `diff` sections and one selected document. A working Parent/HEAD control
changes sample change sets. File edits persist in browser storage via Save;
terminal and repository contents are explicitly simulated. No backend or native
implementation is part of this slice. The mock uses the existing cream/burgundy
palette and warm charcoal terminal colors.

The PR link sits within the navigation column, above scratch/ and diff, and
stays visible across file selections. Its native
action opens the Task's recorded PR URL; do not infer a URL from a branch name.
The prototype uses an explicitly labeled sample PR preview. A Task without a
PR should omit this strip, leaving the file browser available.

Each path owns one draft shared across both navigation sources and view modes.
Switching to Diff shows that draft against the selected sample base, with an
unsaved indicator when appropriate. Switching views or files retains edits.
Save writes browser storage only. Native saving must preserve external edits
and never write a truncated preview.

Proposed comparison semantics: Parent shows the current working tree relative
to the merge base with the Task's recorded parent branch; HEAD shows all current
uncommitted changes, including staged and untracked files. These are draft
choices, not approved semantics. Existing code uses an immutable recorded base
commit, which is not necessarily the current parent merge base.

## Current system

- `swift/LoopflowMac/Views/TaskWorkspaceView.swift`: existing sheet has Changes
  and Terminal sections, flat changed-file list, and plain-text Diff/File reads.
- `swift/Loopflow/Models/TaskWorkspace.swift`: changed-file, patch, content DTOs.
- `swift/Loopflow/Services/RegistryQuery.swift`: typed task file/diff/changes reads.
- `rust/loopflow/src/ops/task.rs`: owns Task worktree/base resolution and Git
  reads; currently caps returned patch/content at 1 MB.
- `swift/Loopflow/Models/MultiplexerLayout.swift` and
  `swift/LoopflowMac/Views/SessionsView.swift`: retained terminal and Monitor panes.
  Native integration should reuse these ownership paths.

## Demo and proof

Open the HTML, select files under both sections, switch Parent/HEAD, edit and
save a scratch document, edit a source file through File view and inspect its
read-only diff, resize the terminal, and hide/reopen Files. Verify
selection, drafts and terminal context survive those interactions. Capture via
`lf screenshot`. This demonstrates interaction only; it cannot prove native
text performance, live Git behavior, or Ghostty focus.

## Forbidden outcomes

Full repository explorer, editor tabs, language server, extension system,
Git staging/commit controls, a second terminal lifetime owner, or a mock
presented as live repository evidence.

## Next internal slices

1. Completed: web mock and approved interaction/layout.
2. **This slice:** native feasibility spike in the prepared Product Task. Keep the
   full end-to-end browser as one core; no independent child Tasks are needed.
3. In that Task, run a focused native feasibility spike before full implementation.
   Continue the approved interaction rather than restart visual discovery.
4. Implement and review the complete browser in one PR, with internal cuts for
   data access, document lifecycle, native integration and the configured demo.

## Focused research before implementation

The first agent performed source/documentation research, not a native benchmark
or hands-on comparison. Start with NSTextView/TextKit 2. Do not repeat the broad
library survey or interpret the web mock as proof of native performance.

Build a disposable native proof beside the existing Ghostty surface. Exercise
continuous selection/copy across diff hunks, native undo/find/IME input, focus
switching while terminal output arrives, 1 MB text/patches and long lines.
Measure first usable display, scrolling and editing responsiveness. Exercise
external agent edits while a local draft is dirty; retain both versions and
demonstrate that saving does not silently replace an observed external change.
Record the machine, sample sizes and failures. Evaluate CodeEditTextView only
if a specific NSTextView limitation appears; CodeMirror is a fallback if native
diff interaction proves disproportionately expensive. Avoid a custom text engine.

Then settle the native data/API and save-conflict contract, including binary,
non-UTF-8, truncated, deleted and renamed files. Keep the existing Task worktree,
PR authority and terminal lifetime owners. Parent and HEAD must use matching
file-list and patch semantics. The existing TaskPr base is already maintained
through PR-range integration (`task_pr_range_tests.rs`); prefer that authority
over a Swift-side merge-base calculation. Record the final meaning of Parent
before implementation, since the mock originally proposed current merge-base.

## Task brief

**Read and edit task files beside the conversation**

When reviewing a Task, Jack needs its working notes and changed files beside the
agent conversation, with quick edits available without opening a full editor.
Add a lightweight native file browser to the Task workspace while preserving
the running Ghostty session and its input.

- One navigator has scratch/ and diff sections, with the Task's PR link above
  just the navigator. LLM, navigator and file headers align.
- Diff is read-only; File view supports small edits and explicit Save. Drafts
  survive switching files, view modes and hiding the browser.
- Parent/HEAD compares the same Task work against clearly identified bases;
  scratch and diff navigation share one document per path.
- Agent edits do not silently overwrite a local draft or get silently lost on
  Save. Binary, unsupported, deleted and truncated files have clear states.
- Keep it minimal and responsive. No full project explorer, language server,
  editor tabs, or Git staging controls.

Interaction design approved by Jack on 2026-09-28. Implementation choice remains
provisional pending a focused native feasibility spike, not another UX redesign.
Design: `scratch/file-browser.md`; mock and research: `scratch/file-browser/`.
These artifacts must be copied into the prepared Task worktree and verified
before any worker starts. The HTML also uses fonts from `swift/Loopflow/Fonts/`.
Source provenance: `jack-heart/file-browser` in the supplied design checkout.

After the spike settles the implementation contract, `pursue` is the proposed
continuation: implement → compress → review-slice → loop-decide → human demo.
Its authored definition was inspected. The handoff selects `pursue`, with the focused native spike and contract resolution
required at the start of its first implement step before full implementation. Syntax
highlighting and richer side-by-side diff interaction are deferred possibilities,
not filed follow-ups or prerequisites for this core.

## Evidence ledger

- 2026-09-28: scratch was empty and worktree clean on entry.
- 2026-09-28: inspected existing task workspace, DTOs, Git reads and app palette.
- 2026-09-28: native research recommends SwiftUI navigation + NSTextView;
  alternatives and sources are in [native research](file-browser/native-research.md).
- 2026-09-28: captured and visually inspected `file-browser/preview.png` at
  1440×900 via `lf screenshot`. Browser interaction checks passed for Parent/HEAD
  membership, edits, draft retention across selection, Save/reload, hide/reopen,
  section collapse, keyboard navigation, keyboard resize and 1000px layout.
  No browser script errors. Native behavior remains untested.
- Review: editable source documents and comparison bases are self-contained
  samples. Dependency-free HTML keeps this study isolated from the production app.
- 2026-09-28: revised editing checks pass: source edits appear in read-only
  Parent and HEAD diffs; save/reload retains source edits; scratch and diff
  navigation share one path buffer. Original interaction checks still pass.
- 2026-09-28: Product Task creation was attempted through `lf task create` and
  failed before returning an issue: incompatible installed development store,
  then “failed to read wave registry: no wave registry on this machine”. No
  Task, destination worktree or worker launch was confirmed. The Task brief
  above remains ready for filing when the configured service is available.
