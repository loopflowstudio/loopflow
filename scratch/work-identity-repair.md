# Slice 6D repair: finish false-Session deletion and prove WorkCensus behavior

## Problem

The wire contracts use Work/Task/Launch, but current source still calls stable
Project or Task identity a Session in production comments, user docs, and test
infrastructure. The review's broad-deletion claim is therefore false.

Known residue:

- `rust/loopflow/src/child.rs`
- `rust/loopflow/src/lf/commands/waves.rs`
- `rust/loopflow/src/pm/linear.rs`
- `rust/loopflow/src/harness/mod.rs`
- `docs/lf.md`
- `swift/DESIGN.md`
- `rust/loopflow/tests/support/mod.rs` (`RegisteredTask.session` and callers)
- `swift/LoopflowTests/RoadmapViewTests.swift`
- `rust/loopflow/tests/wave_resolution_tests.rs`

`WorkActivity.isOpenable` derives from optional `launchId`, but no focused test
proves openability or the absence of the retired action array.

The Slice 6C review also claims an authored flow rejects `receipt`, but the
named proof cannot be located. Either add the smallest real parser proof or
remove that claim; do not retain evidence that cannot be reproduced.

## Required behavior

- Stable Wave, Project, and Task identity is named Work or by its domain noun.
- Run and Launch remain their own nouns.
- Preserve Session only for real provider continuation, tmux/Ghostty/browser,
  URLSession, human work periods, or explicit historical migration/release
  text.
- Rename the shared test fixture field and all callers; do not add aliases.
- Add focused Swift behavior tests showing a Work row with `launchId` is
  openable and one without it is not. The model has no action enum/array.
- Keep the Context Lab LaunchSet wire contract unchanged.
- Correct the review ledger to match reproducible proof.

## Done when

- [x] The listed false-Session residue is renamed after reading each context.
- [x] A broad current-source/docs/test audit leaves only a documented legitimate
      Session allowlist.
- [x] `RegisteredTask.task` replaces `.session` with no compatibility property.
- [x] WorkCensus tests prove both openable and non-openable rows from `launchId`.
- [x] Old `ActiveSession(s)`, `SessionAction`, SessionSet types, keys, aliases,
      and defaults remain absent.
- [x] The Receipt review claim is either backed by a named test or removed.
- [x] Focused Rust/Swift tests, DTO fixtures, format, and Clippy pass.
- [x] `scratch/feedback-runtime-review.md` records the repair exactly.
