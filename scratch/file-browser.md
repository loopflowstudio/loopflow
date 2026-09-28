# Task file browser

Draft · 2026-09-28 · interactive prototype first.

## Intent

Jack Heart requested “One nav window with just those two options as headers basically”:
`scratch/` and `diff`, with possible parent-branch/HEAD comparison. The priorities
are “minmalism, peformance, elegance” and “this is not a full file editor.”
Jack then directed: “start by prototyping in a web mock”.

## What to build

Give each Task a small file browser beside its retained Ghostty session, for
reading scratch documents, making small edits, and inspecting changes.

## Placement

Unresolved: no Wave was supplied. Work remains in this supplied worktree.

## This slice — interactive web mock

`file-browser/index.html` explores one navigator with collapsible `scratch/`
and `diff` sections and one selected document. A working Parent/HEAD control
changes sample change sets. Scratch edits persist in browser storage via Save;
terminal and repository contents are explicitly simulated. No backend or native
implementation is part of this slice. The mock uses the existing cream/burgundy
palette and warm charcoal terminal colors.

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
save a scratch document, resize the terminal, and hide/reopen Files. Verify
selection, drafts and terminal context survive those interactions. Capture via
`lf screenshot`. This demonstrates interaction only; it cannot prove native
text performance, live Git behavior, or Ghostty focus.

## Forbidden outcomes

Full repository explorer, editor tabs, language server, extension system,
Git staging/commit controls, a second terminal lifetime owner, or a mock
presented as live repository evidence.

## Next internal slices

1. **This slice:** web mock and review of interaction/layout.
2. Resolve native text surface and comparison semantics from research/review.
3. Detail native data structures, API extensions, conflict handling and proof
   once the proposed experience has been reviewed. No implementation launch yet.

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
- Review: source content in this mock is explicitly an excerpt, not a complete
  source file; full-file/native editing remains a later decision. Dependency-free
  HTML keeps this interaction study isolated from the production app.
