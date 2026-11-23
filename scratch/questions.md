# Open questions (defaults used in design.md unless steered)

- **Open files when sidebar is collapsed (Round 3 Q2):** Using the clickable breadcrumb filename dropdown plus open files at the top of `⌘P` and the sidebar (no extra file tab row). Alternative: show a compact tab strip under the breadcrumb when 2+ files are open.
- **Auto-scratch replacement (Round 3 Q3):** Using empty-pane initial pick + quiet update bar/dot on passive FSEvents writes (debounced ~800ms for HTML reloads), while explicit `lf open --file` still opens the requested file. Alternative: an opt-in `Follow` toggle in the header.
- **Tree-sitter symbol index scope:** Kickoff chose a declaration index on the owning Machine and the same declaration queries over the open buffer on the Mac. Both directions land in one PR, built in vertical slices. The alternatives were indexing remote files on the Mac and stacking two PRs.

- **Edit shortcut:** Kickoff chose ⌃⌘E for the focused Files pane. ⇧⌘E already toggles the browser.
