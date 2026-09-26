# Cycle 3 — compress: frame, Wave plan, basic Task overview

2026-09-25. Two ownership reductions in Swift; Rust started-evidence path
reviewed and left unchanged. Nothing committed, published, or run against a live store.

## Model before → after

- **Description Markdown.** Before: `MarkdownBlocks` held a second, partial
  Markdown grammar. It hand-parsed heading, bullet, checkbox, ordered, quote and
  fence line prefixes, then used Foundation only for inline runs. After:
  Foundation's `AttributedString(markdown:)` is the one parser. Each run's
  `presentationIntent` gives its block: heading, paragraph, quote, code, rule,
  or table row. List markers and depth come from `listItem`/`orderedList`
  components. Soft breaks render as newlines, which keeps the authored line
  structure the implement pass chose. Inline emphasis, code and links stay
  attributes. If the parser throws, the source is shown verbatim. Tables and
  thematic breaks, which the hand parser dropped into paragraphs, now render as
  rows and a divider. No dependency or web renderer was added.
- **Task selection.** Before: `WorkspaceNavigation.taskPanes` was written from
  four places (`rememberTaskPane` on every multiplexer change, Session open and
  Monitor open). Nothing in production read it after cycle 3. Task selection now
  comes from shared Session membership: one open Session drills in, zero or
  several open the overview. After: the field, `rememberTaskPane`, and its
  notification branch are deleted. The multiplexer notification now only bumps
  `layoutRevision`.

## Deletions

- The hand block grammar, including `inline(_:)` and the `☐/☑` checkbox mapping.
  Foundation doesn't parse GFM task lists, so `- [ ] x` now shows the literal
  `[ ] x` after its bullet. That keeps the source text; it is a visible change.
- `WorkspaceNavigation.taskPanes` and `SessionsView.rememberTaskPane()`.
- The stale README sentence ("Selecting a Task restores its last pane, initially
  Monitor"). It now describes the one-Session/overview rule and Monitor as an
  explicit breadcrumb control. `.lf/directions/desktop.md` now records that Task
  return derives from shared membership, never a remembered pane ID.

## Retained boundaries (reviewed)

- `Store::task_started` stays separate from chapter `TaskStartEvidence.begun`.
  A prepared human `session_run_id` counts as begun for chapter carryover, but it
  must not admit a Task to the sidebar. The sidebar check is one indexed
  `EXISTS` query per Task: Task events or `worker_generation > 0`. The roadmap
  adds published PRs. There is no Run-history scan and no writer. `false` means
  no recorded evidence, not "never ran". Only actual Run capture writes Started
  (`started-read-boundary.md`); Work resolution stays read-only.
- New session still goes through existing scoped launch plus `lf task prepare`.
  It does not run the managed Flow.
- Wave-level and unmatched Sessions remain leaves, and the flat presentation
  lists all of them. Exact Task identity, not cwd or title, drives drill-down.
- DTO fixtures are unchanged here. The `started` field was already mirrored in
  Rust, Swift and `roadmap_snapshot.json` by the implement pass.

## Proof

`swift test --package-path swift -Xswiftc -gnone --jobs 4 --no-parallel --filter
'TaskDirectiveEditorTests|TaskMonitorProofTests|WorkspaceNavigationTests|WorkspaceNavigationProofTests/namedSessionDrillDownRetainsTerminal'`
**28 tests in 4 suites pass**, first run, exit 0. Log: `/tmp/loo291-c3-compress-swift.log`.

- New `descriptionMarkdownBlocks` covers:
  - a heading, and a multiline paragraph whose soft break is kept;
  - a link attribute and inline code;
  - nested bullets (depth 1/2) and an ordered list (`1.`/`2.`);
  - a quote, a fenced code block with its trailing newline trimmed, and a header
    plus body table row with separate cells.
- The editor test now asserts the rendered text literally rather than through
  the deleted inline helper.
- `paneFocusPreservesTaskChoice` is renamed. It no longer asserts on the deleted
  map. It still proves that focusing another Task's pane cannot redirect a
  return to the selected Task: the Task returns to its own Session and pane.
- Real Ghostty/PTY proofs in this selection also pass: Monitor chapter transfer
  and the named Session drill-down.

No Rust edits, so fmt and clippy were not rerun. `git diff --check` passes. No
Xcode fallback build: `MarkdownBlocks.swift` is new, so `project.yml`'s source
glob still needs checking.

SHA-256:

```text
MarkdownBlocks.swift          cc86634190bd5b3a597bb2b0a5bc26517d0f1759b4fd34feb5ca32f041e40b65
SessionsView.swift            e4e89477e84464819c11d2a0e9c09e959ec2058222bc6e5e1c3d2d68a1f4e406
WorkspaceProjection.swift     500d0961161e5865b544be67a3bc0b22e89414368a151b66db4bad3ccfa2d1b4
TaskDirectiveEditorTests.swift 7a737c5d555f64ab13302761b97a27b2605262f583f1864be6f78c6aec67c2cb
TaskMonitorProofTests.swift   ebcc715ab1cc776f4b439dcaff9f8eb167dec737a60630dab7d74955e366d898
```

## Complete-design gaps (unchanged)

- The real Flow catalog, the pinned occurrence diagram with both return edges,
  and the final Advance → queue → land tail.
- The running, paused and blocked Task situations, with Stop & restart and pause
  controls.
- The actual comment count and thread.
- Naming the unavailable Session read on the overview.
- The configured demo, both performance measurements, and the external trials and
  authorized edit. The Wave page still has the chapter-history link and metric
  sections from implement; they were not re-reviewed visually here.
