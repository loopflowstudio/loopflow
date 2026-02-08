# Gate Review: Typeahead Components + Prompt Order

Branch: `jack-heart.concertop.20260207_1101`

## What was implemented

Two categories of changes:

**Rust prompt engine** — Reordered prompt sections so diff (reference material) comes before step/direction (task), extracted `format_direction_tags` helper, moved directions from context prompt into task prompt, and added directory support to `gather_files`.

**Swift typeahead components** — New fish-style typeahead inputs for Direction and Flow selection (`DirectionTypeahead`, `FlowTypeahead`), backed by shared `GhostTextField`, `TypeaheadChip`, and `WrappingHStack` components in `TypeaheadComponents.swift`. Removed `isConfigured` gate on wave launch.

## Key choices

**Prompt ordering: diff before step/direction.** The diff is reference material the agent needs to understand the codebase state; the step and direction are the task instruction. Placing reference before task follows the pattern of "context, then instruction." This also means `format_context_prompt` no longer includes directions — they go in `format_task_prompt` alongside the step, keeping all task-level instructions together.

**Shared typeahead components.** `GhostTextField` uses `NSViewRepresentable` wrapping `NSTextField` to get fish-shell-style ghost text completion with tab-to-accept. This is shared across Area, Direction, and Flow typeaheads. The ghost text is rendered as an attributed string suffix with placeholder color, and a coordinator strips ghost text from input changes to prevent it from being committed as typed text.

**`WrappingHStack` custom layout.** Candidate chips wrap to new lines when they exceed the available width, rather than scrolling horizontally.

**Removed `isConfigured` guard.** Waves no longer require `area` to be set before launching. Area defaults to the whole repo, so blocking on it was unnecessary friction.

## How it fits together

```
TypeaheadComponents.swift  (GhostTextField, TypeaheadChip, WrappingHStack)
        ↑                           ↑                    ↑
AreaTypeahead         DirectionTypeahead          FlowTypeahead
        ↓                           ↓                    ↓
                    WaveDetailPanel (consumer)
```

In the prompt engine:
```
format_prompt:        loopflow → mode → wave → docs → diff → step → direction → clipboard
format_context_prompt: loopflow → mode → wave → docs → diff → clipboard (no step/direction)
format_task_prompt:    direction(s) → step
```

## Risks and bottlenecks

- **Ghost text attributed string updates.** The `updateNSView` method rewrites the attributed string on every SwiftUI state change. If the candidate list is very large or the view updates rapidly, this could cause flickering. In practice, direction/flow lists are small (tens of items) so this is fine.

- **`gather_dir_files` with large directories.** Collects all entries into a Vec for sorting. For very large directories this could use significant memory, but the `ignore` crate's standard filters (respecting `.gitignore`) keep this bounded in practice.

## What's not included

- **AreaTypeahead migration to `onFocusChange`.** The existing AreaTypeahead doesn't use the candidate list pattern (it uses filesystem completion instead). The new typeaheads add `onFocusChange` support to `GhostTextField` but AreaTypeahead doesn't use it yet.

- **Keyboard navigation of candidate list.** Arrow keys don't navigate the candidate chips — selection is mouse/trackpad or type-to-filter with tab-to-complete. This could be added later.

## Test results

- Rust: 299 passed, 0 failed, 1 ignored
- Python: 21 passed, 0 failed
- Golden prompt parity: passing
- `cargo fmt`: clean
- `cargo clippy -- -D warnings`: clean

## Cleanup

- Added `*.actual.md` to `.gitignore` (stale golden test artifacts)
