# LOO-515 · Task details beside the conversation

Research: `scratch/task-details-research.md`. Pages: `scratch/explore-diff-rivals.html`
(the "Combined" panel is the built Diff pane), `scratch/diff-tools-guide.html`,
`scratch/explore.html` (November 18 before/after), `scratch/explore-diff-pane.html`,
`scratch/explore-follow-agent.html`, `scratch/concept-review.html`,
`scratch/prototypes/`. Dated quotes and assumptions: `scratch/questions.md`.
The step-by-step account this file replaced: `ade953f915:scratch/loo-515-task-details.md`
(PR #1949's branch).

Status: built, the tour skill and the two diff settings included. Seen on
screen November 21, in Loopflow Dev; the repairs for what that showed (width,
file bar, syntax colors, the per-file read, the long-line cut, Retry, rows
that take the pane's width), the New code layout, the Diff polish (below), the
count up front and the windowed stream are built and unseen.
PR #1949 is open; its head follows this branch.

## What exists

One PR: Swift, plus Rust for a served read, two settings, the private settings
file and a builtin skill.

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
  - Read file by file (`loadReview`, `readPatches`): `lf task changes` names
    every file, then each shown file's `lf task diff --path` is read six at a
    time in stream order, a file near the viewport first. Hidden and
    filtered-out files are not read until shown. `lf task diff --path` is the
    one read, untracked files included (`diff_snapshot` adds their `--no-index`
    patch). New files render as contents: no `+`, no fill, one lazy row per
    line. A line over 1,000 UTF-16 units shows its first 1,000 and "N more
    characters · Show" (`ReviewLine.cut`, `openedLongLines`); no file
    collapses. A file past 1 MB shows its first 1 MB and "Cut at 1 MB"; a
    failed read says "Could not be read: …" with Retry (`FileNote`,
    `retryFile`).
  - Syntax colors (`SyntaxColors.swift`): comments, strings, numbers and
    keywords by file extension, in the stream and the Files editor.
  - Delivery pill in the title row (none/publishing/open/stale/ahead/
    unavailable/merged/abandoned, "Task still open"); click shows the card with
    every reading and its age.
  - **Sidebar** (`ChangedFilesSidebar`, left): filter, files under the order's
    headings, comment counts, "Hidden by the tour for this change" with
    reasons and Show, "Always hidden" with its add field. Under 620 pt it opens
    over the stream from a toolbar button.
  - **Order** (`DiffTour`, `ReviewGroup.arrange`): the tour's `first:` files,
    then the rest under the `diff_groups` headings, by path within each. The menu reads Tour / By
    group / By path; Tour is the default when `first` has entries. `ReviewKind`
    is deleted.
  - **Tour**: `scratch/tour.yaml` with `at:`, `first:` (files, optional
    why) and `hide:`, written by the builtin `tour` skill
    (`builtins/task/skill/tour.md`). `implement`, `compress` and `human-review`
    point at `lf help tour`; `LOOPFLOW.md` does not, so a plain chat's change
    gets its tour from the button. Behind means a commit after `at` changed a
    file outside `scratch/`, said once in `tour.md` and followed by
    `human-review`; the stream says "The tour was written at <sha> · N files
    changed since" (`task.changes --base <at> --head <head>`). **Write tour** /
    **Update tour** sends the served `skill.run` write with the Task's
    checkout. "Writing the tour…" ends when the file's content changes; once
    the launch has answered, the button is offered again beside the log path
    (`TourWriting`: idle, launching, launched, failed).
  - **Groups**: `diff_groups`, `Repository` applicability, `Replace`: the
    nearest layer that sets it wins whole, first match wins within it. A group
    is `{title, paths}` or `{rest: <title or null>}`, the slot for files no
    group matches. Default: Docs, Data model, `rest: Everything else`, Tests,
    Generated and lock files (`catalog.rs`). This repository's
    `.lf/config.yaml` writes the full list with its own Data model paths and
    `tests/goldens/` under Generated.
  - **Always hidden**: `diff_hidden`, `AppendReset`, default `[scratch/]`;
    every layer adds, each pattern labelled with the layer that last added it
    (Default, Repository, Personal, Override). Desktop edits only yours, in
    `.lf/config.local.yaml`; removing your last one unsets the key, since `[]`
    would clear the layers below.
  - One pattern rule for both settings: matched against `/` + the path,
    ignoring case; contained anywhere, or at the end when it starts with `*`.
  - Desktop reads both settings through `config.explain` and layers nothing
    itself; only the tour sits on top.
  - **File bar** above the stream names the file whose lines are at the top
    edge. It is the first file's only header; while another file's own header
    is the top row the bar still names the file above it, and a sidebar click
    lands on the file's first row with its header just off screen.
  - **Width**: built for 360 pt. Header, toolbar, file bar and stream each
    take exactly the pane's width with content at the leading edge
    (`fillsPane`), so no row can widen the column or sit centered in it. The
    toolbar keeps the sidebar toggle and stepper and gives the rest up in
    order: words become symbols, then the layout picker goes, then order and
    tour, then Focus; what is given up is in the … menu. Waiting comments
    have their own row; the title stacks over the delivery pill. The title
    no longer says "reading N more" (the toolbar's reading mark does, in its
    tooltip), so a reload changes no row's height or width.
    `theDiffFitsItsPane` measures header, toolbar, file title, comments strip,
    stream and page at 360, 480 and 640 pt.
  - **Count up front**: `TaskChangedFile.changes` is each file's runs of
    changed lines, counted in Rust (`count_changes`) from the same
    default-context patch the stream parses: both reads take
    `comparison_diff_args`, renames found and prefixes pinned (`-U0` aligns some blank lines
    differently: `config/files.rs` read 11 against 12). Title and stepper sum
    it (`TaskFilesStore.changeCount`); a file's own lines count only where no
    count came (Files' Diff, drafts).
  - **Rows and window**: every line of a change is its own lazy row (the
    first carries the change's id); `ReviewChange.pairs` is found once at
    parse. The scroll anchor and the current change are observed apart, and
    the top file (`topPath`) is stored, so scrolling redraws the file bar,
    sidebar and declaration bar only. The stream draws a window from
    `windowStart` of about 1,500 rows (`windowBudget`); a jump, step or link
    to a file outside it or more than a budget down starts it at that file;
    its end row draws more on appear; "N files above · Show" brings earlier
    files back with the top line held. Scroll reports (`scrolled(to:in:)`)
    never move it.
  - `ReviewStepper` (`]`/`[`, arrows) crosses files; Focus shows one file with
    a strip reading "Only X · N more files"; New code (default: the file as it
    is now, removed lines folded to "N old lines · Show") / Unified / Side by
    side (≥ 640 pt, disabled with its reason below); changed lines tinted
    fully, gutter too; word emphasis; within-file moves collapsed
    with expand; landing flash; `End of changes`.
  - **Skipped lines**: the row between hunks opens the unchanged lines
    (`ReviewGap`), read with `lf task file`, only while the comparison reads the
    working tree.
  - **Comments** (`ReviewComment`): written under selected lines, counted in
    the sidebar, sent together by `ReviewCommentsStrip` (⌘↩) into the
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
- November 19, the tour after publish: "Tour doesnt need to survive submitting
  the tour, but are there any good alternatives to how to implement it?" On
  Devin Review's "hasn't analyzed the latest changes" banner: "Maybe a button
  would make sense here too". On a skill that reads the diff and writes the
  YAML: "We need this, plus we need to put this into other skills and/or the
  overall loopflow guide."
- November 19, ordering: "i wonder if there is some way we can also to smarter
  changes over itme. we could allow repo level settings to prioritize certain
  folders or file extensions ro something"; "the review guides basically stack
  ontop of the default diff_Order, but we still fall back. and likewise, the
  repo (or potentially peronsal) settings could also add more. and i think
  maybe a whole diff seciton that has filters and ordered list of groups";
  "i would also imagine that we want sort of anyting not covered to go between
  e.g. docs and generated"; "btw i would want doccs up top, then like core data
  mdoel stuff"; "ideally we would roughly suggest similar overall guidance in
  our default skill, and have it eplicitly codified in our repo config".
- November 19, layering: "For this we should use the existing config parser to
  get final values"; "a little complicated on what do on how to combine groups
  from multiple config files (and/or the represetnation from scratch)".
- November 21, on the Diff pane in Loopflow Dev (about 600 pt wide): "still
  deosnt seent to be workign"; "(1) not aligned right and jumps when i hover
  back and forth across panes (2) TaskChangesView not showing"; "also, no
  syntax highlighting". The window showed content wider than the pane, clipped
  on both edges (one fixed toolbar `HStack` set the column's width, "Write
  tour" squeezed to "Wr it…"), and `TaskChangesView.swift` named twice, in the
  file bar and its own header, with the 1 MB notice between.
- November 21, by steer: on the 1 MB notice for a file `diff_hidden` hides,
  "well we're supposed to be hing that right?"; "does that diff need to be 1+
  MB? woudl it be better if we handled new files seaparately"
  (`scratch/task-details-research.html` is 1.28 MB in 239 lines; the steer
  asked for the read T3 Code does: stats always, patches by path); on
  collapsing by size, "Ive always found this annoying, its like, often the
  largest diffs are the ones that matter the most. [ignoring like generated
  files, which we are trying to handle a separate way]".

- November 21, after the width repairs, by steer: "still jittery, still not
  fitting in the pane"; "does t3code have a minimum pane width bc it seems
  like we need a quite wide pane for this to work". The steer's reading of T3
  Code: its diff panel is `w-[42vw] min-w-[360px] max-w-[560px]`, the header
  hides secondary controls under 30rem, every label truncates, code wraps;
  asked: fit 360 pt, controls given up by priority, nothing in the toolbar
  sets the column's width, nothing changes size on hover, test at 360, 480
  and 640 pt.

- November 21, Jack: "im realizing i prefer a view that basically only shows the new code. like in a 2col sideby side in github, i just read the right side", "that should be default, maybe a button to show ol dl ine or something", "or sure 3-part button with my version as the perferred". Built by the session: New code / Unified / Side by side, New code default; removed lines fold to "N old lines · Show". Also: "this seems a little hard to read / subtle coloring" (changed lines now full tint, gutter too) and "this is also a little dense" (tour sidebar reasons quieter, two lines; tour skill asks for reasons under ten words).
- November 21, on the diff polish list: "Diff per agent turn — doesnt fit our model; would be per graph step. could be a nice thing but i mostly dont care." "Viewed checkboxes - yeah this could ok" "real grammar / better highlighting : high priority" "change markers: yeah, medium prii" "copy woithouth +/-: medium pri" "Sticky function headers: low pri but nice" "Rendered diffs for markdown/images/html: highpri" "word-wrap-toggle: low pri".
- November 21, on filing the polish as separate Tasks: "but do em all unless theres some sort of trouble or conflict", then "no, do the in 515", "what of this could really be that hard", "i also feel like there still could be more syntax highlighting", "maybe diff per agent turn?", "er diff per step?". The seven separate Tasks were abandoned unstarted.

- November 21 evening, on the branch build: "i dont like how the # of changes increases as it loads more files. it should know up front"; "teh right hand red/green bar is too much, drop it" (dropped in cfc6c89cc5); "scrolling seems choppy, should we be paginating this thing more? its more important that its fast to jump via the siebar than that no pagination". The order pasted for scrolling: profile; draw every line as its own lazy row; "If it's still slow, paginate by file. The stream shows a window of files around where you are, and the sidebar, j/k and the marks jump between windows."

Assumed, not answered: sidebar on the left docking at 620 pt; ⌥⌘1/2/3; the file
bar outside the scroll view as "headers lock"; several comments as one line.

Chosen while building, not by Jack, reversible before release:

- No group merging. Jack asked for layers that "add more"; `AppendReset`
  appends higher layers after lower ones, so first-match over an appended list
  would let the built-ins win. Groups replace whole instead; hidden adds.
- Two flat keys, `diff_hidden` and `diff_groups`, where Jack said "a whole diff
  seciton": top-level `diff` is the released shorthand for `context.diff`, and
  `lf` 0.16.0 read `diff: {groups: …}` as that and warned "skipping
  context.diff: invalid value".
- "Docs up top, then data model" read as the default order Docs, Data model,
  rest, Tests, Generated.
- November 21: files within a group by path, not largest first (sizes are
  unknown until each file is read, and the stream would reorder as they
  arrive); a line over 1,000 UTF-16 units as "too long to read"; "near the
  viewport first" is a priority, so every shown file is read in the end and
  the change count stays whole; the toolbar gives up the layout picker
  first, then order and tour, then Focus; the title drops "reading N more";
  syntax inks are the working, available and
  warning tones, red and green left to removed and added lines; no
  highlighting library; the `[ ]` key hint is gone to fit 240 pt (the arrows'
  tooltips name the keys).

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
- `lf task changes` carries each file's change count but no line counts or
  added/deleted kind, so a file's +/− and status appear when its lines arrive.
- A file cut at 1 MB counts all its changes and steps through the ones shown.
- Why the pane jumped is measured headless, not confirmed on screen. Measured
  with `NSHostingController.sizeThatFits` before the second repair: toolbar
  minimum 211 pt, stream and file title under 240 pt (this branch's own diff
  included), and the header 399 pt wide in a 480 pt pane, 355 in 360: stacked,
  it took its text's width and sat centered in the column, so it was off the
  left edge and moved whenever the title changed ("· reading N more" on every
  reload). Nothing found in the Diff changes size on hover; in the pane strip
  (main's code) Close tab appears with focus, inside the hover-revealed
  buttons.
- Any read stops at 1 MB (`MAX_FILE_BYTES`): a file past it shows the first
  1 MB of its patch with "Cut at 1 MB". Reading past it is Task 4556d0's.
- A cut long line loses its word emphasis and syntax colors run on the shown
  part only; a line's opened state is kept by path and line number, so it
  follows that number if lines above change.
- Syntax is read one line at a time. A block comment carries across lines in
  the editor, not in the stream; Python triple-quoted strings and Markdown get
  no special reading.
- The editor recolors the whole buffer after each edit, up to 200,000 UTF-16
  units; a larger file keeps one color.
- `lf pr submit` and `lf pr land` clear added `scratch/` files in every
  repository, so `scratch/tour.yaml` ends at publish; after that the Diff
  offers Write tour again. Jack: "Tour doesnt need to survive".
- A remote Task offers neither Write tour nor Update tour (the `skill.run`
  write would run outside its checkout), and its files sit under one "Changed
  files" heading: there is no Swift copy of the default groups. The same
  heading shows until the settings read answers.
- No served read reports a launched Process ending, so a tour run that fails
  after launching keeps "Writing the tour…" with its log path until the file
  changes, the button is clicked again or the app restarts. A rewrite that
  parses to the same tour also leaves the notice up. A second click during a
  healthy run starts a second run that rewrites the same file.
- The released `lf` 0.16.0 prints "skipping diff_groups: unknown setting" on
  every command in a checkout whose `.lf/config.yaml` has the list (seen on
  `lf task status` here).

## Concept review, November 19 (sixth pass)

Page: `scratch/concept-review.html`. The model holds: one comparison read three
ways (order, hidden, filter); the settings give the default order, the tour
stacks on top for one change. Its three repairs are built (`6fb8943d03`) and
described above: one name for the tour, one behind rule, a button that returns.
Chosen by the review, not by Jack, reversible: the file name
`scratch/tour.yaml` (Jack: "the tour could live as some sort of simple yaml";
"diff_Order" was his word for the default groups), the heading "Hidden by the
tour for this change" (the writer is whoever ran the skill, not only the
worker), and accepting a second run from a second click.

Stays different on purpose: two hide lists. The tour's `hide:` is per change,
matches a path or folder prefix, carries a reason and is revealed entry by
entry. `diff_hidden` is a setting, uses the pattern rule above and is revealed
by one toggle. Neither writes into the other; both stay in the count ("4 of 9
files"). The tour skill's "docs, data model, logic, tests, generated" repeats
the default `diff_groups` in prose, as asked ("roughly suggest similar overall
guidance in our default skill").

Seventh pass, November 19, over the repairs as built: nothing to change. Read
at `b8b0f0484f`: no `diff-order`, `DiffOrder` or "worker's order" outside
`scratch/`; `tour.md` states the behind rule and `human-review.md` defers to
it; `TourWriting` is idle, launching, launched, failed, and the button returns
at launched. The five questions under Remaining are product choices no accepted decision
settles; each is about something only the screen shows, so they go to the demo
unanswered.

The `lf` installed on this Machine is `0.16.0+f5ff6a6c7`, a build of main past
the release, and prints the `diff_groups` warning twice per command until the
first `lf install` after this merges; question 5 below is about Machines on
the tagged release.

## Concept review, November 21 (eighth pass)

Read at `cd22a12c29`; its three repairs are built and described above: the
long line is cut instead of its file collapsing (`collapses`, `row(for:in:)`,
`openedLongFiles`, `ReviewFile.bytes`/`longestLine`, the `changes-long-lines`
row and the stepper's skip are gone, so the stepper reaches every change the
title counts), the `lf task file` reread and `ReviewFile.added(_:content:)`
are deleted, and a failed file read has Retry. Chosen by the review, not by
Jack, reversible. Left as is: Files' Diff mode reads its one file with
`loadDiff`, because it compares the unsaved draft.

Ninth pass, November 21, over those repairs and the 360 pt fit as built:
nothing to change. Read at `c9c7bc8b93`: no `collapses`, `openedLongFiles`,
`longestLine` or `changes-long-lines` outside `scratch/`; `readFile` makes one
read, `lf task diff --path`; `FileNote` is cut or failed and only failed has
Retry; a change and a long line share one id (`ReviewLine.id(in:)`); the
toolbar's four forms give up layout, then order and tour, then Focus, as
`swift/README.md` says. Two things are called cut and stay two: a long line
(Show brings the rest) and a file past 1 MB (nothing does until Task 4556d0).
Noticed then, replaced since by the count up front: while files were still
being read the title counted only the changes read so far. The five questions under Remaining are unchanged and unanswered.

Tenth pass, November 21, over the Diff polish at `9fdfc72f79`: one repair
for implement. A step is a Graph step's own commits (Jack: "steps do commits,
so we can just use the git logs"), but `changes_snapshot` lists `git log
base..head`, every commit reachable, merges included. On this branch the menu
would offer 54 entries, 10 of them `Merge <sha>`; picking `Merge a0548bfe28`
compares 1,624 files (+128,835 −136,735) of main's work as this Task's step.
After `lf sync <another Task>`, that Task's commits would join the list, and
`ReviewStep.steps` takes each entry's base from the entry before it, which is
then not its parent.

Built November 21: `lf task changes` walks `--first-parent --no-merges` and
`TaskCommit` carries `parent`; a step's base is its first commit's parent, and
a commit joins the step before it only when its parent is that step's head.
This branch's menu now reads 46 commits, none a merge. Checks:
`ops::task::tests::task_changes_offer_first_parent_commits_without_merges`,
`dto_fixtures::task_files_share…`, `server::schema::tests::` and the Swift
`commitsReadAsGraphSteps`, `aGraphStepComparesItsOwnCommits`, `DTOFixtureTests`
pass; the menu on screen is for the demo.

Left as chosen while building: a step also sets what Files' Diff and the
tree's badges read, as a `.changes(comparison:)` tab already does; under a step
Files' Diff compares commits, not the draft. Every non-`lf` commit stands
alone, so this branch would still offer 44 entries; whether those runs fold
together is for the screen, not settled by any decision.

Eleventh pass, November 21, over the repair at `3484509549`: nothing to
change. `--no-merges` leaves every listed commit one parent, so `%P` is that
parent (48 commits here, none with two); a commit after a merge compares
against the merge, so a step never carries main's or a synced Task's work.

Twelfth pass, November 21, over the count up front and the window at
`2d2f33f6c1`: one repair for implement. The plan says the count is read "from
the same default-context patch the stream parses"; it is a second `git diff`
with its own flags. `change_counts` passes `--find-renames` and
`--src-prefix=a/ --dst-prefix=b/`; `diff_snapshot`, which serves each file's
`lf task diff --path`, passes neither, so a Machine with `diff.renames=false`,
`diff.noprefix` or `diff.mnemonicPrefix` reads a different patch from the one
counted. A renamed and edited file then counts its edits in the title and
steps through one whole-file addition, and the stepper never reaches its
total. Direction: one function builds the comparison's diff arguments and both
reads use it; a test parses the per-file patch of the count fixture in Swift
and finds the stops `lf task changes` counted. The stop rule itself agrees:
both start a change after an unchanged line or a hunk header and on a removal
after additions, and a move is two stops on each side. Repaired: both reads
take `comparison_diff_args`; `tests/fixtures/review/change_counts.patch` is
counted in Rust and stepped in Swift, which found Swift kept Git's trailing tab
on `+++ b/with space.txt` (fixed in `ReviewDiff.strip`).

Left as built: the title counts every shown file, Viewed ones included, while
the stepper's total leaves Viewed files out ("2 of 4" under "6 changes"); a
file read from a later live update keeps the count from the file list until
the list is reread. The window's "N files above · Show" button where Jack's
pasted order said jumps "between windows" is the chosen form; whether
scrolling up to it reads as a wall is for the screen.

Thirteenth pass, November 21, over the repair at `969f707c90`: nothing to
change. `change_counts` and `diff_snapshot` both build their arguments with
`comparison_diff_args`, and the untracked files' `--no-index` patch takes the
same `PINNED_PATCH_FLAGS`; an untracked file's count is `untracked_changes`,
one addition. A step's read replaces the counts with its own comparison's,
since `showStep` clears `changes` and `refreshChanges` reads the step.

## Remaining

- Demo/review on screen (below). November 21 showed the tab strip, the Diff
  rename and the stream; nothing was said about them beyond the quotes under
  Decisions. Unseen: the November 21 repairs at several widths and under
  hover across panes (360, 480, 640 pt), a large new file read as contents,
  a cut long line, Retry on a failed read, the sidebar (narrow too), skipped lines, comments with Send into a
  live conversation, the tour with Write tour / Update tour and its behind
  notice, By group under this repository's `diff_groups`, and Always hidden's
  layers and save against a running server; New code with its old-line
  folds, the step menu, Viewed, the enclosing declaration,
  copy without markers and Wrap long lines; the count whole from the first
  frame, scrolling and sidebar jumps on this PR's Diff, "N files above · Show".
- Open, for the person at the demo:
  1. What is the pane called? It has four names: **Toggle Browser** (View menu
     and ⇧⌘E, on main), **Show Task Details** (this branch's action), "Files"
     (`swift/README.md`: "show / hide Files", "Toggle Files", not a menu item
     on main either) and the Task identifier on its own menu. "Files" names
     both the pane and one tab.
  2. Task identity. The brief asks that nobody wonders "which Task they are
     viewing"; the pane shows only the identifier on the strip's menu, after
     "why does files show up three times" removed the header row.
  3. Focus: with a sidebar click bringing a file to the top, the filter
     narrowing the stream and Files' Diff showing one file, does Focus keep
     its button?
  4. The three choices under "Chosen while building": replace instead of
     merge, flat keys instead of a `diff:` section, the default order.
  5. This repository's `diff_groups` block makes the released `lf` warn on
     every command in every checkout from merge until a release knows the key.
     Keep the block and take the warning, or land the block with that release.
     The block is what "eplicitly codified in our repo config" asked for.
- Answered: the tour does not outlive publish.
- The parts shown on the explore pages and not named (read marks, pause,
  depth slider, folded renames, hunk grouping) stay out. Syntax colors came in
  on November 21.
- The released `lf` reads private settings at `lf.config.local.yaml`; once a
  branch build moves the file, the released one stops seeing them until it
  ships. Task b108f6 removes the former-path move in `config/files.rs` (filed
  locally; its Linear export was queued behind a rate limit).
- `global_commands::open_delivers_exact_task_to_desktop_without_preparing_its_execution`
  failed on this Machine in an isolated home ("No agent selected ... exactly
  one supported harness must be installed"); not run on unmodified code. CI
  decides.
- Not built: moves inside mixed change runs; drag to select lines.
- Grammar highlighting and rendered diffs (Markdown, images, HTML) are Task c9fbcb's.
- Noted, not this Task: a `.changes(comparison:)` browser tab and the Diff page
  are two representations of one view; merging them changes the persisted
  `WorkBrowserTab` shape, which LOO-497/416 own.

## Demo

Conversation on the left, Files on the right, on a Task whose change spans many
files:

1. ⌥⌘1, **Brief**: state line, outline, related Task chips that open in-app.
   Scroll halfway.
2. ⌥⌘2, **Diff**: with no tour, Write tour; then it starts on the files
   `scratch/tour.yaml` names, the rest under Docs, Data model, Everything
   else, Tests, Generated, with `scratch/` hidden. Commit a source change:
   Update tour. Click a file in the sidebar; it comes to the top with its
   header in the bar. `]` steps across files; open the skipped lines above a
   hunk; expand a move; toggle Side by side. Comment on two lines in two
   files, Send; the conversation's unsent input holds both.
3. ⌥⌘3, **Files**: expand a folder, open a changed file; in Diff, `]` past its
   last change opens the next changed file.
4. Ask in the conversation; switch to another Task and back: tree, scroll and
   comments are where they were.
5. Edge cases: no-PR Task, `gh` offline (`GitHub unavailable · last read …`),
   merged PR on an open Task, a 20k-word brief, a new file over 1 MB, a
   minified file, the pane at 600 pt and narrower with the pointer moving
   across panes, a `node_modules`-scale folder, a deleted file, a remote Task.

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

Measured (November 21 evening, `DiffScrollMeasurement`, this Task's Diff with nothing hidden, 76 files, 600 × 900 pt): before, scroll frames of 40 pt median 4.6 ms, p95 12.1, 5/300 over 16.7 ms, sidebar jumps median 245 ms, max 1,102 ms. Separating anchor from current alone: about the same. Lazy line rows: p95 9.7, 2/300, jumps median 109, max 1,524 (the farthest). With the window: jumps median 92, max 119 ms; "files above" 350 ms; first frame 92 → 51 ms. A per-line syntax span cache was tried and dropped: jumps draw unseen lines, so it never hit. Jumps keep ~90 ms of drawing a new screen of rows; whether that reads as instant is for the screen.

Check (November 21 evening): `cargo test -p loopflow-execution --lib task_changes_` 2, `dto_fixtures::` 49, `server::schema::tests::` 8, `documented_commands::` 4 passed; `scripts/test_desktop.sh --skip-build --filter 'TaskFilesTests|ReviewDiffTests|DTOFixtureTests|RegistryQueryTests'` 135 passed. Gate/CI: clippy, the full Swift suite (the FSEvents tests `commitInLinkedWorktreeRereadsTheComparison` and `ignoredWritesFromReadsNeverScheduleAnotherRead` failed earlier under a busy `fseventsd`), `global_commands`. Demo: scrolling and jumps on screen.

Check (November 21 night, shared `PINNED_PATCH_FLAGS` for counts, the Diff and untracked files): `cargo test -p loopflow-execution --lib ops::task::` 65 passed, `cargo clippy -p loopflow-execution --all-targets` clean; `scripts/test_desktop.sh --filter ReviewDiffTests` 15 passed, `stopsMatchTheCountsTheServerSends` among them.

## Diff polish, November 21 — built

Steer, November 21: "ok grammar highlighitng and rendered diffs can be a single v2 task"; items 1 and 2 leave LOO-515 for Task c9fbcb ("Diff v2"), `SyntaxColors.swift` stays as it is. Built here, unseen on screen:

- **Diff per step.** `lf task changes` carries `commits` (`{sha, parent, subject}`, first-parent, no merges, oldest first; `TaskCommit` in Rust and Swift, fixtures and schema updated). `ReviewStep.steps` reads a run of one `lf <step>:` prefix as one step when each commit's parent is the step's head, any other commit alone; a step's base is its first commit's parent. A menu under the title ("Whole Task · N steps") picks one; `showStep` compares `.branch(base: parent of its first, head: its last)` and the stream reads anew. Steps come only from the whole Task's read. A pane tab change returns to the whole Task.
- **Copy without + and −.** The marker is not selectable; each line's context menu copies the line, the selected lines, or the side's shown code (`ReviewSelection.side`).
- **Viewed** per file: checkbox in the file title and bar; folds the file to its header, sidebar "12 / 60 viewed" and a check per row, stepping passes over it; `ReviewFile.fingerprint` clears it when the file's lines change. In memory only.
- **Enclosing declaration** under the file bar, overlaid on the stream (`ReviewFile.enclosing`): the nearest shown line above the top row that opens a function or type (or a Markdown heading), else git's hunk declaration.
- **Wrap long lines** in the … menu, which is now always shown; off, each line keeps one row and ends in "…".

Chosen while building, reversible: steps are consecutive runs (a second `implement` after `gate` is a second step); wrap off truncates instead of scrolling sideways (the November 21 width repairs ruled out a row wider than the pane); viewed does not outlive the app.
