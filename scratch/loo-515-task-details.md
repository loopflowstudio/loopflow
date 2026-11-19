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
- `lf pr submit` and `lf pr land` clear added `scratch/` files in every
  repository, so `scratch/diff-order.yaml` ends at publish. In the `code` Graph
  `human-review` comes before `deliver`, so the tour exists when a person first
  reads the Diff and is gone for `verify` and for anyone reopening the Task
  after its PR is published.

## Concept review, November 19 (fifth pass)

Page: `scratch/concept-review.html`. Reviewed: the two commits since the fourth
pass (`7a7e367634`, `7b3484d303`). Both wording repairs that pass named are in:
Focus's strip reads "Only X · N more files", and the Swift `["scratch/"]` says
it is a copy of the catalog default. No concept merges or splits; the model
holds as one comparison read three ways (order, hidden, filter). Nothing here
needs a build before the demo.

Stays different on purpose:

- Two hide lists. The worker's `hide:` is per change, matches a path or folder
  prefix, carries a reason and is revealed entry by entry. Yours
  (`diff_hidden`) is per repository, matches text anywhere ignoring case
  ("always filter tests/") and is revealed by one toggle. Neither writes into
  the other; both stay in the count ("4 of 9 files").

Open, for the person at the demo (not answered by any recorded decision):

1. What is the pane called? It has four names: **Toggle Browser** (View menu
   and ⇧⌘E, on main), **Show Task Details** (this branch's action), "Files"
   (`swift/README.md`: "show / hide Files", "Toggle Files", which is not a menu
   item on main either) and the Task identifier on its own menu. Its first
   three tabs are now Brief, Diff and Files, so "Files" names both the pane and
   one tab. This is the same question as Task identity below.
2. Focus: with a sidebar click bringing a file to the top, the filter narrowing
   the stream and Files' Diff showing one file, does Focus keep its button?
3. Should the worker's order outlive publish (see Limits found)?

## Remaining

- Demo/review on screen (below). Unseen so far: the tab strip, the Diff
  rename, the completed comparison, sidebar (narrow too), file bar, skipped
  lines, comments with Send into a live conversation, the order file, and
  Desktop's read and save of `diff_hidden` against a running server.
- Task identity. The brief asks that nobody wonders "which Task they are
  viewing"; the pane now shows only the identifier on the strip's menu, after
  "why does files show up three times" removed the header row. Whether that is
  enough is for the demo.
- Open choices: the three under Concept review; the parts shown on the page
  and not named (read marks, syntax colors, pause, depth slider, folded
  renames, hunk grouping) stay out.
- The released `lf` reads private settings at `lf.config.local.yaml`; once a
  branch build moves the file, the released one stops seeing them until it
  ships. Not checked: what the released `lf` prints on the unknown
  `diff_hidden` key. Task b108f6 removes the former-path move in
  `config/files.rs` (filed locally; its Linear export was queued behind a rate
  limit).
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

Check (implement, November 19, after the merge of main `932702681e`): `scripts/test_desktop.sh --filter 'TaskFilesTests|ReviewDiffTests|TaskBriefTests|PullRequestTests'` → 52 passed; `cargo test -p loopflow-execution --lib config::` → 104 passed (again after compress). Gate/CI: clippy, `documented_commands::`, golden, the full swift suite, `global_commands`. Demo: the pane on screen.

Jack, on the tour after publish: "Tour doesnt need to survive submitting the tour, but are there any good alternatives to how to implement it?" Then, on Devin Review's "hasn't analyzed the latest changes" banner with Analyze latest / Analyze automatically: "Maybe a button would make sense here too". Not yet built; proposal below.

Proposed (not decided): the tour records the commit it was written at (`at: <sha>` in `scratch/diff-order.yaml`). When the head has moved past it, or no tour exists, the Diff's order control offers **Write tour** / **Update tour**, which runs a headless agent in the Task's checkout that rewrites the file; the order switches to Tour when it lands. Files changed since `at` show where they fall today (after the named ones). On demand only; no automatic rewrite.

Jack: "i wonder if there is some way we can also to smarter changes over itme. we could allow repo level settings to prioritize certain folders or file extensions ro something". Not decided. Today By kind is a fixed rule in `ReviewKind` (`ReviewOrder.swift`): generated and lock files, tests, docs, source by path patterns.

## The tour skill and its button (November 19) — built

Jack, quoting the proposal ("a small skill whose whole job is to read the diff and write the YAML … the worker and the button write the same thing the same way"): "We need this, plus we need to put this into other skills and/or the overall loopflow guide."

1. **A builtin skill that writes the tour.** Its whole job: read the Task's comparison (`lf task diff`), write `scratch/diff-order.yaml` with `at:` (the head commit it read), `first:` and `hide:`, each entry `- path:` with an optional one-line `why:`. It rewrites the file whole, never appends. It decides only the order and what to hide; it edits nothing else. Name it for what it makes (*assumed:* `tour`). Keep it short and plain like the other builtins.
2. **Every skill that hands a change to a reader points at it** instead of restating the format: `implement` (replace its sentence), and any other builtin that changes the diff before someone reads it or builds the reading page (check `compress`, `human-review`, `demo`, `pr-publish`). `LOOPFLOW.md` is paid on every run: add one line there only if a run outside those skills (a plain chat that makes the change) would otherwise never write it; say which you chose and why.
3. **Desktop reads `at:`.** When the head has moved past it, the order control says how many commits old the tour is; files changed since fall after the named ones as today.
4. **The button.** In the order control: **Write tour** when there is none, **Update tour** when it is stale. It runs the skill headless in the Task's checkout, the way Desktop already starts Graph runs as children, shows that it is running, and switches to Tour when the new file lands. A failure says why in the pane. No automatic rewrite on push.
5. Tests a person would notice: a tour written at an older commit reads as stale; the skill's golden; the button's command line.

Not in this slice: the repository `diff_order` setting, `.gitattributes`, "Always hide?" (proposed in the section above, not decided).

Built: builtin `tour` (`rust/loopflow-definitions/src/builtins/task/skill/tour.md`, golden `tests/goldens/builtin_tour.md`). `implement`, `compress` (a final step) and `human-review` (when the file is missing or behind) point at `lf help tour`; `demo` and `pr-publish` don't change what a reader opens. `LOOPFLOW.md` unchanged: a plain chat's change gets its tour from Desktop's button, so every run need not pay for the line. Desktop parses `at:`; stale means a commit after `at` changed a file outside `scratch/` (committing the tour itself keeps it current), and the stream says "The tour was written at <sha> · N files changed since" — files changed, not commits, since that is the read that exists (`task.changes --base <at> --head <head>`). **Write tour** / **Update tour** sends the served `skill.run` write with the Task's checkout; a remote Task offers neither (the write would run outside its checkout). Limit: a run that fails after launching stays "Writing the tour…"; its log path is in the notice.

Jack, on ordering layers: "right, and we could say that the review guides basically stack ontop of the default diff_Order, but we still fall back. and likewise, the repo (or potentially peronsal) settings could also add more. and i think maybe a whole diff seciton that has filters and ordered list of groups"

Direction from that (sent to the running worker as a steer):
- One `diff:` setting section replaces `diff_hidden`: `diff.hidden` (path text to hide) and `diff.groups` (ordered list of `{title, paths}`). Settable in the repository's `.lf/config.yaml` (shared) and in personal settings, globally or per repository.
- Layers, top first: the tour (`scratch/diff-order.yaml`, this change only) → personal groups → repository groups → the built-in groups (today's `ReviewKind` rule, which becomes the default value). A file sits in the first group that matches it; files no layer names come last. Every layer only adds; nothing below it is lost.
- Hidden is the union of the tour's `hide`, personal and repository `hidden`; each stays labelled with where it came from in the sidebar, and only your own are editable there.
- The migration of an existing `diff_hidden` value: per AGENTS.md's shared-file rule, the released `lf` 0.16.0 doesn't know `diff_hidden` at all, so no fallback is needed; drop the key.

Jack: "i would also imagine that we want sort of anyting not covered to go between e.g. docs and generated". So unmatched files do not go last: `diff.groups` has one "everything else" slot, an entry `- rest: Everything else` (title optional). The built-in default is Source, Tests, Docs, rest, Generated. A layer that lists its own `rest` moves the slot; otherwise the slot stays where the lowest layer put it. Files no group matches land there.

Jack: "btw i would want doccs up top, then like core data mdoel stuff". Read as the default order: Docs, Data model, rest (source not otherwise named), Tests, Generated. Source as its own built-in group goes; it is what `rest` holds. Built-in Data model uses generic paths (migrations, `*.sql`, `schema`, `models/`, `*.proto`, `dto`); this repository adds its own in `.lf/config.yaml`: `rust/loopflow-model/`, `rust/loopflow-storage/`, `tests/fixtures/dto/`.

Jack: "ideally we would roughly suggest similar overall guidance in our default skill, and have it eplicitly codified in our repo config". So the order lives in two places on purpose: the tour skill says it as reading guidance (docs and the outcome first, then the data model and public APIs, then the logic that uses them, tests, generated last), and this repository's `.lf/config.yaml` writes the full `diff.groups` list out explicitly rather than leaning on the built-in default.

Jack, on the layers: "For this we should use the existing config parser to get final values". So Desktop does no layering of its own: it reads the final `diff` value from lf's settings resolution, and only the tour (a per-change file, not a setting) sits on top in Desktop. Found: list settings with `AppendReset` (e.g. `docs`, `exclude`) append each higher layer after the lower ones (`settings.rs:825`), so a "first match wins" rule would let the built-in groups win. The matching and display rule has to fall out of that final list.

Jack: "a little complicated on what do on how to combine groups from multiple config files (and/or the represetnation from scratch)". Chosen in this conversation (the operator's call, not Jack's; reversible): no group merging at all.
- `diff.groups`: Replace. The most specific layer that sets it wins whole (personal for this repo > personal > repository > built-in). To add to it, copy the list `lf config explain diff` shows.
- `diff.hidden`: AppendReset, the existing list combination: every layer adds; `[]` clears below.
- The tour is not groups: `first:` pins files above whatever groups resolve to, `hide:` adds to hidden for this change. Nothing to merge.
This supersedes the steer about same-title groups merging and last-match-wins.
