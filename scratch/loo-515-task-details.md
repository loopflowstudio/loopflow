# LOO-515 · Task details beside the conversation

Research: `scratch/task-details-research.md`.
The Changes build direction is in "Build direction — November 14" below.

Status: Changes/Brief built November 14 (see Implementation status); October
sections below are the plan for what remains. Accepted
inputs: LOO-497 (one Files tool per Task, document/Brief/Changes tabs, hidden
tools keep drafts), LOO-489 Console (one titlebar, Lato, neutral selection),
LOO-327 (document per Task/path, drafts, recovery), the LOO-515 brief.

## Build direction — November 14

Jack, asked whether the B+A hybrid is the direction and how moved code appears (a paired inline, b collapsed with expand, c both): "yeah lets build it with (b)". Latest page: `scratch/explore.html` (B).

What pursue builds for the Changes page:

- **B · Review stream** is the reading surface: the PR-equivalent comparison (what `lf pr open` would show) as one continuous document, with a persistent change navigator.
- **From A:** a Side by side / Unified toggle for the stream, and temporary Focus file / All files. Next/previous change keeps crossing files while focused.
- **Moved code (b):** a within-file moved block collapses to one row, "moved from L<n>, <k> lines changed", that expands to a paired before/after showing word-level edits inside the block.
- **The five priorities** apply throughout: next/previous change across files, word-level highlighting, selecting code to put its file, side, range and revision into an unsent conversation draft, live updates that keep the reading anchor (no pause control), and moved-code detection.
- C's story grouping is not built.

Sections below from October (PR card, Files, Brief) still stand as the plan unless they conflict with this section; the October line "No GitHub patch renderer remains" stands: the stream renders the local PR-equivalent diff.

## How the direction was chosen — November 14

- Jack: "i tink of htose, 1, 2, 5, 6 [live, not pause], 7 are the important ones for me" — the five priorities above, from a seven-item list. Reviewed marks, changed-since-reviewed and a pause control are secondary; basic place retention is expected.
- Jack: "$explore build me 3 prototypes" → `scratch/explore.html` (A Precision, B Review stream, C Change stories).
- Jack: "probably mostly B review steream", then "maybe a little bit of A", then "yeah lets build it with (b)".
- Kaleidoscope is the familiar reading reference. PR-equivalent comparison, reusable focus filters and prompt GitHub publication/opening were selected earlier; filter scope and publication remain separate (see `scratch/questions.md`).

## Implementation status — November 14

Built (one PR, Swift + one Rust routing line):

- `Page` is `.brief | .changes | .files`; header names the Task on every page with a segmented control (menu when narrow). `details` DesktopAction, ⌥⌘1/2/3, View menu; `PaneNeed` routes it to the browser.
- **Changes** (`TaskChangesView`, `ReviewDiff`): delivery card (`TaskDelivery`: none/publishing/open/stale/ahead/unavailable/merged/abandoned, "Task still open"); changed-file list from `lf task changes`; whole-comparison stream from `lf task diff` (path nil), live via the existing file observation; `]`/`[` and toolbar arrows cross files; Focus file; Unified/Side by side (≥ 640 pt); word-level emphasis; within-file moves (pure removal ↔ pure addition, ≥ 3 lines, ≥ 60 % trimmed-line overlap) collapsed with expand; line selection → Copy / Ask in conversation (types one-line reference into the session terminal without Enter). `.changes(comparison:)` browser tabs open this page.
- **Brief** (`TaskBriefView`): state, next move, condition, PR chip → Changes, due; related Task chips (identifier regex, open `loopflow://<repo>/task/<id>`); outline from headings/`Acceptance:`; "Brief updated" marker; scroll kept per Task in-app.
- **Files**: document row and location line (Machine · checkout/dir) only on this page; ⌘P works from every page. Parent SHA and saved versions moved to Changes.
- Deleted: `TaskPullRequestView`, `GitHubPullRequestFile`, `PullRequestStore.loadFiles` and its state, `publishedDiffDisclosesMissing…` test, stale README tree paragraph.

- **Files tree** (`CheckoutTree`, `TaskFilesStore.tree`): checkout from `lf task files` through a new served read `task.files` (Rust `Read::TaskFiles`, Swift `ServedRead.taskFiles`, `RegistryQuery.taskFiles`); a remote Task's reads carry `--machine` and no cwd. Folders read when opened, pages of 500 with Load more; M/R/U badges and `•` on holding folders; ↑/↓/→/←/Return; search over scratch, loaded entries and changed paths; a failed read shows under its folder with Retry; a write re-reads the loaded folders holding it (as many pages as were loaded). Tree, selection and scroll live on the store, retained per Task workspace. The navigator's `Changed files` section and `showsChanges`/`changedPaths` are deleted.

Remaining:

- Moves inside mixed change runs; shortcut for Ask (none yet).
- Demo on a real Task: long brief, no-PR Task, offline `gh`, large patch performance of the LazyVStack stream, a `node_modules`-scale folder.

Checks: `scripts/test_desktop.sh --filter 'TaskFilesTests|ReviewDiffTests|TaskBriefTests|PullRequestTests|DesktopPaneControlTests|DTOFixtureTests|WorkDestinationTests|DesktopPerformanceFixtureTests|RegistryQueryTests'` → 222 run; after one test fix `TaskFilesTests` 33 passed; `ServerProtocolTests` 20 passed. `cargo nextest -p loopflow --lib 'test(server::schema::tests::) | test(lf::commands::read::)'` → 11 passed (schema fixture regenerated). Gate: clippy, `documented_commands::` (README edits), golden not run.

## Demo

In a Task workspace with the conversation on the left and Files on the right:

1. Press ⌥⌘1. **Brief** shows `LOO-515 · Make Task PR…` in the header, a state
   line (`Moving · kickoff running · PR none`), an outline of the brief's
   headings/Acceptance, and related Tasks `LOO-489 LOO-497 LOO-416 LOO-402` as
   chips that open those Tasks in-app. Scroll halfway down.
2. ⌥⌘2. **Changes** shows the delivery card (`No pull request yet` here;
   `Open #… · checks · read 20 s ago · Open on GitHub ↗` once published)
   above the review stream. `]` steps change by change across files; toggle
   Side by side; expand a collapsed move; select lines → Ask in conversation
   puts `path:Lx-y (side, revision)` in the terminal unsent.
3. ⌥⌘3, expand `rust/loopflow/src/ops/task/` in the tree, open `directory.rs`.
4. Click the conversation, ask "why is PAGE_SIZE 500?", read the answer.
5. Switch to another Task with Cmd-2 and back with Cmd-1: Files, the tree's
   expanded directories, the Diff's scroll and the Brief's scroll are where you
   left them; the conversation never left the screen.
6. Edge cases in the same build: a no-PR Task (`No pull request yet · changes
   below are local`), `gh` offline (`GitHub unavailable · last read 14 min ago`,
   checks greyed), a merged PR on an open Task (`Merged · Task still open`), a
   20k-word brief, `node_modules`-scale directory paging, a deleted file.

## Chosen approach

### Pages, Brief, Changes

Built as described in Implementation status. Kept from October: no new panes;
conversation left, Files right at 61.8/38.2; Brief derives from the
`RoadmapTask` the pane already has; GitHub reads refresh on appear, on URL
change and every 60 s only while Changes is visible.

### Files

- **Navigator** = filter field, pinned `scratch` group (agent pages, unchanged
  follow behavior), then the checkout **tree** from `lf task files <issue>
  <dir> --json` (restore `RegistryQuery.taskFiles` + `TaskDirectory` DTO).
  Directories load lazily on expand, 500 entries per page; `Load more` passes
  `next_cursor`.
  Changed paths get a change badge from the existing `changes` snapshot.
  Keyboard: ↑/↓ move, →/← expand/collapse, Return opens, ⌘P toggles, Esc
  dismisses dropdown. Filter matches loaded names plus scratch/changed paths
  (no recursive server search in this Task).
- **Docked vs dropdown**: unchanged mechanism, docked threshold raised to
  480 pt so the document keeps ≥ 300 pt.
- **Retention (in-app)**: expanded directories, loaded pages, tree selection,
  per-document reading scroll (Markdown/HTML), per-path Diff scroll, Brief and
  Changes scroll all live on `TaskFilesStore`, which `SessionWorkspace`
  already retains per Task across switches. Drafts/tabs/selection continue to
  persist across restart through the existing `WorkBrowserTab.files` intent.
- **Failed reads**: a directory read failure shows inline under that directory
  with Retry; a missing file keeps the existing `File no longer exists`.

## Risk findings

- `lf task files` works today (`lf task files LOO-515 . --json` returns sorted
  entries, 500 per page with `next_cursor`) and is in the remote-routable set,
  so remote Tasks browse through their Machine. Desktop reads are served, so it
  gained a `task.files` served read (November 14); nothing else in Rust.
- #1719 (`db5736d11`) deleted only the Swift wrapper; the `TaskDirectory` DTO
  and its fixture section survived, so no DTO change is needed.
- `RoadmapTask` has no dependency relations; detection over description text
  is the only source without a new Linear read. Accepted as the scope here.
- Page and scroll are not in the persisted `WorkBrowserTab.files` intent;
  adding them is a Rust/Swift DTO + migration change owned by LOO-497/416 —
  excluded (see Exclusions).
- GitHub reads cost one `gh` process per refresh; 60 s cadence only while the
  Changes page is visible keeps it bounded to one Task at a time.

## Alternatives considered

Separate Brief/PR panes (violates LOO-497's one Files tool), PR as its own page
(duplicated the local diff), Brief as a popover (loses scroll, keyboard), and a
`git ls-files` tree (omits untracked, no remote routing) were rejected.

## Delete — do not maintain

Done: `TaskPullRequestView`, `GitHubPullRequestFile`, `PullRequestStore` file
state, the published-diff test, the mixed page/tab strip, the navigator footer,
the navigator's `Changed files` section (`showsChanges`, `changedPaths`).

## Forbidden outcomes

- A second Diff renderer, or a second file list that disagrees with
  `lf task diff --files`.
- Showing a GitHub reading without its read time, or green checks on a failed
  or stale reading.
- Any local path fallback for a remote/unavailable checkout.
- Treating `merged` or all-green as Task completion in copy or state.
- New panes, or hiding the conversation to show details.

## Exclusions

- Persisting page/scroll/tree expansion across app restart (LOO-497/416 own the
  persisted intent shape).
- Waiting/attention indicators in navigation rows (LOO-402).
- Recursive server-side file search, editing outside text files, an IDE.
- Linear relation fields for dependencies; PR history beyond the current
  `RoadmapTask.pr` (no multi-PR reader exists in the row).

## Internal slices (one PR)

1–5 done (delete, header/Brief, Changes, Files tree, README).

## Done when

- A Task workspace shows Task identity in Files on every page, with Brief,
  Changes and Files reachable by click and ⌥⌘1/2/3, and the conversation pane
  unchanged in size and focusable throughout.
- Brief shows state, PR chip, related Tasks and an outline for structured
  briefs; empty and changed briefs read correctly.
- Changes distinguishes none / publishing / unavailable / stale / open /
  merged / abandoned with distinct text, and opens any changed file's local
  Diff.
- Files browses the actual checkout tree (local or remote Machine), paginates
  large directories, shows location, and survives page/Task switches with
  tree, selection, scroll and drafts intact.
- No GitHub patch renderer remains.

### Acceptance checks (gate, headless)

- `uv run python scripts/test.py --reuse-passing` → swift suite passes,
  including new `LoopflowTests` cases:
  - `TaskFilesTests`: tree loads `TaskDirectory` pages lazily, `Load more`
    appends, failed directory read is local to that node; page/tree/scroll
    state survives `SessionWorkspace` Task switch; remote store never issues a
    local path read.
  - Exist: `PullRequestTests.deliveryDistinguishesEveryStateWithDistinctText`
    (all delivery states, `Task still open`), `TaskBriefTests` (related Tasks,
    outline), `ReviewDiffTests` (stream, cross-file next/previous, word
    emphasis, moves, selection reference).
- Demo/review (Jack): the six-step loop above on a real Task, including a
  no-PR Task and offline `gh`.

