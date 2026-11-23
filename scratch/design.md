# LOO-1067 · Redesign the file browser

**Prototype:** [explore.html](explore.html) (Round 3)
**Research notes:** [research.md](research.md)

## Exact input recorded

- **Jack Heart (2026-11-22):** "lets also research what t3code, paseo, orchestrator inc, rune, cursor etc do to make nice agent-native file browsing experiences. and lets see what makes sense to move from the diff experience eg the comments, the sidebar expereince. lets also look at what were trying to do with teh auto scratch seelction and figure out alternatives to that because its not working well right now, leads to a lot of random flashing. also the nav/tabs is super awkward."
- **Jack Heart (2026-11-22):** "lets also look at integrating tools that command line geeks like like fzf / or jump to definitio / i think i want something between a and b, not any of the other layous, but show me another round of exploring leanign with that more basic experience but showing how we could lean into different directions based on other expereinces elsewehre / i also noticed taht cmd f seems to not be captured by the files browser and always goes to the main session"
- **Jack Heart (2026-11-22):** "lets do directions 1 and 2"
- **Jack Heart (2026-11-22):** "4a i think --read only by defautl / easy to turn on editable"

## Chosen direction

Build the **A↔B base layout** layered with **Direction 1 (Telescope / `fzf --preview` + `ripgrep`)** and **Direction 2 (Tree-sitter Jump-to-Definition, Outline & References)**, while fixing the `⌘F` focus/find bug, moving `Diff`'s sidebar and inline comment mechanics into `Files`, and replacing auto-scratch hijacking with calm update signals.

### 1. Fixed A↔B Base Layout & Nav

- **Keep `Brief | Diff | Files` in the pane strip** (`WorkspacePanesView`). No Miller columns, no second stacked tab bar, no hidden `1 of 3 ▾` file menu, and no static `This Mac / path` location row.
- **One compact header bar (32pt)** above the document:
  - Sidebar toggle (`sidebar.left`), back/forward jump buttons (`⌘[` / `⌘]`), and an **interactive path breadcrumb** (clicking any folder segment opens a sibling menu; clicking the filename switches open/recent files; extends with the enclosing symbol when reading code, e.g. `Views / TaskFilesView.swift › filesPage`).
  - Right controls (`ViewThatFits` adaptive like `TaskChangesView`): `⌘F` (in-file find), `⌘P` (`fzf` files), `⌘⇧F` (`rg` grep), `@` symbol outline (`⌘⇧O`), `Read / Edit` switch, and `Diff` toggle when the file has changes.
- **Responsive 236pt left rail** sharing `TaskChangesView`'s `dockedWidth = 620` breakpoint:
  - Auto-docked when wide; slides over cleanly as a drawer when narrow (and dismisses on file pick); single `sidebar.left` toggle deletes the `.closed / .dropdown / .pinned` pin dance.
  - Three sections in one rail: **Open & Scratch** (with unsaved and calm agent-update dots), **Changed in Task** (`+N −M`, matching `Diff`), and **Checkout** (lazy tree with single-child folder compaction, auto-reveal of the active file, and a toggle for `show_ignored` already supported by `task.files`).

### 2. Fixed `⌘F` Focus & In-File Find

- Attach `PaneFocusTarget(isFocused: isFocused)` to `TaskFilesView` in `WorkspacePanesView.swift` (matching `.graphLog`) so clicking in rendered Markdown, HTML (`WebView`), Diff, or the file tree holds AppKit `firstResponder` off `GhosttyTerminalNSView`.
- Capture `⌘F` inside `TaskFilesView` (when `isFocused`) with a unified inline find bar beneath the header across `Read` code, `Edit` (`NSTextView`), rendered Markdown, rendered HTML (`WebView`), and `Diff`, with match counter, `Enter`/`⇧Enter` stepping, and a one-click `in repo →` button that promotes the query to repo-wide `rg` (`⌘⇧F`).

### 3. Shared `Diff` Experience in `Files`

- **Line-level comments & `ReviewCommentsStrip`:** In `Read` mode on code files, render syntax-colored, line-numbered code with subtle change-gutter bars and clickable line numbers (matching `ReviewStream`). Clicking or Shift-clicking lines opens the same `ReviewCommentEditor` (`Comment` / `Copy` / `Reference in conversation` / `Cancel`), pooling into the shared `TaskFilesStore.comments` queue and `ReviewCommentsStrip` (`⌘↩ Send to conversation`).
- **`Read` vs `Edit` becomes meaningful on every file (confirmed 4a):** `Files` opens **read-only by default** (`Read` shows rendered Markdown/HTML or the line-numbered, commentable code reader), with an **easy, one-click + shortcut toggle** into `Edit` (`NSTextView`, retaining 3-way merge, autosave, and recovery untouched).

### 4. Calm Scratch Updates (Replaces Auto-Scratch Flashing)

- Delete the silent `autoSelected` ping-pong loop and 120-second timer hijack (`TaskFileDocument.swift:423-467`).
- Empty pane picks the latest `scratch/` artifact once on initial load.
- Once any file or tab (`Brief` / `Diff` / `Files`) is open, passive FSEvents writes under `scratch/` never steal focus or switch pages: they badge the file in the sidebar and show a quiet one-line bar (`Agent updated scratch/explore.html · Open`).
- If the person is already viewing that file, reload in place only after writes settle (debounced ~800ms for HTML `WebPage` to stop mid-turn white flashes) while preserving scroll position.

### 5. Direction 1: Telescope / `fzf --preview` + `ripgrep`

- One two-column fuzzy modal with live syntax-colored preview on the right (collapsing cleanly to single-column at narrow widths), with three switchable modes:
  - **Files (`⌘P`):** fuzzy path matching across the entire checkout (`git ls-files` + untracked + `scratch/`), ranked by scratch → changed → open → fuzzy score.
  - **Grep (`⌘⇧F`):** live content search across the checkout, previewing the file centered on the matched line.
  - **Changed:** instant filter over files modified in this Task.
- **Rust RPCs (local + remote Machine safe):** Add `task.files.index` and `task.files.grep` in `loopflow-execution` / `read.rs` using pure-Rust `ignore` + `grep-searcher` / `grep-regex` crates (bounded result limits, respects `.gitignore` with `scratch/` included, zero external binary dependencies on remote Machines).

### 6. Direction 2: Tree-sitter Jump-to-Definition, Outline & References

- Build on the 28 Tree-sitter grammars already bundled in `Grammar.swift` / `TreeSitterGrammars.xcframework` — zero LSP startup cost per fresh Task worktree.
- **Jump to definition (`⌘-click` / `gd`) + Jump stack (`⌘[` / `⌘]`):**
  - Single match jumps immediately and pushes the prior location onto the jump stack.
  - Multiple matches open an inline peek list beneath the line so you pick without losing scroll position.
- **File symbol outline (`⌘⇧O` / `@ Symbols`):** Lists structs, classes, enums, functions, methods, and Markdown headings in the open file and updates the sticky breadcrumb (`file › symbol`).
- **Find references (`gr` / inline peek):** Combines symbol query + `task.files.grep` to list call sites inline.
- **Backend/index:** Lightweight `tags.scm` / declaration queries for core languages (Swift, Rust, TypeScript/JavaScript, Python, Go, Markdown) with a fast regex fallback for other languages; worktree symbol index cached in Rust (or via `task.files.symbols` RPC) so remote checkouts work identically.

### Visual & Codebase Constraints

- **Design system (`DESIGN.md` & `DesignDisciplineTests`):** All Swift views use named tokens (`Typography.*`, `Spacing.*`, `CornerRadius.*`, `LoopflowPalette`, `LoopflowButtonStyle`, `EmptyState`, `FailureLine`). No raw sizes, no `ContentUnavailableView`.
- **DTOs:** Any new Rust/Swift wire types (`task.files.index`, `task.files.grep`, `task.files.symbols`) have no `#[serde(default)]`, no Swift init defaults, and round-trip fixtures under `tests/fixtures/dto/`.
- **Delete what is replaced:** Remove the `.closed / .dropdown / .pinned` navigator state machine, `documentRow`'s `1 of 3 ▾` menu, the static `location` row, and the 120s `autoSelected` hijack logic once replaced.
- **Preserve existing strengths:** Keep local FSEvents and remote `watchTaskFiles` streaming, 3-way text merge (`TaskTextEdit`), autosave recovery, tree-sitter coloring, offline `TaskHTMLView`, and image viewing.

## Open Threads (not explicitly answered in Round 3)

Questions 2–3 from [explore.html](explore.html) were not specified separately. Defaults applied above unless steered otherwise (mirrored in [questions.md](questions.md)):

- **Open files when sidebar is collapsed (Q2):** Defaulting to `2a` + `2b` (clickable breadcrumb filename menu for open/recent files, plus open files at the top of `⌘P` and the sidebar; no extra tab row).
- **Auto-scratch replacement (Q3):** Defaulting to `3a` + explicit `lf desktop files PATH[:LINE[-END]] [--mode read|diff]` (auto-pick only when empty, quiet update bar/dot on passive FSEvents writes, ~800ms settle debounce for HTML reloads).
