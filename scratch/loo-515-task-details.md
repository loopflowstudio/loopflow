# LOO-515 · Task details beside the conversation

Research: `scratch/task-details-research.md`. Pages: `scratch/explore-diff-rivals.html`
(the "Combined" panel is the built Diff pane), `scratch/diff-tools-guide.html`,
`scratch/explore.html` (November 18 before/after), `scratch/explore-diff-pane.html`,
`scratch/explore-follow-agent.html`, `scratch/concept-review.html`,
`scratch/prototypes/`. Dated quotes and assumptions: `scratch/questions.md`.
The step-by-step account this file replaced: `ade953f915:scratch/loo-515-task-details.md`
(PR #1949's branch).

Status: built; nothing since the November 18 first try in the Dev app has been
seen on screen. PR #1949 is open.

## What exists

One PR, Swift plus Rust for a served read, a setting and the private settings file.

- **Pages.** The Files tool has Brief, Diff and Files as the first tabs of one
  pane strip, then other open tabs; the strip's menu is labelled with the Task
  identifier. `details` DesktopAction, ⌥⌘1/2/3, View menu; `lf desktop` page
  argument `diff`. No second header row; the Task's name is not in the pane.
- **Brief** (`TaskBriefView`): state, next move, condition, PR chip → Diff, due;
  related Task chips from identifiers in the brief text; outline from headings
  and `Acceptance:`; "Brief updated" marker; scroll kept per Task in-app.
- **Diff** (`TaskChangesView`, `ReviewStream`, `ReviewDiff`, `ReviewOrder`):
  - The PR-equivalent comparison (what `lf pr open` would show) as one stream,
    live from the file observation. Title `<n> changes in <m> files`, then
    `against <base> at <sha> · includes uncommitted`; base is GitHub's
    `baseRefName` once a PR is read, else "the Task's base".
  - A comparison past `lf task diff`'s 1 MB is completed with one read per
    remaining file.
  - Delivery pill in the title row (none/publishing/open/stale/ahead/
    unavailable/merged/abandoned, "Task still open"); click shows the card with
    every reading and its age.
  - **Sidebar** (`ChangedFilesSidebar`, left): filter, files under the order's
    headings, comment counts, "Hidden by the worker for this change" with
    reasons and Show, "Always hidden" with its add field. Under 620 pt it opens
    over the stream from a toolbar button.
  - **Order** (`DiffOrder`): `scratch/diff-order.yaml` with `first:` (files,
    optional why) and `hide:`; unnamed files follow in the default order. Tour
    is the default when `first` has entries; By path groups by top-level folder.
    The builtin `implement` skill tells the worker to write the file.
  - **Always hidden**: the `diff_hidden` setting (personal, global or per
    repository, default `[scratch/]`), path text matched anywhere, ignoring
    case; Desktop saves edits to the repository's `.lf/config.local.yaml`.
  - **File bar** above the stream holds the header of the file under the top
    edge (`ReviewFileTitle`, the same view as the in-stream header).
  - `ReviewStepper` (`]`/`[`, arrows) crosses files; Focus shows one file with
    a strip naming how many are hidden; Unified / Side by side (≥ 640 pt,
    disabled with its reason below); word emphasis; within-file moves collapsed
    with expand; landing flash; `End of changes`.
  - **Skipped lines**: the row between hunks opens the unchanged lines
    (`ReviewGap`), read with `lf task file`, only while the comparison reads the
    working tree.
  - **Comments** (`ReviewComment`): written under selected lines, counted in
    the sidebar, sent together by `ReviewCommentsControl` (⌘↩) into the
    conversation's unsent input; cleared only when the text reached the
    terminal. Re-anchored by matching line text; unmatched ones show detached
    and are still sent.
- **Files**: checkout tree from the served read `task.files` (local or the
  Task's Machine), folders read on open in pages of 500, M/R/U badges, keyboard,
  search over scratch, loaded entries and changed paths, failed reads inline
  with Retry. Diff mode is the same `ReviewStream` on one file, draft included;
  `]` past the last change opens the next changed file. Tree, selection and
  scroll live on `TaskFilesStore`, kept per Task in-app.
- **Private settings** live at `.lf/config.local.yaml`; a file at the former
  `lf.config.local.yaml` moves on first lookup, Git exclude written first.
- Deleted: `TaskPullRequestView`, the GitHub patch list and its store state,
  `TaskPatch*`, `ChangedFilesPanel`, `ReviewSelectionBar`, the navigator's
  `Changed files` section, the second header row.

## Decisions

- November 14, Jack: "i tink of htose, 1, 2, 5, 6 [live, not pause], 7 are the
  important ones for me" — next/previous change across files, word-level
  highlighting, selection to conversation, live updates, moved-code detection.
  Then "probably mostly B review steream", "maybe a little bit of A", "yeah lets
  build it with (b)" (moves collapsed with expand).
- November 18, on the first try: "why does files show up three times"; "Also
  Changes should probably just be called Diff? is that not more normal"; on
  "24 changes in 44 files": "how can there be fewer changes than filkes. and i
  scrolled through all 24 and didnt find anything actually meaningful"; "in diff
  mode i also think we kinda want to ignore scratch/ and i want some way to
  always filter tests/ and maybe other things". The run behind it: the
  comparison was 1,832,409 bytes, cut at 1 MB in path order, so no Swift
  reached the stream.
- November 18, on the cut: "This is ... really bad, where else do we do stuff
  like this"; "that we felt we couldnt afford to see a full diff". Filed:
  d92894 (commit and PR writers' 20,000-character diff prefix; steer: "We
  should also let teh agent decide what to read in waht order i think") and
  4556d0 (the rest, after LOO-515).
- November 18, dropped tracking the diff by agent turn: "in our system the task
  session is not even the one actually making the changes. The diff is really
  about the underlying worker not the highest-level session."
- November 18, redesign brief: "focus on clarity, polish, and delight, not
  trying to change the fundamental behavior that it enables or functionality";
  on the before/after page: "yeah looks good".
- November 18, the hidden list: "I think this is personal, per-repo config".
  The file: "lf.config.local.yaml Is this the right place to keep this file?
  looks weird", chose ".lf/config.local.yaml", then "you should jsut move it
  here its fien" and "make sure it gets auto hidden for users too".
- November 19: "I like the commenting from A. I like the context/sidedbar of B.
  I like the idea of auto-ordering from C". The tour: "i would think the tour
  could live as some sort of simple yaml or something in scratch that basically
  just jump files to the top" and "maybe also auto sets filters as well". On
  the worker's hide and the reading mode: "1 hidden at start 2 stream, clicking
  beings it to top, headers lock into to a fixed top bar as you scroll".

Assumed, not answered: sidebar on the left docking at 620 pt; ⌥⌘1/2/3; the file
bar outside the scroll view as "headers lock"; several comments as one line.

## Limits found

- The session's unsent input takes one line: Desktop's literal typing refuses
  control characters and cannot confirm bracketed paste. Several comments go as
  `2 comments on the diff, <comparison>: (1) `path:L4` (after): … (2) …`.
- In-stream file headers scroll; `LazyVStack` pinned headers cover the row
  `scrollPosition(id:)` lands on, which is where next/previous lands.
- Change ids are line-anchored, so no "new changes below" chip: an edit above
  shifts every later id.
- A comment on an opened unchanged line detaches when its file's diff changes.
  A repeated short line (`}`) can re-anchor to the wrong identical line.
- Moves are found only between a pure removal and a pure addition in one file.
- The file menu and sidebar carry no per-file +/−; file headers do.
- This repository's gate clears `scratch/`, so `scratch/diff-order.yaml` does
  not outlive gate here.

## Remaining

- Demo/review on screen (below). Unseen so far: the tab strip, the Diff
  rename, the completed comparison, sidebar (narrow too), file bar, skipped
  lines, comments with Send into a live conversation, the order file, and
  Desktop's read and save of `diff_hidden` against a running server.
- Task identity. The brief asks that nobody wonders "which Task they are
  viewing"; the pane now shows only the identifier on the strip's menu, after
  "why does files show up three times" removed the header row. Whether that is
  enough is for the demo.
- Open choices: whether Focus still earns its button beside the sidebar and
  Files' Diff; whether a tour should survive gate; the parts shown on the page
  and not named (read marks, syntax colors, pause, depth slider, folded
  renames, hunk grouping) stay out.
- The released `lf` reads private settings at `lf.config.local.yaml`; once a
  branch build moves the file, the released one stops seeing them until it
  ships. Not checked: what the released `lf` prints on the unknown
  `diff_hidden` key. The former-path move in `config/files.rs` names no Task
  that removes it.
- `global_commands::open_delivers_exact_task_to_desktop_without_preparing_its_execution`
  failed on this Machine in an isolated home ("No agent selected ... exactly
  one supported harness must be installed"); not run on unmodified code. CI
  decides.
- Not built: moves inside mixed change runs; drag to select lines.
- Noted, not this Task: a `.changes(comparison:)` browser tab and the Diff page
  are two representations of one view; merging them changes the persisted
  `WorkBrowserTab` shape, which LOO-497/416 own.

## Demo

Conversation on the left, Files on the right, on a Task whose change spans many
files:

1. ⌥⌘1, **Brief**: state line, outline, related Task chips that open in-app.
   Scroll halfway.
2. ⌥⌘2, **Diff**: starts on the files `scratch/diff-order.yaml` names, with
   `scratch/` hidden. Click a file in the sidebar; it comes to the top with its
   header in the bar. `]` steps across files; open the skipped lines above a
   hunk; expand a move; toggle Side by side. Comment on two lines in two
   files, Send; the conversation's unsent input holds both.
3. ⌥⌘3, **Files**: expand a folder, open a changed file; in Diff, `]` past its
   last change opens the next changed file.
4. Ask in the conversation; switch to another Task and back: tree, scroll and
   comments are where they were.
5. Edge cases: no-PR Task, `gh` offline (`GitHub unavailable · last read …`),
   merged PR on an open Task, a 20k-word brief, a comparison over 1 MB, a
   `node_modules`-scale folder, a deleted file, a remote Task.

## Forbidden outcomes

- A second diff renderer, or a file list that disagrees with `lf task changes`.
- A GitHub reading without its read time, or green checks on a failed or stale
  reading.
- Any local path fallback for a remote or unavailable checkout.
- `merged` or all-green read as Task completion.
- New panes, or hiding the conversation to show details.
- A comparison cut short without saying so.

## Exclusions

- Page, scroll and tree expansion across app restart (LOO-497/416 own the
  persisted intent).
- Waiting indicators in navigation rows (LOO-402).
- Recursive server-side file search, editing outside text files, an IDE.
- Linear relation fields; PR history beyond `RoadmapTask.pr`.
- Reusable Wave-level filters and prompt GitHub publication from the pane
  (selected November 14 as related outcomes, not built here).

## Done when

- Brief, Diff and Files are reachable by click and ⌥⌘1/2/3 with the
  conversation unchanged in size and focusable, and the pane says which Task it
  shows.
- Brief shows state, PR chip, related Tasks and an outline; empty and changed
  briefs read correctly.
- Diff distinguishes none / publishing / unavailable / stale / open / merged /
  abandoned, reads the whole comparison in a useful order, and sends comments
  to the conversation.
- Files browses the actual checkout (local or remote Machine), pages large
  folders, shows location, and keeps tree, selection, scroll and drafts across
  page and Task switches.
- No GitHub patch renderer remains.

Check (compress, November 19): `scripts/test_desktop.sh --filter 'TaskFilesTests|ReviewDiffTests|TaskBriefTests|PullRequestTests'` → 52 passed; `cargo test -p loopflow-execution --lib config::files::` → 13 passed. Gate/CI: clippy, `documented_commands::`, golden, the full swift suite, `global_commands`. Demo: the pane on screen.
