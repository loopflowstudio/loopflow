# LOO-515 · Task details beside the conversation

Research: `scratch/task-details-research.md`.

Status: Changes/Brief built November 14 (see Implementation status); October
sections below are the plan for what remains. Accepted
inputs: LOO-497 (one Files tool per Task, document/Brief/Changes tabs, hidden
tools keep drafts), LOO-489 Console (one titlebar, Lato, neutral selection),
LOO-327 (document per Task/path, drafts, recovery), the LOO-515 brief.

## Changes pane redesign — November 18

Jack, on the before/after page (`scratch/explore.html`, as built beside redesigned): "yeah looks good". The brief for it, in his words: "focus on clarity, polish, and delight, not trying to change the fundamental behavior that it enables or functionality". The page's four questions were not answered; the choices below marked *assumed* are the page as shown.

Next for implement — restyle `TaskChangesView` and `ReviewStream`; behaviour stays:

1. Pane title: `<n> changes in <m> files`, then `against <base> at <sha> · includes uncommitted`. The comparison line leaves the footer.
2. The inline changed-file list goes; the current file's name in the bar opens a menu of files with +/− and a proportion bar; choosing one jumps. *Assumed: menu, not inline or a side column.*
3. Delivery card collapses to a pill (status dot, `#<n> · <state>`) in the title row; click shows the same card, every reading and its age kept.
4. `ReviewStepper`: one bordered control, position in the middle, `[` `]` shown beside it.
5. File headers pin while their file is on screen (`pinnedViews`); bar always names the current file; name bold, directory quiet; `Open` replaces the `doc.text` icon.
6. Hunk headers: no raw `@@`; a dashed rule with `<k> unchanged lines · <function>`.
7. Unified has one gutter: new number, old on a removed line.
8. Lighter row tint, darker changed word, no underline; `+`/`−` markers stay.
9. Moved rows get their own tint and `Moved here from L<n> · <k> lines, <unchanged | j lines changed>`; whole row expands.
10. Selection: line numbers highlight on hover and drag to select (⇧-click still extends); actions float over the code, Ask primary with ⌘↩. *Assumed: drag and the key are in.*
11. Focus: tinted strip `Only <file> · <n> other files hidden` with a close button; the Focus button stays in place and shows as on.
12. Landing on a change flashes it once; an edit arriving outside the view shows `<n> new change(s) below` and never scrolls; the stream ends with `End of changes · n of n · Back to the first change`. *Assumed: the chip is in.*

Built November 18 (`TaskChangesView`, `ReviewStreamView`), with these departures from the page:

- 5: file headers do not pin. A pinned header covers the row `scrollPosition` puts at the top, which is where next/previous lands; the bar names the current file instead. Open: pin with an offset landing.
- 2: the file menu lists paths (and `•` for a draft) without +/− or bars; counting lines for every file on each render is too slow on a large comparison. File headers keep the counts.
- 10: hover and click/⇧-click, no drag. ⌘↩ asks while the pane has focus.
- 12: no "new changes below" chip. Change ids are line-anchored, so an edit above shifts every id after it and would read as new.

Side by side stays as built (one gutter per side), offered only at ≥ 640 pt and visibly disabled with its reason below that. Files' Diff shares `ReviewStream`, so 5–10 reach it through the reader; its header and bar are not redesigned here. *Assumed.*

Dropped on November 18: tracking the diff by agent turn. Jack: "in our system the task session is not even the one actually making the changes. The diff is really about the underlying worker not the highest-level session."

## Build direction — November 14

Jack, asked whether the B+A hybrid is the direction and how moved code appears (a paired inline, b collapsed with expand, c both): "yeah lets build it with (b)". Latest page: `scratch/explore.html` (B).

What pursue builds for the Changes page:

- **B · Review stream** is the reading surface: the PR-equivalent comparison (what `lf pr open` would show) as one continuous document, with a persistent change navigator.
- **From A:** a Side by side / Unified toggle for the stream, and temporary Focus file / All files. Next/previous change keeps crossing files while focused.
- **Moved code (b):** a within-file moved block collapses to one row, "moved from L<n>, <k> lines changed", that expands to a paired before/after showing word-level edits inside the block.
- **The five priorities** apply throughout: next/previous change across files, word-level highlighting, selecting code to put its file, side, range and revision into an unsent conversation draft, live updates that keep the reading anchor (no pause control), and moved-code detection.
- C's story grouping is not built.

Sections below from October (PR card, Files, Brief) still stand as the plan unless they conflict with this section; the October line "No GitHub patch renderer remains" stands: the stream renders the local PR-equivalent diff.

## Concept review — November 14

Page: `scratch/concept-review.html`. Both findings are built:

- **One diff reader.** `ReviewStream` (`Views/ReviewStreamView.swift`) renders
  Changes' whole comparison and Files' Diff mode (`TaskFileDiffView`, one file
  from `lf task diff <path>`, draft included with the 180 ms debounce). Same
  rows, layout toggle, word emphasis, moves, selection → Copy/Ask. The text
  patch renderer is deleted.
- **Comparison named as the PR would.** Footer, Files' Diff header and the Ask
  reference read `Compared with <base branch> at <sha> · includes uncommitted
  changes`; the branch is GitHub's `baseRefName` once a PR is read, else
  "the Task's base" (see `scratch/questions.md`).
- **Second pass, built:** both readers step and keep their place.
  `ReviewReader` is `.changes | .file(path)`; each has a `ReviewPlace`
  (anchor, current stop) on `TaskFilesStore`. `ReviewStepper` is the one
  arrows/position/`]`/`[` control. Past a file's last (first) change, Files'
  Diff opens the next (previous) changed file, `lf task changes` order, on its
  first (last) change.
- **Third pass (after the stepper merge):** nothing to change. Changes and
  Files' Diff share renderer, place store and stepper; the one difference left
  is deliberate: Changes reads the PR-equivalent comparison (disk, no drafts,
  `•` marks a dirty file), Files' Diff reads the draft as typed. Changes'
  Focus file and Files' Diff on a changed file now look alike (one file, `]`
  crossing files); both were accepted separately, so the demo should show
  whether Focus file still earns its button. Compress: the `Unified or side by
  side` doc comment in `ReviewStreamView.swift` slid above `ReviewStepper`;
  it belongs on `ReviewLayoutPicker`.
- **Noted, not this Task:** a `.changes(comparison:)` browser tab and the Files
  tool's Changes page are two representations of one view; merging them
  changes the persisted `WorkBrowserTab` shape, which LOO-497/416 own.

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

Stepping compress: `scripts/test_desktop.sh --filter 'TaskFilesTests|ReviewDiffTests'` → 41 passed.
Checks: `scripts/test_desktop.sh --filter 'TaskFilesTests|ReviewDiffTests|TaskBriefTests|PullRequestTests|DesktopPaneControlTests|DTOFixtureTests|WorkDestinationTests|DesktopPerformanceFixtureTests|RegistryQueryTests'` → 222 run, then `TaskFilesTests` 33 passed; `ServerProtocolTests` 20 passed. `cargo nextest -p loopflow --lib 'test(server::schema::tests::) | test(lf::commands::read::)'` → 11 passed (schema fixture regenerated). Gate runs clippy, `documented_commands::`, golden.

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
3. ⌥⌘3, expand `rust/loopflow/src/ops/task/` in the tree, open `directory.rs`;
   in Diff, `]` past its last change opens the next changed file's Diff.
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
  <dir> --json`.
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
the navigator's `Changed files` section, Files' patch renderer (`TaskPatch*`).

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

- `uv run python scripts/test.py --reuse-passing` → swift suite passes. Exist:
  `TaskFilesTests` (lazy pages + Load more, failed node, remote reads via
  Machine, tree kept across Task switch), `PullRequestTests.deliveryDistinguishesEveryStateWithDistinctText`
    (all delivery states, `Task still open`), `TaskBriefTests` (related Tasks,
    outline), `ReviewDiffTests` (stream, cross-file next/previous, word
    emphasis, moves, selection reference).
- Demo/review (Jack): the six-step loop above on a real Task, including a
  no-PR Task and offline `gh`.


## After the first try in the Dev app — November 18

Jack, trying the build:

- "we need to change the whole paradigm of [pane strip screenshot] why does files show up three times"
- "Also Changes should probably just be called Diff? is that not more normal"
- On "24 changes in 44 files": "this is also weird, how can there be fewer changes than filkes. and i scrolled through all 24 and didnt find anything actually meaningful"
- "i think there neeeds to be some way to expand the 44 files thing sn then filter files/dirs"
- "in diff mode i also think we kinda want to ignore scratch/ and i want some way to always filter tests/ and maybe other things. what do the most loved diff-viewing tools do for these kinds of things"

What the run showed: this branch's comparison is 1,832,409 bytes and `lf task
diff` stops at 1 MB. Files arrive in path order (rust, scratch, swift), so the
24 changes were five Rust files and `scratch/` HTML; no Swift reached the stream.

Built in answer, not yet seen on screen:

- One row of tabs in the pane strip (Brief, Diff, Files, then other open tabs);
  the second header row with the Task name and segmented control is gone, and
  the strip's menu is labelled with the Task identifier. The Task name no
  longer appears in the pane.
- Page renamed Diff (`lf desktop` page argument `diff`). Files' per-file mode is
  also called Diff.
- A truncated comparison is completed with one read per remaining file.
- The title opens the changed files by top-level folder: a filter field, click
  to jump, Hide per folder, and an "Always hidden" list of path text matched
  anywhere in the path, ignoring case. It is one list for the whole app
  (`diff.hiddenPaths` in UserDefaults), starting with `scratch/`.

Where the hidden list lives. Jack: "I think this is personal, per-repo config.
I think we have that in like <repo>/.lf/config.yaml.local or something". That
file is `lf.config.local.yaml` at the repository root. Built: a `diff_hidden`
setting (personal, global or per repository, default `[scratch/]`); Desktop
reads its effective value and saves edits to the repository's private file.
Checked with the branch `lf` in a throwaway repository: `lf config set
diff_hidden '["scratch/","tests"]'` wrote the file and `lf config settings
--json` returned it. Not checked: Desktop's read and save against a running
server, and what the released `lf` prints on meeting the unknown key.

## Silent limits — November 18

Jack, on the 1 MB cut: "This is ... really bad, where else do we do stuff like this", then "what does it imply aobut our overall architecture that needs to chagen", and his own answer: "that we felt we couldnt afford to see a full diff".

Filed from it: d92894 (commit and PR writers' 20,000-character diff prefix; "file and start a task for fixing this", running) and 4556d0 (everything else; "i think all the other ones can be one more catch all", not started, lands after LOO-515).

On the writers' budget: "We should also let teh agent decide what to read in waht order i think". Sent to d92894 as a steer.

Private settings file location. Jack: "lf.config.local.yaml Is this the right place to keep this file? looks weird", then chose ".lf/config.local.yaml". Filed as Task 7333f4, not started.

Then: "you should jsut move it here its fien" and "make sure it gets auto hidden for users too". Done in this checkout: `PRIVATE_FILE` is `.lf/config.local.yaml`; a file at the old root path moves on first lookup, with the Git exclude rule written first. Task 7333f4 abandoned as done here. `global_commands::open_delivers_exact_task_to_desktop_without_preparing_its_execution` fails on this Machine with "No agent selected ... exactly one supported harness must be installed" in an isolated home; not run on unmodified code.

## Comparison with other diff tools — November 19

Pages: `scratch/diff-tools-guide.html` (comparison), `scratch/explore-diff-rivals.html` (three directions: A review then send once, B reader, C ordered by the worker).

Jack, on commenting: "do any of these diff readers allow commenting in the diff? I think we should add that (where the ocmment sends to the session)". Suggested Rune (https://rune.build/) as another tool to compare.

Jack, quoting T3's "Joins the composer draft with file, range and diff context; several collect before you send": "This feels right. doe t3 have tuis or is it chat only?"

Jack asked for the page: "can you show me 3 explorations that each redesign the experience around trying to match the best of the compettiors?" No direction chosen yet.

Built November 19: a Comment field in the selection bar. The comment travels on the same line as the reference into the session's unsent input (`ReviewSelection.reference(revision:comment:)`); each is typed as it is added. Collecting in the Diff and sending once is direction A, not built.

Jack, after the three directions: "I like the commenting from A. I like the context/sidedbar of B. I like the idea of auto-ordering from C". The page now leads with one combined pane built from those three parts; its four questions are unanswered. Read marks, one-file-at-a-time, syntax colors, pause, depth slider and folded renames were not named and are left out of the combined pane.

Jack, on how the tour is stored: "i would think the tour could live as some sort of simple yaml or something in scratch that basically just jump files to the top". The page's tour is now that: a list of files, optional why, unnamed files follow in the default order. Hunk grouping, the accounting check and the stale banner are dropped from the combined pane.

Jack, on the same file: "and maybe also auto sets filters as well". The page's file now has `first:` and `hide:`; how a worker's hide behaves on open is an open question on the page.

Jack, answering the page's questions on the worker's hide and stream-or-one-file: "1 hidden at start 2 stream, clicking beings it to top, headers lock into to a fixed top bar as you scroll". The left-out parts (read marks, syntax colors, pause, depth slider, folded renames) were not answered.

## Combined Diff pane — built November 19

Mock: `scratch/explore-diff-rivals.html`, the "Combined" panel. Comparison behind it: `scratch/diff-tools-guide.html`. Jack's words for each part are in the section above. Built, not yet seen on screen.

Demo: open a Task whose change spans many files. The Diff starts on the files `scratch/diff-order.yaml` names, not on `docs/`. Click a file in the sidebar and it comes to the top with its header in the bar. Open the skipped lines above a hunk. Comment on two lines in two files, press Send; the conversation's unsent input holds both, each naming its file and lines.

1. **Sidebar** (`ChangedFilesSidebar`, left of the stream): filter, files under the order's headings, comment counts, "Hidden by the worker for this change" with reasons and Show, "Always hidden" with its add field. `ChangedFilesPanel` and the title's toggle are deleted. Below 620 pt it opens over the stream from a toolbar button and closes on choosing a file. *Assumed: 620 pt, left side as in the mock.*
2. **File bar.** A fixed bar above the stream holds the header of the file under the stream's top edge (name, status, the tour's why, counts, Open). It sits outside the scroll view, so nothing next/previous lands on is covered. Departure: the in-stream file headers still scroll; they do not themselves stick. `LazyVStack` pinned headers were not used because `scrollPosition(id:)` lands rows under them.
3. **Skipped lines.** The row between hunks is a button; its lines are read with `lf task file` (the unsaved draft in Files' Diff) and replace the hunk header (`ReviewGap`, `ReviewFile.opening`). Offered only while the comparison reads the working tree. Opened lines close when their file's diff changes.
4. **Comments** (`ReviewComment`, `store.comments`): written in an editor under the selected lines, kept under their last line, counted in the sidebar; `ReviewCommentsControl` in both readers' bars sends them. `ReviewSelectionBar` is deleted; Copy and a bare reference are in the editor. Re-anchoring matches the commented lines' text to the nearest lines that still read the same; none is detached, shown under its file header (or at the top when the file left the comparison) and still sent. Sent comments clear only when the text reached the terminal (`addToConversation` returns Bool).
5. **Order** (`DiffOrder`, `ReviewGroup.arrange`, `ReviewKind`): as specified. Tour is the default when `first` has entries. The builtin `implement` skill carries the sentence telling the worker to write the file.

Limits found while building:

- The unsent input takes one line only. Desktop's literal typing refuses control characters, and a newline is safe only under bracketed paste, which Desktop cannot confirm for the program in the terminal. Several comments go as one line: `2 comments on the diff, <comparison>: (1) `path:L4` (after): … (2) …`.
- A comment on an opened (unchanged) line detaches when its file's diff changes, because the opened lines close then.
- A repeated short line (`}`) re-anchors to the nearest identical line, which can be the wrong one after an edit between them.
- This repository's gate clears `scratch/`, so a tour does not outlive gate here.
- By path groups by top-level folder, top-level files first, so it is not strict path order.

Not in this slice (shown on the page, not asked for): read marks, syntax colors, pause, a depth slider, folded renames, hunk-level grouping.

Check: `scripts/test_desktop.sh --filter 'TaskFilesTests|ReviewDiffTests'` → 47 passed; `cargo test -p loopflow --test golden_prompt` → passed after regenerating `builtin_implement.md`. Gate: `documented_commands::`, the full swift suite. Demo: the pane on screen, including the narrow sidebar and Send into a live conversation.
