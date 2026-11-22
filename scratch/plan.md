# c9fbcb Diff v2: grammar highlighting and rendered diffs

Stacked on LOO-515 (#1949). Options page: `scratch/explore.html` (round 1). Concept review: `scratch/concept-review.html` (headless, November 21) made four choices for review, folded in below: one read of each side serves highlights, skipped lines and rendered readers; the per-file control is Rendered · Source and the Diff's layout arranges both; images stay `binary` with `data`; Markdown blocks pair like lines do.

## Decisions

Jack Heart, November 21, answering the explore page: "2a 2a 3a 4c 5a 6a 7b", then "1a".

1. Engine: **A**, native tree-sitter in the app, coloring both the Diff and the Files editor.
2. Theme colors: **a**, derived from each theme's terminal palette (ANSI 1–6), so custom themes work.
3. A changed Markdown file opens **rendered**, with source one click away.
4. Markdown marks: **redline by default**, with side by side one click away.
5. A changed image opens **side by side**.
6. A changed HTML file opens as **two live pages side by side**.
7. First languages: **this repository's** (Swift, Rust, Python, TypeScript/TSX, JavaScript, Markdown, YAML, TOML, JSON, SQL, shell, HTML, CSS) **plus Go, C/C++, Ruby, Java and Kotlin**. Later the same day: "Wait why would we not kee as much color as pissible. you can add them all to the cosen grammar list" (Dockerfile, Makefile, justfile, ini, Objective-C, C#), and "make sure we have zig too".

## Done when

- A Task's Diff colors keywords, types, functions, properties, strings, numbers and comments by grammar in every language with a grammar, in every bundled theme, at 4.5:1 or better on the row it sits on. A hunk that opens inside a Python docstring colors as a docstring once the file's text is read.
- The Files editor colors through the same grammar while you type, with no size limit of its own (the 1 MB read cap still applies).
- A changed `.md` opens rendered, redlined in New code and Unified; the layout picker's Side by side shows old and new blocks in two columns; the file's Source button shows today's line Diff.
- Skipped lines open in a step or other committed comparison too, read from that comparison's newer commit, and color like the rows around them.
- A changed PNG, JPEG, GIF, WebP or SVG shows old and new side by side with pixel sizes and byte sizes. "Binary file · no text to compare" no longer appears for images.
- A changed `.html` shows the old and new page live, side by side, offline.

## Findings that shape it

**Grammar packages can't come from SPM.** Probed November 21 in `/tmp/tsprobe` (Swift 6.2.3): all 19 grammar repositories have a `Package.swift` and resolve together beside SwiftTreeSitter 0.25.0 (5 min 20 s cold). But five of them (JavaScript, Python, CSS, YAML, Go) add `scanner.c` only `if FileManager.default.fileExists(atPath: "src/scanner.c")`, which is false under SwiftPM's manifest sandbox, so JavaScript, Python, CSS and YAML fail to link. `DerekStride/tree-sitter-sql` commits no `parser.c` at all; its GitHub release tarball has one. Swift needs the `0.7.4-with-generated-files` tag.

**Vendoring the C is too big to commit.** The generated parsers total ~134 MB of C (Kotlin 23.6 MB, Swift 23.1 MB, TypeScript 17.5 MB, C++ 17.3 MB, Ruby 15.3 MB). Compiled `-Os` for arm64 they come to ~22 MB of code and data. The app is arm64-only and 129 MB today, so it grows about 17%, not the 8–15 MB the explore page guessed.

**So the grammars ship like GhosttyKit:** one prebuilt `TreeSitterGrammars.xcframework` as a SwiftPM `binaryTarget` with URL and checksum, built by `uv run python scripts/loopflow-dev.py grammars-build` from sources pinned by URL and SHA-256 in `third_party/manifest.json` (release tarballs or tag archives that contain `parser.c`). It exposes one header, `const TSLanguage *tree_sitter_<name>(void)` per language. The tree-sitter runtime and SwiftTreeSitter come from SwiftPM (`ChimeHQ/SwiftTreeSitter`, which depends on `tree-sitter/tree-sitter` 0.25; both built cleanly in the probe). Grammar ABIs are 14 and 15; runtime 0.25 accepts both. Wire both into `swift/Package.swift` and `swift/project.yml`. A new grammar set uploads under a new, checksum-named zip; a hosted one is never overwritten.

**Queries are ours, committed.** Upstream `highlights.scm` files inherit: TypeScript's (515 bytes) assumes JavaScript's is prepended, C++'s assumes C's (C++'s alone fails to compile against its grammar: "nodeType(43)"). Markdown is two grammars, block and inline. Queries live in `swift/LoopflowMac/Grammars/<language>/highlights.scm`, copied from the pinned source by `grammars-build --queries` and edited in place, so capture names can be normalized and reviewed. Each `Grammar` names the query files it concatenates.

**Speed.** `swift/LoopflowMac/WorkModel.swift` (3,649 lines, 185k UTF-16 units): parse 21 ms, query and resolve 129 ms, 37,801 captures, release build. A 20-line Rust or Swift snippet takes under 0.5 ms. So highlighting runs off the main thread everywhere, and the editor applies colors when they arrive for the text revision they were computed from.

**Theme inks.** Of the 24 bundled themes, 4–6 have each of ANSI 3, 4, 5 and 6 under 4.5:1 on their background (worst: Noctis Lilac 1.85:1, Gruvbox Light 2.19:1, Catppuccin Latte 2.31:1). Each ink is the theme's ANSI color mixed toward `text` just far enough to reach 4.5:1 against the surface and the added and removed row fills (`ThemeRGB.mixed`, `contrast`). Red (1) and green (2) stay with removed and added lines.

**Images and SVG.** `NSImage(data:)` reads SVG (`_NSSVGImageRep`), PNG, JPEG, GIF and WebP; no decoder needed. Served answers spill to bulk up to 32 MB (`MAX_BULK`, `rust/loopflow-execution/src/machine/wire.rs`), so an image of up to 16 MB fits as base64.

## Source of truth

The Task checkout's Git objects and working tree, read through `lf task file`. The app holds no file contents of its own beyond the reads it caches.

### `lf task file <issue> <path> [--at <rev>]`

- `--at` takes what `lf task diff --base` takes (`parent`, `head`, or a resolved 40-hex SHA, through `resolve_file_base`) and reads the path from that commit with `git cat-file blob <sha>:<path>`. Absent, it reads the working tree as today. A path missing at that commit is `missing`. A read at a commit carries `read_only_reason: "A committed version is read-only."` and no recoveries. Named `--at` rather than `--base` because it names the version read, not a comparison (chosen for review).
- `TaskFileSnapshot` gains a required nullable field `data`: a `binary` file's bytes, base64, up to 16 MB (`MAX_BINARY_DATA_BYTES`); null over that and for every other state. No new state: whether bytes are an image is the decoder's call, made once in the app by `NSImage(data:)`, so no extension list lives in Rust (chosen for review; passed over: an `image` state keyed by extension, which puts the same list in Rust and Swift). `size_bytes` already carries the size a too-large file names. SVG stays `text`; the app renders its content.
- Rust `TaskFileSnapshot`/`TaskFileState` in `rust/loopflow-execution/src/ops/task.rs`, reader in `ops/task/snapshot.rs`; served `task.file` gains `at`; Swift `TaskFileSnapshot`/`TaskFileState` in `swift/Loopflow/Models/TaskWorkspace.swift` and `RegistryQuery.taskFile(issue:path:at:)`; the DTO fixture under `tests/fixtures/dto/` and each language's fixture test gain `data` and a `binary` case carrying it.

## In the app

- **`Grammar`** (`swift/LoopflowMac/Grammar.swift`) replaces `SyntaxColors.swift`. `Grammar.named(path)` picks by extension or file name, as `SyntaxLanguage.named` does today, adding `.kts`. Every language the old colorer covered keeps color: Dockerfile, Make, just, INI (`.ini .cfg .conf`), Objective-C (`.m .mm`) and C# have grammars, and so does Zig (`.zig .zon`). `highlights(of: String) -> [Highlight]` gives UTF-16 ranges and a capture name. Markdown runs the inline grammar over each `inline` node of the block parse; no other injections.
- **One ink table** maps capture prefixes to roles: keyword → ANSI 5, function and method → 4, type and constructor and tag → 3, string and character and `markup.raw` → 6, number and constant and boolean → bright 13, property and `variable.member` → bright 12, comment → `textTertiary`, punctuation and operator → `textSecondary`, `markup.heading` → 4 bold, `markup.italic`/`markup.strong` → font traits, `markup.link` → `linkInk`. Anything unnamed stays `text`. The exact assignment is the implementer's and is seen in Flexoki Dark and Light at demo.
- **One read of each side.** `FileSides { old: TaskFileSnapshot?, new: TaskFileSnapshot? }` per changed path, held by `TaskFilesStore` beside `openedGaps` and dropped with them when the comparison's commits change. The old side reads `--at <baseCommit>` (the old path for a rename; none for an added file); the new side reads `--at <headCommit>` when the comparison has a head, else the working tree (none for a deleted file). It is read when the file nears the view, after its patch, at the same priority. Everything that needs whole-file text takes it from here: highlights, skipped lines and the rendered readers. `openGap` reads `sides.new` instead of the working tree, so `canOpenGaps` goes and step comparisons open skipped lines; Files' Diff of an unsaved draft still reads the draft. A working-tree new side that changed under the patch still fails with "the file changed since this comparison was read".
- **Diff rows.** Each file in the stream gets `FileHighlights { old: [Int: [Span]], new: [Int: [Span]] }`: removed rows look up `old` by old line number, added and context rows `new` by new line number, computed from `FileSides`. Until both sides are read, rows are colored from a parse of the hunk's own lines (new side for context and added, old side for removed). Over 1 MB or a failed read, the hunk parse stays. Opened skipped lines are new-side lines and look up `new` like context rows. A line cut at 1,000 units colors only what it shows.
- **Files editor.** `TaskFileDocument.colorSyntax()` hands the text and its revision to a background highlight and applies the colors only if the buffer is still at that revision. `NSTextStorage.syntaxLimit` goes.
- **Rendered readers** sit in the file's place in the stream. A file that has one (Markdown, SVG, HTML, and any binary whose path ends `.png .jpg .jpeg .gif .webp`) shows Rendered by default and a **Source** toggle in its header; Source is today's line Diff. Images have no source, so no toggle. Choice is per file, in memory, like Viewed. There is no per-file "Side by side": the Diff's layout picker (New code · Unified · Side by side) arranges rendered files as it arranges lines. Markdown reads as a redline in New code and Unified and as two columns in Side by side; images and HTML are two panes in every layout, since a page or picture has no unified form; under 640 pt, where Side by side already falls back to Unified, they stack old above new (chosen for review; passed over: the plan's per-file Rendered · Side by side · Source control, which put a second "Side by side" beside the picker's and let the two disagree).
  - **Markdown:** blocks pair the way lines do. `MarkdownBlocks.blocks` for old and new; blocks of equal kind and text anchor by longest common subsequence (`LongestCommon`). Between two anchors, the old run and the new run align in order as `ReviewPair.align` aligns removed and added lines: a pair of the same kind that `ReviewSegment.edit` accepts is one changed block, its words marked on the block's `AttributedString` (removed words struck in the removed tone, added words in the added tone); anything else stands alone, an old block struck or a new block filled. `MarkdownRedline` is that list of anchored, changed, removed and added blocks; redline shows it in order, Side by side puts old left and new right, aligned by pair. The view reuses `MarkdownBlocks`' block rendering; it doesn't fork it.
  - **Images:** old left, new right, each fitted to half the pane, with `W×H · size` under each. Added or deleted shows one side and "Added"/"Deleted" on the other.
  - **HTML:** two offline `WebPage`s from `TaskFilesView`'s `TaskHTMLView` rules (same navigation decider, no network), each loaded from its side's text, height fixed at the pane's visible height with its own scroll.
- **Files Read** shows a `binary` file whose `data` `NSImage` decodes as an image instead of "Binary file", whatever its extension.

## Absent and error states

- No grammar for the path: plain `text`, as an unknown language is today.
- Old or new read fails: the file keeps hunk coloring; rendered readers show the Source view with the read's error and Retry, as the per-file Diff read does.
- An image whose `data` is null (over 16 MB) or that `NSImage` can't decode: that side reads "Image too large to show · 23.4 MB" (from `size_bytes`) or "Can't decode this image"; the other side still shows.
- Markdown that `MarkdownBlocks` can't parse renders verbatim, as it does in Files.
- A peer Machine's Task: the same served reads through `--machine`; nothing local is assumed.

## Exclusions

- iOS. `Grammar` lives in `LoopflowMac`; the phone has no Diff.
- Injections beyond Markdown inline (code fences, script and style in HTML, SQL in strings).
- Incremental parsing in the editor (`tree.edit`). The background full parse is enough at the sizes measured; revisit if a 1 MB file lags.
- Image swipe, onion skin and difference modes. Question 5 chose plain side by side.
- Structural diffing (explore option D) and colored `lf task diff` in a terminal (option B).
- Line counts and added/deleted kind in `lf task changes` (open since November 21, still a separate protocol change).

## What goes

- `swift/LoopflowMac/SyntaxColors.swift`: `SyntaxLanguage`, its `Kind.color(in:)`, `AttributedString.colorSyntax(of:path:palette:)`, `NSTextStorage.colorSyntax(path:palette:)` and `syntaxLimit`.
- The `SyntaxLanguage` tests in `swift/LoopflowTests/ReviewDiffTests.swift` (around lines 320–345), replaced by `GrammarTests`.
- The "Binary file" placeholder for image paths in the Diff and for decodable binaries in Files Read.
- `TaskFilesStore.canOpenGaps` and the Diff's working-tree-only gap read.
- Must survive: the stream's lazy per-file read and its priority order, word fills on edited lines (grammar color goes under them, as today), cut long lines, every theme passing `readabilityErrors`.

## Acceptance check at gate

Headless, on this Task (`SyntaxColors.swift` is deleted on the branch; `swift/LoopflowMac/dmg-background.png` is unchanged):

```sh
lf task file c9fbcb swift/LoopflowMac/SyntaxColors.swift --at parent --json | jq -r '.state, (.content|length > 0), .read_only_reason'
# text / true / A committed version is read-only.
lf task file c9fbcb swift/LoopflowMac/SyntaxColors.swift --json | jq -r .state      # missing
lf task file c9fbcb swift/LoopflowMac/dmg-background.png --json | jq -r '.state, (.data|length > 0)'   # binary / true
cd swift && swift test --filter 'GrammarTests|MarkdownRedlineTests|TaskFilesTests'   # TaskFilesTests holds the DTO fixture's Swift cases
```

`GrammarTests` asserts, per language, that a fixture line yields the captures a person would see (a Rust `fn` name is `function`, a Swift `struct` name is `type`, a Python docstring row read from the whole file is `string` while its hunk-only parse is not), and that every bundled theme's inks are at 4.5:1 on surface and both row fills. `MarkdownRedlineTests` asserts block pairing and word marks on a heading edit, an inserted paragraph, a deleted list item and a paragraph rewritten past `ReviewSegment.edit`'s threshold (struck and filled, not paired). A store test opens a skipped run in a step comparison and gets the head commit's lines. The DTO fixture tests cover `binary` with `data` in Rust and Swift.

## Demo

Open Task c9fbcb in the Dev build's Diff. `TaskFileDocument.swift` and `ReviewStreamView.swift` show types, calls and properties in color, in Flexoki Dark and Flexoki Light. `scratch/plan.md` opens rendered with its edits redlined; the layout picker's Side by side shows it in two columns, and its Source button shows the line Diff. Pick an `implement` step and open a skipped run. `scratch/explore.html` opens as two live pages. In Files, open `WorkModel.swift`, wrap a few lines in `/* … */` near the top and see them turn comment-colored within a moment while the cursor never stalls. For images, the demo makes a throwaway Task that edits `swift/LoopflowMac/dmg-background.png` and opens its Diff: old and new side by side with sizes.

## Slices

**Built November 21 (slices 1 and 2, `615f7a186c`):** `grammars-build` (21 grammars from 19 pinned sources; local xcframework, 36 MB static archive), SwiftTreeSitter 0.25.0 / tree-sitter 0.25.10 in `Package.swift` and `project.yml`, queries in `swift/LoopflowMac/Grammars/`, `Grammar.swift` with `SyntaxInk` and `SyntaxInks`, Diff rows colored from background hunk parses (`TaskFilesStore.highlights(of:)`), the Files editor parsing off the main thread by `editVersion`, `SyntaxColors.swift` and its test deleted, notices for every grammar plus SwiftTreeSitter and tree-sitter. Both the SwiftPM build and Xcode's `LoopflowMac` link it and copy `Grammars` (Xcode was stopped in its unrelated `lf` bundling step). `GrammarTests` (24) and `theEditorColorsByGrammarAsYouType` pass.

What it taught:
- The queries are written for **the later pattern wins** (toml's `(bare_key) @type` comes before `(pair (bare_key)) @property`; Rust's `@property` before `@function.method`), so `Grammar.highlights` keeps the last pattern for the same text and a language's own query file follows the one it builds on (`c` then `cpp`, `javascript` then `typescript`). Captures without an ink are dropped before precedence, so a broad `@variable` never hides a call. Edited in place: python's attribute pattern moved above its call patterns, toml's pair capture moved onto the key, html colors quoted attribute values.
- An unclosed `/*` in Swift is a parse error, not a comment; the editor turns code comment-colored once the comment closes.

**Built November 21 (slice 3, `f982c27276` and the commit before it):** `lf task file --at <rev>` (read-only, from `git ls-tree -l` and `cat-file`; served `task.file` takes `at`), `data` on `TaskFileSnapshot` in Rust, Swift and `tests/fixtures/dto/task_files.json` (new `image` case), `TaskFilesStore.FileSides` with `readSides`/`sides(of:)`, rows recolored by `FileHighlights.reading(old:new:)` once both sides are read, `openGap` reading `sides.new` (`canOpenGaps` deleted), `ReviewImages` in the Diff for binary `.png .jpg .jpeg .gif .webp`, `TaskImage` in Files Read for any binary that decodes. The gate's three `lf task file` commands answer as written on this Task; `documented_commands::`, `server::schema::tests::` and the DTO fixture pass; `aStepOpensSkippedLinesFromItsHeadCommit` and `aDocstringRowColorsFromTheWholeFile` pass.

What it taught:
- A binary file over the 1 MB text limit used to read as `truncated`; it now reads as `binary` (NUL in its first 1 MB) and carries `data` up to 16 MiB, so a large image is still an image.
- Files' Diff of an unsaved draft keeps hunk colors: the sides are the file on disk, not the draft the rows show.

**Built November 21 (slice 4, `02431969d6`):** `MarkdownRedline` (blocks anchored by `LongestCommon` after trimming shared ends, runs between anchors paired in order; a same-kind pair `ReviewWords.edit` accepts is one changed block), `ReviewWords` (one ordered word edit, removed before added; `ReviewSegment.edit` derives from it), `MarkdownBlocks.Row` shared by Files and the redline, `ReviewRendering`/`ReviewRendered` in `ReviewRenderedView.swift` (Markdown as redline or two columns by the layout picker; SVG through `TaskImage` from its text; HTML as two `TaskHTMLView`s at the pane's height), **Source**/**Rendered** in the file header (`TaskFilesStore.sourceFiles`), a failed side read falling back to the line Diff with Retry. `MarkdownRedlineTests` (heading edit, inserted paragraph, deleted list item with renumbering, rewrite past the threshold, inline style kept, added document) pass with `GrammarTests` and `TaskFilesTests`.

**Built November 21 (slice 5, steered: `b5037958e5`, `c3815a9b16`):** the stepper no longer shows a count that climbs while files read ("4/12 files", "2 of …"; `theChangeCountWaitsForEveryFile`); Dockerfile, Make, just, INI, Objective-C, C# and Zig grammars, pinned with notices, queries in `Grammars/` (28 grammars, 27 languages; `GrammarTests` 27 fixtures pass; `test_third_party.py` passes).

Slice 5 also reworked the scrollbar marks; the parent branch then deleted them (merged in `bc192e4194`), so none remain. The 28-grammar archive is hosted: `TreeSitterGrammars-f9a0593.xcframework.zip` on bin.loopflow.studio hashes to `Package.swift`'s checksum (checked November 21, after the merge, when `GrammarTests` and `MarkdownRedlineTests` passed again).

Checks left for gate: `documented_commands::` (swift/README.md's language list changed), CI's Swift build.

QA November 21 (`scratch/qa.html`, `ce178ae48f`): the redline, Side by side, HTML, grammar colors and Files images work in the Dev build; image diffs stacked for any picture wider than half the pane (fixed); the notices inventory was stale (refreshed). Not reached without clicks: the Source and layout buttons, a step's skipped lines. Open product questions are in `questions.md`, From qa.

Remaining: nothing to build; the rendered-file stop repair below is built. Demo checks for qa: `scratch/plan.md` rendered and redlined in the Dev build, Side by side in two columns, Source showing the line Diff; `scratch/explore.html` as two pages; a step's skipped lines.

## Concept review after the build (November 21, headless)

Page: `scratch/concept-review.html`, over the whole branch as built. Every concept lines up except one, and it is what the qa run's first question touched: **a "change" is still a line hunk, but a rendered file doesn't show its hunks.** The title, the stepper and next/previous count `ReviewDiff.stops`, which come from the patch alone. So:

- A changed `swift/README.md` reads as a redline, yet counts its line hunks ("3 of 41") and `]` lands on stop ids the rendered view never draws (`ReviewRendered` carries only `file.topID`), so `scrollPosition(id:)` doesn't move: the count climbs while the pane stands still.
- An image-only change has no hunks: "0 changes in 1 file", "No changes", and its header shows "+0 −0".

**Chosen for review: a rendered file is one change.** The stream's stops come from what it shows: a file the store renders (`rendering(of:in:)` non-nil) is exactly one stop, `"\(path)#rendered"`, and `ReviewRendered` (and `ReviewSidesFailed`, which falls back to lines only for Source-capable files) carries that id; with Source on, its line hunks are the stops again. Passed over: one stop per changed redline block, which needs both sides read before the count is known and would make the count jump as files near the view, the thing `theChangeCountWaitsForEveryFile` just removed. The redline is read as a document; one stop lands on its top.

Across layers: Swift only. `ReviewDiff.stops` / `ReviewFile.stops` stay the patch's; the store gains the one place that turns a shown diff into stops (`stops(_ reader:)`, rendered files collapsed to one) and `position`, `step`, `TaskChangesView.title` and the focus/sidebar lookups read it instead of `diff.stops`/`shownReview.stops`. A binary file's header drops "+0 −0" (it has no lines to count). No Rust, wire or fixture change.

Built: `TaskFilesStore.stops(of:in:)` / `stops(_:)` and `topID(of:in:)`; `ReviewStop.rendered(path)`; stepping moved to `[ReviewStop].stop(after:offset:)`; binary headers and sidebar rows drop their counts. The title's count is checked through `position` (the title is a private view property). After merging the parent's one-patch counts (`31fc852dca`), the title and `position` sum `changeCount`, which returns 1 for a rendered file and the parent's up-front count otherwise. Check: `scripts/test_desktop.sh --filter 'TaskFilesTests|ReviewDiffTests'` passed after that merge (65 tests); qa checks `]` on a rendered README in the Dev build.

Test (`TaskFilesTests/aRenderedFileIsOneChange`): a stream of one Swift file with two changes, a changed `.md` with three hunks and a changed `.png` counts "4 changes in 3 files"; stepping visits the Swift stops, then `doc.md#rendered`, then `logo.png#rendered`; turning on the `.md`'s Source makes it 2 + 3 + 1 = 6. Break the feature in your head (stops from the patch again) and the count reads 5, and the image is never visited.

## Docs

Written ahead of the build: `swift/README.md` (Diff: grammar colors, skipped lines in step comparisons, rendered Markdown, image and HTML diffs and **Source**; Files Read shows images) and `docs/workspaces.md` (`lf task file ENG-123 README.md --at parent`). Slice 3's flag makes that example pass `documented_commands`; slice 4 built what the README's rendered-Markdown, HTML and **Source** lines describe, and slice 5 added the new languages to its grammar list.
