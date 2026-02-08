# Gate Review: Typeahead Components, Prompt Engine, Worktrees + Triggers

Branch: `jack-heart.concertop.20260207_1101`

## What was implemented

Three categories of changes:

**Rust prompt engine** — Reordered prompt sections so diff (reference material) comes before step/direction (task). Extracted `format_reference_sections` and `format_step_tag` helpers to eliminate duplication between `format_prompt`, `format_context_prompt`, and `format_task_prompt`. Removed backward-compat wrappers (`trim_context`, `analyze_tokens`) and consolidated `inline`/`step_args`/`message` into a single `message` field on `GatherContextOpts`. Added directory support to `gather_files`.

**Rust daemon** — Wave runs now get their own worktree via `ensure_wave_worktree`, with a schema-based branch naming convention. The `lfd/loops/` module (cron, watch, loop_ticker, recovery) was deleted and replaced with a `lfd/triggers/` module. New `wave_runs` HTTP routes for listing/managing runs. Output streaming via `OutputHub`. Flow composition (fork/synthesize) executes in the executor with parallel branch support and worktree isolation.

**Swift Concerto** — Fish-style typeahead inputs for Direction and Flow selection (`DirectionTypeahead`, `FlowTypeahead`), backed by shared `GhostTextField`, `TypeaheadChip`, and `WrappingHStack` components. `OutputBuffer` replaces `SessionState` for output streaming. `StepRunner` simplified. Removed `isConfigured` gate, `AreaPicker`, `DirectionPills`, and `SessionState`. `dev.py` moved from `swift/scripts/` to repo-root `scripts/`.

**Python client** — New `test_python_client.py` with 21 tests covering model validation, URL resolution, error handling, and client API calls via `httpx.MockTransport`.

## Key choices

**Prompt ordering: diff before step/direction.** The diff is reference material the agent needs to understand the codebase state; the step and direction are the task instruction. Placing reference before task follows "context, then instruction."

**`format_reference_sections` extraction.** Both `format_prompt` (all-in-one) and `format_context_prompt` (system prompt file) render the same reference sections. Extracting the shared logic removes ~80 lines of duplication and ensures the two paths stay consistent.

**Consolidated `message` field.** `GatherContextOpts` had three fields (`inline`, `step_args`, `message`) for user-provided text. The caller already merged these before passing them in. Collapsing to one `message` field removes dead parameters.

**Wave worktrees.** Each wave run creates a worktree (or reuses an existing one) so waves don't conflict with each other or with the user's main checkout. Branch names follow the schema from `loopflow.toml`.

**Triggers replacing loops.** The old `loops/` module had five files implementing cron, watch, loop tick, and recovery logic. This was replaced with a simpler `triggers/` module that handles the same responsibilities with less code.

**Shared typeahead components.** `GhostTextField` uses `NSViewRepresentable` wrapping `NSTextField` to get fish-shell-style ghost text completion with tab-to-accept. Shared across Area, Direction, and Flow typeaheads. `WrappingHStack` custom layout wraps candidate chips to new lines.

## How it fits together

```
TypeaheadComponents.swift  (GhostTextField, TypeaheadChip, WrappingHStack)
        |                           |                    |
AreaTypeahead         DirectionTypeahead          FlowTypeahead
        |                           |                    |
                    WaveDetailPanel / StepRunner
```

Prompt engine:
```
format_reference_sections:  loopflow_doc, run_mode, wave, docs, summaries, area_docs, diff
format_prompt:              reference_sections + step + directions + clipboard + message
format_context_prompt:      reference_sections + clipboard
format_task_prompt:         directions + step
```

Daemon execution:
```
HTTP POST /waves/:id/run
  -> create_wave_run_with_id (worktree, branch)
  -> WaveExecutor::execute (step loop)
     -> run_step / run_fork / wait_interactive
     -> OutputHub streams lines to /waves/:id/logs
```

## Risks and bottlenecks

- **Ghost text attributed string updates.** `updateNSView` rewrites the attributed string on every SwiftUI state change. Direction/flow lists are small (tens of items), so this is fine in practice.

- **Fork worktree creation.** Each fork branch creates a separate worktree. For forks with many branches, this could consume disk space. `cleanup_fork` removes them after completion.

- **Trigger module is new.** The `triggers/` replacement for `loops/` is less tested in production than the code it replaces. The old code was deleted entirely (no backward-compat shim).

## What's not included

- **AreaTypeahead migration to `onFocusChange`.** The existing AreaTypeahead uses filesystem completion instead of the candidate list pattern. The new typeaheads add `onFocusChange` support to `GhostTextField` but AreaTypeahead doesn't use it yet.

- **Keyboard navigation of candidate list.** Arrow keys don't navigate the candidate chips. Selection is mouse/trackpad or type-to-filter with tab-to-complete.

- **Trigger test coverage.** The new `triggers/` module doesn't have dedicated tests yet (the old `loops/` module didn't either).

## Test results

- Rust: all pass (including new worktree and flow tests), 0 failed
- Python: 21 passed, 0 failed
- Swift: builds clean
- `cargo fmt`: clean
- `cargo clippy -- -D warnings`: clean

## Cleanup

- Added `*.actual.md` to `.gitignore` (stale golden test artifacts)
- Removed dead `idleWaveView` from `WaveDetailPanel`
- Removed duplicate Spacer in `WaveSidebar.disconnectedState`
- Fixed inline `import os` in `scripts/dev.py` (already imported at top)
- Deleted `AreaPicker.swift`, `DirectionPills.swift`, `SessionState.swift` (replaced by typeahead components and OutputBuffer)
