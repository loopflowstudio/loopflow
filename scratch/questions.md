# Open questions & choices made for review (see scratch/concept-review.html)

- **Open files when sidebar is collapsed (Round 3 Q2):** Using the clickable breadcrumb filename dropdown plus open files at the top of `⌘P` and the Open & Scratch rail section (MRU-capped for clean auto-opened files; no extra tab strip). Alternative: show a compact tab strip under the breadcrumb when 2+ files are open.
- **Presentation.** Jack Heart: “i feel like it should be lf desktop though or soemthing / lf open can open things in rex so it seems like a weird hybrid api if it also selcts panes in the loopflow desktop”. The plan places presentation on `lf desktop files`. Passive writes refresh the list, mark updated paths and settle a visible HTML reload after 800 ms. Explicit presentation offers a quiet bar during active Edit or Diff review.
- **Symbol index architecture (Concept Review Q1):** Chosen **1a** — one Tree-sitter runtime (Desktop's existing 28-grammar `TreeSitterGrammars.xcframework`) for open-file outline, enclosing breadcrumb and live same-file definitions; pure-Rust `grep-searcher` pre-filter + comment/string-aware declaration matcher on the Machine for cross-file `task.files.symbols` (keeps `lf` lean for ARM self-hash and non-incremental builds; clean wire boundary if AST is ever added later). Alternative **1b**: compile 7–8 C/C++ Tree-sitter grammar crates into Rust `lf`.
- **CLI search subcommands (Concept Review Q2):** Chosen **2a** — peer verbs `lf task find`, `lf task grep`, `lf task symbols` (one verb, one JSON schema). Alternative **2b**: overload `lf task files` with `--index` / `--grep` / `--symbols`.
- **Read / Edit / Diff scope (Concept Review Q3):** Chosen **3a** — per-file view mode (Read by default, draft visible in Read, lazy `NSTextView` on Edit, Files' Diff steps within that file only). Alternative **3b**: single global mode resetting on every file switch.
- **Jump history keys (Concept Review Q4):** Chosen **4a** — register `files-back`/`files-forward` (`⌘[`/`⌘]`) in `DesktopAction`, falling through to Desktop window Back/Forward when Files' jump stack is empty. Alternative **4b**: bind Files jump history to `⌃-`/`⌃⇧-`.
- **Edit shortcut:** Kickoff chose `⌃⌘E` for the focused Files pane (`⇧⌘E` already toggles the browser).

- **Implementation order.** Build Read and per-file Edit first so the checkout picker and its preview can share the code reader. The existing Task file read already supplies content from the owning Machine.

- **Checkout inventory cache.** Recheck an idle checkout on the next request after Git’s three-second observation interval. Keep up to 100,000 path entries across idle inventories, most recently used first. A live reading keeps its inventory until it finishes.

- **Opening a grep hit.** Return opens the numbered source even for Markdown and HTML, since a rendered page has no source-line coordinate. Selecting the file normally opens its rendered view. Disk byte ranges highlight only when the reader holds that revision.

- **Definition lookup order.** Prefer exact declarations in the live source file. When it has none, search the owning checkout. Closing the peek leaves history untouched. The source reader uses the same syntax tree for outline entries and coloring.

- The Open bar shows the most recent offered path. Further presentations of that path replace its previous line range. Other pending paths stay available in Open & Scratch.
- Line totals for untracked text use the reader’s 1,000,000-byte ceiling. Larger untracked files retain their change stop and omit the line total.

- In-file Find uses case-insensitive literal text. Promoting the query to Grep keeps Find open underneath so Escape returns to the same match. A pure deletion marks the surviving line above it, or line one at the start of the file.

- The Swift journey simulates SSH by forwarding addressed commands to a private Machine. Its checkout path on the Desktop side is absent. The Rust Machine fixture covers SSH dispatch.

- Keep the existing `lf task changes --json` scratch fields for CLI consumers. Desktop gets its scratch list independently through `task.files.find`.
- **Removing `lf open --file` (Post-build Concept Review Q1):** Chosen **1a** — delete `lf open --file` (`open/files.rs`, `desktop::present_files`) now that all builtin skills and docs use `lf desktop files`; keep `lf open` purely for cross-host conversation opening. Alternative **1b**: keep `lf open --file` as a compatibility wrapper over `lf desktop files`.
- **Presenting from a zoomed Session (Post-build Concept Review Q2):** Chosen **2a** — `lf desktop files` (via `MultiplexerStore.showBrowser(..., focus: false)`) unzooms another zoomed pane while keeping `focusedPaneId` on the terminal so the presented page appears beside the conversation without stealing focus. Alternative **2b**: keep Session zoomed and leave the presentation hidden until the user unzooms manually.
- **Notice banner placement & dismissal (Post-build Concept Review Q3):** Chosen **3a** — separate persistent Open & Scratch rail badges from an ephemeral dismissible (`×`) layout banner shown in Files (and in Diff for deferred presentations), never raised for the file already visible in Files Read. Alternative **3b**: keep the bottom overlay bar without dismissal.

- A scratch write uses the owning Machine’s current directory listing to check which names survived an atomic save. The checkout-wide picker inventory can lag this event by three seconds.

- An explicit presentation keeps its banner through passive writes. Later writes still mark their files in Open & Scratch. Grep returns the byte offset of its cropped excerpt so Desktop can distinguish repeated occurrences after a Unicode prefix.

- Choosing a pending presentation by filename accepts its latest line range and mode with either a click or Return in the navigator. Back returns to the prior reading line.

## Sync onto main (#2040 remote Task workspace) & Post-Demo Concept Review, 2026-11-24–25

- **Code Read implementation (Post-Demo Concept Review Q1):** Chosen **1a** — replace the per-line SwiftUI `LazyVStack` in `TaskCodeReader` with one read-only TextKit 2 `NSTextView` + pinned line-number/change-mark gutter, direct `mouseDown` for `⌘-click` & word underlines, immediate plain-text display with async Tree-sitter coloring/outline, and `paragraphSpacing` inline slots for comments & symbol peek (keeping `TaskFileDocument.editor` separate for Edit mode). Fixes broken multi-line drag-select, deletes the untestable `RunLoop.default` `⌘-click` hack, pins line numbers during sideways scroll, and removes 3–6 s open / 271–600 ms jump stalls on large files. Alternative **1b**: keep the per-line SwiftUI `LazyVStack`.
- **Peer scratch observation over `task.files.watch` (Post-Demo Concept Review Q2):** Chosen **2a** — add `scratch_pages: Vec<TaskScratchPage>` (`{ path, stamp }` from mtime + size) to `TaskFilesRevision` and keep `revision` strictly for working-tree/Git changes, so peer scratch writes badge Open & Scratch, raise `Updated · Open ×`, survive reconnects, drop vanished temp files, and skip redundant `git diff` reloads over SSH. Alternative **2b**: leave `TaskFilesRevision` as a single UUID with no peer `Updated` notice and full rescans on scratch writes.
- **Jump landing highlight & Back to rendered Markdown/HTML (Post-Demo Concept Review Q3):** Chosen **3a** — mark the destination line on definition, outline, and Back/Forward source jumps (cleared on clicking another line), and store rendered vs. source explicitly on `FilePosition` / `FileVisit` so `⌘[` Back to a rendered Markdown/HTML page stays rendered. Alternative **3b**: leave definition/outline/Back landings unmarked and open Markdown/HTML as raw source whenever a `FileVisit` is present.
- **In-file Find across file switches (Post-Demo Concept Review Q4):** Chosen **4a** — keep the Find bar and query open across file switches (re-scanning the new file until closed with `Esc`/`×`). Alternative **4b**: close or clear Find whenever selection changes.
- The QA hook's branch `key <window> <mods> <chars>` is now `windowkey`; main's `key post|send …` keeps the name.
