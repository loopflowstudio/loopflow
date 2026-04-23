# Flows View Polish

**Finish line:** Flows tab handles search/filter, reads xor/maybe path labels clearly, and has an iOS layout that works on a single-column screen.

Speculative. The static Mac catalog is shipped and usable; these refinements wait until the session-state overlay lands and the `maybe` primitive changes the rendering surface.

## Open questions (from flows-view design)

- **xor path labels in the tree.** Show just the key (`act`, `silence`) or the full description too? Key is compact, description is self-documenting. Probably show both, with the description as secondary text — but verify after `maybe` ships and xor has fewer users.
- **Search/filter.** Live search across both panes: type `gate`, see the step in the catalog and every flow that uses it. Useful once the catalog grows past what fits on one screen.
- **iOS layout.** The two-pane Mac model (catalog left, used-by right) doesn't fit. Likely shape: single tree, tap a step to push-nav into a "used by" detail screen. Defer until Mac v1 patterns are stable — inventing an iOS layout for a Mac UX that's still shifting wastes the work.

## Scope

- **In**: search/filter, xor/maybe label polish, iOS layout.
- **Out**: cross-wave flow comparisons, editable catalog, drag-to-reorder. The catalog is a reference surface, not an editor.

## Risks

- Search is easy to over-engineer. Start with substring match on flow/step name; defer fuzzy and semantic search until the naive version proves insufficient.
- iOS layout could grow a third UI that fights with Mac — anchor it to the same catalog DTO and same "used by" walk. No iOS-specific data shapes.
