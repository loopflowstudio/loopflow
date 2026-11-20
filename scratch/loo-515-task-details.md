# LOO-515 · Task details beside the conversation

Research: `scratch/task-details-research.md`. Pages: `scratch/explore-diff-rivals.html`
(the "Combined" panel is the built Diff pane), `scratch/diff-tools-guide.html`,
`scratch/explore.html` (November 18 before/after), `scratch/explore-diff-pane.html`,
`scratch/explore-follow-agent.html`, `scratch/concept-review.html`,
`scratch/prototypes/`. Dated quotes and assumptions: `scratch/questions.md`.
The step-by-step account this file replaced: `ade953f915:scratch/loo-515-task-details.md`
(PR #1949's branch).

Status: built, the tour skill and the two diff settings included; nothing since
the November 18 first try in the Dev app has been seen on screen. PR #1949 is
open at `6d503e2991`, mergeable, level with main.

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
  - A comparison past `lf task diff`'s 1 MB is completed with one read per
    remaining file.
  - Delivery pill in the title row (none/publishing/open/stale/ahead/
    unavailable/merged/abandoned, "Task still open"); click shows the card with
    every reading and its age.
  - **Sidebar** (`ChangedFilesSidebar`, left): filter, files under the order's
    headings, comment counts, "Hidden by the tour for this change" with
    reasons and Show, "Always hidden" with its add field. Under 620 pt it opens
    over the stream from a toolbar button.
  - **Order** (`DiffTour`, `ReviewGroup.arrange`): the tour's `first:` files,
    then the rest under the `diff_groups` headings. The menu reads Tour / By
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
  - **File bar** above the stream holds the header of the file under the top
    edge (`ReviewFileTitle`, the same view as the in-stream header).
  - `ReviewStepper` (`]`/`[`, arrows) crosses files; Focus shows one file with
    a strip reading "Only X · N more files"; Unified / Side by side (≥ 640 pt,
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

The `lf` installed on this Machine is `0.16.0+f5ff6a6c7`, a build of main past
the release, and prints the `diff_groups` warning twice per command until the
first `lf install` after this merges; question 5 below is about Machines on
the tagged release.

## Remaining

- Demo/review on screen (below). Unseen so far: the tab strip, the Diff
  rename, the completed comparison, sidebar (narrow too), file bar, skipped
  lines, comments with Send into a live conversation, the tour with Write tour
  / Update tour and its behind notice, By group under this repository's
  `diff_groups`, and Always hidden's layers and save against a running server.
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
- The parts shown on the explore pages and not named (read marks, syntax
  colors, pause, depth slider, folded renames, hunk grouping) stay out.
- The released `lf` reads private settings at `lf.config.local.yaml`; once a
  branch build moves the file, the released one stops seeing them until it
  ships. Task b108f6 removes the former-path move in `config/files.rs` (filed
  locally; its Linear export was queued behind a rate limit).
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

Check (November 19): `cargo test -p loopflow-execution --lib config::` → 105 passed (before the conflict-free merge of main `2750c9e0fb`); after the sixth-pass repairs and their compress `scripts/test_desktop.sh --filter 'TaskFilesTests|ReviewDiffTests'` → 51 passed and `cargo test -p loopflow --test golden_prompt` → passed with regenerated goldens; branch `lf config explain diff_groups` selects the repository's list. Gate/CI: clippy, `documented_commands::`, the full swift suite, `global_commands` (PR #1949's suites are skipped at `6d503e2991`). Demo: the pane on screen.
