# 02: Inline Glanceability

Inline the glance, link to the deep dive. Eliminate context-switches to GitHub/Cursor for the "check in" workflow.

**Status: done**

## What to build

All items shipped. G1 shipped in prior PR. G2–G5 shipped — see `scratch/ux-inline-glance.md`.

### ~~File items show diffs (G1)~~ — done

`DiffLinesView` renders colored inline diffs. `synthesize_edit_diff()` in the Claude harness populates `FileEdit.diff` from Edit tool inputs.

### ~~Wave diff stat per-file expand (G2)~~ — done
### ~~Roadmap item expansion (G3)~~ — done
### ~~Wave README full content (G4)~~ — done
### ~~Scratch doc glance (G5)~~ — done

## Constraints

- Inline views are for glancing. Deep dives link to GitHub (diffs/PRs) or Cursor (editing/docs).
- `DiffLinesView` is a shared component, reusable for G1, G2, and future features.
- No syntax highlighting in diffs — colored +/- lines only.

## Validation

- `swift test --package-path swift`
- `cargo test --all` (if lfd endpoint added)
- Manual: expand a file item to see diff, tap diff stat file to see per-file diff, expand roadmap item to read content

## Done when

You can check "what changed?" and "what's the plan?" without leaving Concerto.
