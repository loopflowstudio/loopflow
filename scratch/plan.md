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

## What gets built

- `Grammar` (SwiftTreeSitter plus one SPM package per grammar and its `highlights.scm`) replaces `swift/LoopflowMac/SyntaxColors.swift`. One table maps capture names to inks taken from the theme's ANSI palette. Red and green stay with removed and added lines.
- Diff rows are colored from whole-file parses of the old and new text, indexed by line number. Until the full text arrives, a row is colored from a parse of its hunk alone. Parsing is lazy, for files near the view.
- The Files editor recolors through the same `Grammar`.
- New served read `lf task file <issue> <path> --base <rev>`, which gives the old side as text, or as bytes for images. Highlighting and the rendered diffs both use it.
- Markdown: old and new are parsed into `MarkdownBlocks` blocks and matched in order. Changed blocks get word marks. The default is redline, side by side is a toggle, and source is one click away.
- Images (PNG, JPEG, GIF, WebP, SVG): old and new side by side, with their sizes in the header. This replaces "Binary file · no text to compare".
- HTML: two offline `WebPage`s side by side, with the same offline rules as Files.

## Open

- Whether swipe, onion skin and difference ship as image modes: question 5 chose plain side by side.
