# Goals Worktree Triage - 2026-07-06

## Summary

The dirty Goals worktree is not one change. It mixes three real concerns plus
local bookkeeping:

1. Dispatch extraction from `lfd::executor` into `dispatch`.
2. Harness conformance trace hardening.
3. Swift deletion of local file-roadmap parsing now that PM is the roadmap
   source.
4. Local run/review artifacts.

Do not land this as-is. Split it before any more work accumulates.

## Concern Split

### Dispatch extraction

Files:
- `rust/loopflow/src/bin/lf.rs`
- `rust/loopflow/src/dispatch/mod.rs` (untracked)
- `rust/loopflow/src/lf/commands/chat.rs`
- `rust/loopflow/src/lfd/executor/helpers.rs`
- `rust/loopflow/src/lfd/executor/mod.rs`
- `rust/loopflow/src/lib.rs`
- `rust/loopflow/tests/wave_worktree_tests.rs`

Assessment:
- This overlaps directly with clean Architecture branch
  `jack-heart.architecture.20260705_1756` at `0b51525ed`.
- The moved dispatch code is effectively the same as Architecture's
  `rust/loopflow/src/dispatch.rs`; the only functional text difference found
  is use of the `Result` import alias in Goals.
- The important conflict is file layout: Architecture adds
  `rust/loopflow/src/dispatch.rs`; Goals adds
  `rust/loopflow/src/dispatch/mod.rs`. If both survive under `pub mod dispatch`,
  Rust will report duplicate module-file candidates.
- Architecture also updates the `lfd::executor` module comment to remove
  worktree placement from executor ownership; Goals removes the exports but
  leaves the older wording.

Recommendation:
- Prefer the Architecture branch as source of truth for dispatch extraction.
- Do not carry the Goals dispatch diff forward independently unless it is
  rebased onto Architecture and normalized to the same file layout.
- If preserving this work before splitting, create a patch from the dirty Goals
  tree rather than moving files in place:
  `git -C /Users/jack/src/loopflow.jack-heart.bugs.20260705_1627.goals diff --binary > scratch/goals-dirty-dispatch.patch`
  plus a separate archive of untracked files.

### Harness conformance traces

Files:
- `rust/loopflow/src/harness/codex.rs`
- `rust/loopflow/src/harness/conformance_tests.rs`
- `rust/loopflow/src/harness/testdata/claude_trace_manifest.json` (untracked)
- `rust/loopflow/src/harness/testdata/codex_trace_manifest.json` (untracked)
- `rust/loopflow/src/harness/testdata/opencode_trace_manifest.json`

Assessment:
- This is independent of dispatch.
- Adds explicit manifest metadata checks, fixture coverage checks, Codex method
  surface checks, OpenCode event surface checks, and Codex send/steer/interrupt
  driver tests.
- Safe to split into its own harness-focused branch after preserving untracked
  manifests.

Recommendation:
- Checkpoint as a harness-only patch including untracked manifests.
- Run focused Rust harness tests before landing this concern.

### Swift file-roadmap parser deletion

Files:
- `swift/Concerto/UX_DESIGN.md`
- `swift/ConcertoTests/WaveContentParserTests.swift`
- `swift/LoopflowCore/Models/WaveContent.swift`
- `swift/LoopflowCore/Services/WaveContentParser.swift`
- `swift/LoopflowCore/State/RepoState.swift`
- `swift/README.md`

Assessment:
- This removes `RoadmapItem`, `RoadmapPriority`, `roadmapItems`,
  numbered-markdown roadmap parsing, and local priority renaming.
- `rg` found no remaining Swift references to those symbols after the dirty
  edits.
- This is separate from Rust dispatch and harness work. It matches
  `scratch/wave-review.md`'s stated "File-roadmap leftovers" lane.

Recommendation:
- Split as a Concerto/Swift branch.
- Run the Swift test target that includes `WaveContentParserTests` in an
  environment where SwiftPM can run.

### Local artifacts

Files:
- `.lf/metrics/ops.jsonl`
- `scratch/wave-review.md` (untracked in target worktree)

Assessment:
- `.lf/metrics/ops.jsonl` records the creation of the sibling
  `goals.dispatch` worktree.
- `scratch/wave-review.md` describes the same three mixed lanes and claims them
  landed, but they are still uncommitted in the target worktree.

Recommendation:
- Do not include `.lf/metrics/ops.jsonl` in product changes unless the branch
  intentionally tracks local metrics.
- Keep or copy `scratch/wave-review.md` only as review context; it is not part
  of any one implementation concern.

## Safe Next Steps

1. Leave `/Users/jack/src/loopflow.jack-heart.bugs.20260705_1627.goals` dirty
   until patches are captured. It is the only place where all three concerns
   currently coexist.
2. Use the already-created clean sibling worktree
   `/Users/jack/src/loopflow.jack-heart.bugs.20260705_1627.goals.dispatch`
   only as a split target; it is clean at `93e13c4bf`.
3. Land or rebase onto `jack-heart.architecture.20260705_1756` before doing
   anything with the dispatch extraction. Adopt `src/dispatch.rs`, not both
   `src/dispatch.rs` and `src/dispatch/mod.rs`.
4. Split harness conformance and Swift parser deletion into separate branches
   from the same base. They do not need to wait on each other.
5. After each split, run focused validation:
   - Dispatch: `cargo fmt`, then targeted Rust worktree/dispatch tests.
   - Harness: targeted harness conformance and Codex harness tests.
   - Swift: `WaveContentParserTests` and any Concerto model/view compile checks.

## Evidence

Commands run:
- `git -C /Users/jack/src/loopflow.jack-heart.bugs.20260705_1627.goals status --short --branch`
- `git -C /Users/jack/src/loopflow.jack-heart.bugs.20260705_1627.goals diff --stat`
- `git -C /Users/jack/src/loopflow.jack-heart.bugs.20260705_1627.goals diff --name-status`
- `git -C /Users/jack/src/loopflow.jack-heart.bugs.20260705_1627.goals status --porcelain=v1 --untracked-files=all`
- `git -C /Users/jack/src/loopflow.jack-heart.bugs.20260705_1627.goals worktree list --porcelain`
- `git -C /Users/jack/src/loopflow.architecture.1bda806f status --short --branch`
- `git -C /Users/jack/src/loopflow.architecture.1bda806f show --stat --oneline --decorate --no-renames HEAD`
- `diff -u <(git -C /Users/jack/src/loopflow.architecture.1bda806f show HEAD:rust/loopflow/src/dispatch.rs) /Users/jack/src/loopflow.jack-heart.bugs.20260705_1627.goals/rust/loopflow/src/dispatch/mod.rs`
- `rg -n "RoadmapItem|roadmapItems|RoadmapPriority|updateRoadmapPriority" /Users/jack/src/loopflow.jack-heart.bugs.20260705_1627.goals/swift`
- `rg -n "lfd::executor::(create_run_for_placement|ensure_wave_worktree|Placement)|crate::lfd::executor::(create_run_for_placement|ensure_wave_worktree|Placement)|dispatch::" /Users/jack/src/loopflow.jack-heart.bugs.20260705_1627.goals/rust/loopflow/src /Users/jack/src/loopflow.jack-heart.bugs.20260705_1627.goals/rust/loopflow/tests`
