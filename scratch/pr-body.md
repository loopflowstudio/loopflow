Wave memory could come from the origin checkout while a Task worked on a different copy. This change gathers the selected Wave's Markdown and its ancestors' files from the executing checkout, so local decisions travel with the work.

## What changes

- Gather top-level Wave Markdown root first, including GOAL.md, MEMORY.md and notes. Ordinary context excludes children, siblings and unrelated Waves; scratch remains recursive.
- Use one Wave renderer for assembled prompts and native skill seeds. Remove the separate registry-backed memory reader and preassembled memory input while preserving memory attribution.
- Update context fixtures, the Wave prompt golden fragment and user documentation.

This is the read slice of [Subwaves · LOO-329](https://linear.app/loopflow/issue/LOO-329), built on [One SQLite owner per product object · LOO-298](https://linear.app/loopflow/issue/LOO-298). Parent discovery, single-segment stored names, rename identity and the release ownership/schedule split remain pending. Accepted design decisions and verification limits are retained in Infrastructure memory.

## Checks

**Not ready to merge:** focused Rust tests cannot compile against the inherited LOO-298 implementation. All-target Clippy, full formatting and architecture coverage also fail in unchanged inherited files; the portable architecture HTML is stale.

Changed-file formatting, diff whitespace and README/index synchronization pass. A current-source harness with reduced surrounding types passes ancestor order, file inclusion/exclusion, checkout-local edits, scratch recursion and single Wave rendering; its Wave fragment matches the committed golden. This is component evidence only. Full golden verification, real launch behavior and the design's disposable-Home proof remain open.
