# c9fbcb Diff v2: grammar highlighting and rendered diffs

Stacked on LOO-515 (#1949). Options page: `scratch/explore.html` (round 1).

## Decisions

Jack Heart, November 21, answering the explore page: "2a 2a 3a 4c 5a 6a 7b", then "1a".

1. Engine: **A**, native tree-sitter in the app, coloring both the Diff and the Files editor.
2. Theme colors: **a**, derived from each theme's terminal palette (ANSI 1–6), so custom themes work.
3. A changed Markdown file opens **rendered**, with source one click away.
4. Markdown marks: **redline by default**, with side by side one click away.
5. A changed image opens **side by side**.
6. A changed HTML file opens as **two live pages side by side**.
7. First languages: **this repository's** (Swift, Rust, Python, TypeScript/TSX, JavaScript, Markdown, YAML, TOML, JSON, SQL, shell, HTML, CSS) **plus Go, C/C++, Ruby, Java and Kotlin**.

## Done when

- A Task's Diff colors keywords, types, functions, properties, strings, numbers and comments by grammar in all 18 languages, in every bundled theme, at 4.5:1 or better on the row it sits on. A hunk that opens inside a Python docstring colors as a docstring once the file's text is read.
- The Files editor colors through the same grammar while you type, with no size limit of its own (the 1 MB read cap still applies).
- A changed `.md` opens rendered, redlined; one click shows old and new side by side, one more shows source.
- A changed PNG, JPEG, GIF, WebP or SVG shows old and new side by side with pixel sizes and byte sizes. "Binary file · no text to compare" no longer appears for images.
- A changed `.html` shows the old and new page live, side by side, offline.

## Findings that shape it

**Grammar packages can't come from SPM.** Probed November 21 in `/tmp/tsprobe` (Swift 6.2.3): all 19 grammar repositories have a `Package.swift` and resolve together beside SwiftTreeSitter 0.25.0 (5 min 20 s cold). But five of them (JavaScript, Python, CSS, YAML, Go) add `scanner.c` only `if FileManager.default.fileExists(atPath: "src/scanner.c")`, which is false under SwiftPM's manifest sandbox, so JavaScript, Python, CSS and YAML fail to link. `DerekStride/tree-sitter-sql` commits no `parser.c` at all; its GitHub release tarball has one. Swift needs the `0.7.4-with-generated-files` tag.

**Vendoring the C is too big to commit.** The generated parsers total ~134 MB of C (Kotlin 23.6 MB, Swift 23.1 MB, TypeScript 17.5 MB, C++ 17.3 MB, Ruby 15.3 MB). Compiled `-Os` for arm64 they come to ~22 MB of code and data. The app is arm64-only and 129 MB today, so it grows about 17%, not the 8–15 MB the explore page guessed.

**So the grammars ship like GhosttyKit:** one prebuilt `TreeSitterGrammars.xcframework` as a SwiftPM `binaryTarget` with URL and checksum, built by `uv run python scripts/loopflow-dev.py grammars-build` from sources pinned by URL and SHA-256 in `third_party/manifest.json` (release tarballs or tag archives that contain `parser.c`). It exposes one header, `const TSLanguage *tree_sitter_<name>(void)` per language. The tree-sitter runtime and SwiftTreeSitter come from SwiftPM (`ChimeHQ/SwiftTreeSitter`, which depends on `tree-sitter/tree-sitter` 0.25; both built cleanly in the probe). Grammar ABIs are 14 and 15; runtime 0.25 accepts both. Wire both into `swift/Package.swift` and `swift/project.yml`. **Needs a yes:** uploading a new artifact to the `bin` R2 bucket is covered by the GhosttyKit authorization only for GhosttyKit (`swift/GhosttyKitPatches/README.md`). Until then the branch builds against `swift/.build/local/TreeSitterGrammars.xcframework` and CI can't link it.

**Queries are ours, committed.** Upstream `highlights.scm` files inherit: TypeScript's (515 bytes) assumes JavaScript's is prepended, C++'s assumes C's (C++'s alone fails to compile against its grammar: "nodeType(43)"). Markdown is two grammars, block and inline. Queries live in `swift/LoopflowMac/Grammars/<language>/highlights.scm`, copied from the pinned source by `grammars-build --queries` and edited in place, so capture names can be normalized and reviewed. Each `Grammar` names the query files it concatenates.

**Speed.** `swift/LoopflowMac/WorkModel.swift` (3,649 lines, 185k UTF-16 units): parse 21 ms, query and resolve 129 ms, 37,801 captures, release build. A 20-line Rust or Swift snippet takes under 0.5 ms. So highlighting runs off the main thread everywhere, and the editor applies colors when they arrive for the text revision they were computed from.

**Theme inks.** Of the 24 bundled themes, 4–6 have each of ANSI 3, 4, 5 and 6 under 4.5:1 on their background (worst: Noctis Lilac 1.85:1, Gruvbox Light 2.19:1, Catppuccin Latte 2.31:1). Each ink is the theme's ANSI color mixed toward `text` just far enough to reach 4.5:1 against the surface and the added and removed row fills (`ThemeRGB.mixed`, `contrast`). Red (1) and green (2) stay with removed and added lines.

**Images and SVG.** `NSImage(data:)` reads SVG (`_NSSVGImageRep`), PNG, JPEG, GIF and WebP; no decoder needed. Served answers spill to bulk up to 32 MB (`MAX_BULK`, `rust/loopflow-execution/src/machine/wire.rs`), so an image of up to 16 MB fits as base64.

## Source of truth

The Task checkout's Git objects and working tree, read through `lf task file`. The app holds no file contents of its own beyond the reads it caches.

### `lf task file <issue> <path> [--at <rev>]`

- `--at` takes what `lf task diff --base` takes (`parent`, `head`, or a resolved 40-hex SHA, through `resolve_file_base`) and reads the path from that commit with `git cat-file blob <sha>:<path>`. Absent, it reads the working tree as today. A path missing at that commit is `missing`. A read at a commit carries `read_only_reason: "A committed version is read-only."` and no recoveries. Named `--at` rather than `--base` because it names the version read, not a comparison (chosen for review).
- `TaskFileState` gains `image`. A path ending `.png .jpg .jpeg .gif .webp` that reads as binary is `image` up to 16 MB (`MAX_IMAGE_BYTES`), with its bytes in a new required nullable field `data` (base64). Every other state has `data: null`. SVG stays `text`; the app renders its content.
- Rust `TaskFileSnapshot`/`TaskFileState` in `rust/loopflow-execution/src/ops/task.rs`, reader in `ops/task/snapshot.rs`; served `task.file` gains `at`; Swift `TaskFileSnapshot`/`TaskFileState` in `swift/Loopflow/Models/TaskWorkspace.swift` and `RegistryQuery.taskFile(issue:path:at:)`; the DTO fixture under `tests/fixtures/dto/` and each language's fixture test gain `data` and an `image` case.

## In the app

- **`Grammar`** (`swift/LoopflowMac/Grammar.swift`) replaces `SyntaxColors.swift`. `Grammar.named(path)` picks by extension or file name, as `SyntaxLanguage.named` does today, adding `.kts`. Today's `hashed` files (`Dockerfile`, `Makefile`, `justfile`, `.ini`, `.cfg`, `.conf`), Objective-C (`.m`, `.mm`) and C# (`.cs`) have no grammar in the chosen list and go uncolored (chosen for review; each is one manifest entry later). `highlights(of: String) -> [Highlight]` gives UTF-16 ranges and a capture name. Markdown runs the inline grammar over each `inline` node of the block parse; no other injections.
- **One ink table** maps capture prefixes to roles: keyword → ANSI 5, function and method → 4, type and constructor and tag → 3, string and character and `markup.raw` → 6, number and constant and boolean → bright 13, property and `variable.member` → bright 12, comment → `textTertiary`, punctuation and operator → `textSecondary`, `markup.heading` → 4 bold, `markup.italic`/`markup.strong` → font traits, `markup.link` → `linkInk`. Anything unnamed stays `text`. The exact assignment is the implementer's and is seen in Flexoki Dark and Light at demo.
- **Diff rows.** Each file in the stream gets `FileHighlights { old: [Int: [Span]], new: [Int: [Span]] }`: removed rows look up `old` by old line number, added and context rows `new` by new line number. While the full texts are being read, rows are colored from a parse of the hunk's own lines (new side for context and added, old side for removed). The full read runs when the file nears the view, the same priority as the per-file Diff read: `--at <baseCommit>` for the old side (the old path for a rename), `--at <headCommit>` or the working tree for the new. No old side for an added file, no new side for a deleted one. Over 1 MB or a failed read, the hunk parse stays. A line cut at 1,000 units colors only what it shows.
- **Files editor.** `TaskFileDocument.colorSyntax()` hands the text and its revision to a background highlight and applies the colors only if the buffer is still at that revision. `NSTextStorage.syntaxLimit` goes.
- **Rendered readers** sit in the file's place in the stream with a three-way control in its header (Rendered · Side by side · Source for Markdown; Side by side · Source for SVG and HTML; images have no source). Source is today's line Diff. Choice is per file, in memory, like Viewed.
  - **Markdown:** `MarkdownBlocks.blocks` for old and new, matched in order by longest common subsequence over each block's plain text. A matched pair whose text differs is a changed block; its words are marked with `ReviewSegment` applied to the block's `AttributedString` (removed words struck in the removed tone, added words in the added tone). Unmatched old blocks show struck, unmatched new blocks filled. Side by side puts old blocks left and new right, aligned by match. The view reuses `MarkdownBlocks`' block rendering; it doesn't fork it.
  - **Images:** old left, new right, each fitted to half the pane, with `W×H · size` under each. Added or deleted shows one side and "Added"/"Deleted" on the other.
  - **HTML:** two offline `WebPage`s from `TaskFilesView`'s `TaskHTMLView` rules (same navigation decider, no network), each loaded from its side's text, height fixed at the pane's visible height with its own scroll.
- **Files Read** shows an `image` file as an image instead of "Binary file". It costs one case in the switch `TaskFileState.image` forces anyway.

## Absent and error states

- No grammar for the path: plain `text`, as an unknown language is today.
- Old or new read fails: the file keeps hunk coloring; rendered readers show the Source view with the read's error and Retry, as the per-file Diff read does.
- An image over 16 MB or that `NSImage` can't decode: that side reads "Image too large to show · 23.4 MB" or "Can't decode this image"; the other side still shows.
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
- The "Binary file" placeholder for image paths in the Diff and in Files Read.
- Must survive: the stream's lazy per-file read and its priority order, word fills on edited lines (grammar color goes under them, as today), cut long lines, every theme passing `readabilityErrors`.

## Acceptance check at gate

Headless, on this Task (`SyntaxColors.swift` is deleted on the branch; `swift/LoopflowMac/dmg-background.png` is unchanged):

```sh
lf task file c9fbcb swift/LoopflowMac/SyntaxColors.swift --at parent --json | jq -r '.state, (.content|length > 0), .read_only_reason'
# text / true / A committed version is read-only.
lf task file c9fbcb swift/LoopflowMac/SyntaxColors.swift --json | jq -r .state      # missing
lf task file c9fbcb swift/LoopflowMac/dmg-background.png --json | jq -r '.state, (.data|length > 0)'   # image / true
cd swift && swift test --filter 'GrammarTests|MarkdownRedlineTests|TaskFileSnapshot'
```

`GrammarTests` asserts, per language, that a fixture line yields the captures a person would see (a Rust `fn` name is `function`, a Swift `struct` name is `type`, a Python docstring row read from the whole file is `string` while its hunk-only parse is not), and that every bundled theme's inks are at 4.5:1 on surface and both row fills. `MarkdownRedlineTests` asserts block matching and word marks on a heading edit, an inserted paragraph and a deleted list item. The DTO fixture tests cover `image` and `data` in Rust and Swift.

## Demo

Open Task c9fbcb in the Dev build's Diff. `TaskFileDocument.swift` and `ReviewStreamView.swift` show types, calls and properties in color, in Flexoki Dark and Flexoki Light. `scratch/plan.md` opens rendered with its edits redlined; the header switches to side by side and to source. `scratch/explore.html` opens as two live pages. In Files, open `WorkModel.swift`, type `/*` near the top and see the rest of the file turn comment-colored within a moment while the cursor never stalls. For images, the demo makes a throwaway Task that edits `swift/LoopflowMac/dmg-background.png` and opens its Diff: old and new side by side with sizes.

## Slices

1. **Grammar artifact to colored Diff rows.** `grammars-build` with the manifest entries for all 18 languages, the local xcframework, SwiftTreeSitter in `Package.swift` and `project.yml`, committed queries, `Grammar` with the ink table, Diff rows colored by hunk parse. Shows whether the artifact links in both the SwiftPM and Xcode builds, whether every query compiles against its grammar, and whether the inks read in the low-contrast themes. This is the slice that could prove the plan wrong. Then ask for the R2 upload and switch `Package.swift` to the hosted URL.
2. **Editor and deletion.** Files editor on `Grammar` in the background; `SyntaxColors.swift` and its tests go; notices for SwiftTreeSitter, tree-sitter and each grammar through `scripts/third_party.py refresh`.
3. **`lf task file --at` and `image`.** Rust, served read, both DTOs and fixtures; Diff rows recolored from whole-file parses; images side by side in the Diff and shown in Files Read.
4. **Markdown redline, then HTML side by side.**
